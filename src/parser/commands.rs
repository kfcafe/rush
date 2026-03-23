use super::*;

impl Parser {
    pub(crate) fn parse_command_or_pipeline(&mut self) -> Result<Statement> {
        // Check for pipeline negation (! prefix)
        let negated = if self.match_token(&Token::Bang) {
            self.advance();
            true
        } else {
            false
        };
        
        let first_statement = self.parse_pipeline_element()?;

        // Check if this is a parallel execution
        let result = if self.match_token(&Token::ParallelPipe) {
            // Only commands can be in parallel execution for now
            let first_command = match first_statement {
                Statement::Command(cmd) => cmd,
                _ => return Err(anyhow!("Only commands can be used in parallel execution")),
            };

            self.advance();
            let mut commands = vec![first_command];

            loop {
                let stmt = self.parse_pipeline_element()?;
                let cmd = match stmt {
                    Statement::Command(cmd) => cmd,
                    _ => return Err(anyhow!("Only commands can be used in parallel execution")),
                };
                commands.push(cmd);

                if !self.match_token(&Token::ParallelPipe) {
                    break;
                }
                self.advance();
            }

            Statement::ParallelExecution(ParallelExecution { commands })
        }
        // Check if this is a pipeline
        else if self.match_token(&Token::Pipe) {
            // Build elements list supporting commands, subshells, and compound commands
            let first_element = Self::statement_to_pipeline_element(first_statement)?;

            self.advance();
            let mut elements = vec![first_element];

            loop {
                let stmt = self.parse_pipeline_element()?;
                let elem = Self::statement_to_pipeline_element(stmt)?;
                elements.push(elem);

                if !self.match_token(&Token::Pipe) {
                    break;
                }
                self.advance();
            }

            // Parse any redirects that follow the pipeline and apply to the last command
            let mut redirects = Vec::new();
            while self.match_redirect_token() {
                redirects.push(self.parse_single_redirect()?);
            }

            // Apply redirects to the last command in the pipeline (only if it's a command, not a subshell)
            if !redirects.is_empty() {
                if let Some(PipelineElement::Command(cmd)) = elements.last_mut() {
                    cmd.redirects.extend(redirects);
                } else {
                    // If last element is a subshell, we need to convert the pipeline and apply redirects differently
                    // For now, we'll store the redirects and handle them in execution
                    // This would require extending the Pipeline struct
                }
            }

            // Build backward-compatible commands vec from command-only elements
            let commands: Vec<Command> = elements
                .iter()
                .filter_map(|e| match e {
                    PipelineElement::Command(cmd) => Some(cmd.clone()),
                    PipelineElement::Subshell(_) | PipelineElement::CompoundCommand(_) => None,
                })
                .collect();

            Statement::Pipeline(Pipeline { commands, elements, negated })
        } else if negated {
            // Single command with negation - wrap in a Pipeline with negated=true
            let element = Self::statement_to_pipeline_element(first_statement)?;
            let commands = match &element {
                PipelineElement::Command(cmd) => vec![cmd.clone()],
                _ => vec![],
            };
            Statement::Pipeline(Pipeline { commands, elements: vec![element], negated: true })
        } else {
            first_statement
        };

        // Check for |? (pipe to AI)
        if self.match_token(&Token::PipeAsk) {
            self.advance();
            let prompt = self.parse_pipe_ask_prompt()?;
            return Ok(Statement::PipeAsk(PipeAsk {
                command: Box::new(result),
                prompt,
            }));
        }

        Ok(result)
    }

    fn parse_pipe_ask_prompt(&mut self) -> Result<String> {
        match self.peek() {
            // Quoted string prompt
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                let unquoted = Self::strip_outer_quotes(&s, '"');
                Ok(Self::process_double_quote_escapes(&unquoted))
            }
            Some(Token::SingleQuotedString(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Self::strip_outer_quotes(&s, '\''))
            }
            Some(Token::AnsiCString(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            // Unquoted word prompt
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            // No prompt provided - use empty string (AI will use default behavior)
            _ => Ok(String::new()),
        }
    }

    fn match_redirect_token(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::GreaterThan)
                | Some(Token::StdoutAppend)
                | Some(Token::StdinRedirect)
                | Some(Token::StderrRedirect)
                | Some(Token::StderrToStdout)
                | Some(Token::BothRedirect)
                | Some(Token::FdDup(_))
                | Some(Token::HereString)
        )
    }

    fn parse_single_redirect(&mut self) -> Result<Redirect> {
        match self.peek() {
            Some(Token::GreaterThan) => {
                self.advance();
                let target = self.parse_redirect_target()?;
                Ok(Redirect {
                    kind: RedirectKind::Stdout,
                    target: Some(target),
                })
            }
            Some(Token::StdoutAppend) => {
                self.advance();
                let target = self.parse_redirect_target()?;
                Ok(Redirect {
                    kind: RedirectKind::StdoutAppend,
                    target: Some(target),
                })
            }
            Some(Token::StdinRedirect) => {
                self.advance();
                let target = self.parse_redirect_target()?;
                Ok(Redirect {
                    kind: RedirectKind::Stdin,
                    target: Some(target),
                })
            }
            Some(Token::StderrRedirect) => {
                self.advance();
                let target = self.parse_redirect_target()?;
                Ok(Redirect {
                    kind: RedirectKind::Stderr,
                    target: Some(target),
                })
            }
            Some(Token::StderrToStdout) => {
                self.advance();
                Ok(Redirect {
                    kind: RedirectKind::StderrToStdout,
                    target: None,
                })
            }
            Some(Token::BothRedirect) => {
                self.advance();
                let target = self.parse_redirect_target()?;
                Ok(Redirect {
                    kind: RedirectKind::Both,
                    target: Some(target),
                })
            }
            Some(Token::FdDup((src, dst))) => {
                let src = *src;
                let dst = *dst;
                self.advance();
                Ok(Redirect {
                    kind: RedirectKind::FdDup { src, dst },
                    target: None,
                })
            }
            Some(Token::HereString) => {
                self.advance();
                let target = self.parse_redirect_target()?;
                Ok(Redirect {
                    kind: RedirectKind::HereString,
                    target: Some(target),
                })
            }
            _ => Err(anyhow!("Expected redirect token")),
        }
    }

    fn parse_pipeline_element(&mut self) -> Result<Statement> {
        // Check for compound commands first (can appear after pipe)
        match self.peek() {
            Some(Token::While) => return self.parse_while_loop(),
            Some(Token::Until) => return self.parse_until_loop(),
            Some(Token::For) => return self.parse_for_loop(),
            Some(Token::If) => return self.parse_if_statement(),
            Some(Token::Case) => return self.parse_case_statement(),
            Some(Token::LeftBrace) => return self.parse_brace_group(),
            Some(Token::LeftParen) => return self.parse_subshell(),
            _ => {}
        }
        
        if self.is_bare_assignment() {
            self.parse_bare_assignment_or_command()
        } else {
            Ok(Statement::Command(self.parse_command()?))
        }
    }

    fn parse_brace_group(&mut self) -> Result<Statement> {
        self.expect_token(&Token::LeftBrace)?;
        
        let mut statements = Vec::new();
        
        // Skip leading newlines
        while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }
        
        // Parse statements until we hit a closing brace
        while !self.match_token(&Token::RightBrace) && !self.is_at_end() {
            // Skip newlines between statements
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }
            
            if self.match_token(&Token::RightBrace) {
                break;
            }
            
            statements.push(self.parse_conditional_statement()?);
            
            // Handle statement separators (semicolon)
            if self.match_token(&Token::Semicolon) {
                self.advance();
            }
        }
        
        self.expect_token(&Token::RightBrace)?;
        
        Ok(Statement::BraceGroup(statements))
    }

    fn statement_to_pipeline_element(stmt: Statement) -> Result<PipelineElement> {
        match stmt {
            Statement::Command(cmd) => Ok(PipelineElement::Command(cmd)),
            Statement::Subshell(stmts) => Ok(PipelineElement::Subshell(stmts)),
            // Compound commands can be pipeline elements
            Statement::WhileLoop(_)
            | Statement::UntilLoop(_)
            | Statement::ForLoop(_)
            | Statement::IfStatement(_)
            | Statement::CaseStatement(_)
            | Statement::BraceGroup(_) => Ok(PipelineElement::CompoundCommand(Box::new(stmt))),
            _ => Err(anyhow!("This statement type cannot be used in pipelines")),
        }
    }

    pub(crate) fn parse_command(&mut self) -> Result<Command> {
        let name = match self.advance() {
            Some(Token::Identifier(s)) | Some(Token::Path(s)) | Some(Token::GlobPattern(s)) => s.clone(),
            Some(Token::LeftBracket) => "[".to_string(),
            Some(Token::DoubleLeftBracket) => "[[".to_string(),
            Some(Token::Colon) => ":".to_string(),
            Some(Token::Dot) => ".".to_string(),
            _ => return Err(anyhow!("Expected command name")),
        };

        let mut args = Vec::new();
        let mut redirects = Vec::new();

        while !self.is_at_end()
            && !self.match_token(&Token::Pipe)
            && !self.match_token(&Token::PipeAsk)
            && !self.match_token(&Token::ParallelPipe)
            && !self.match_token(&Token::Newline)
            && !self.match_token(&Token::Semicolon)
            && !self.match_token(&Token::And)
            && !self.match_token(&Token::Or)
            && !self.match_token(&Token::Ampersand)
            && !self.match_token(&Token::RightParen)
            && !self.match_token(&Token::Then)
            && !self.match_token(&Token::Fi)
            && !self.match_token(&Token::Elif)
            && !self.match_token(&Token::Else)
            && !self.match_token(&Token::Do)
            && !self.match_token(&Token::Done)
            && !self.match_token(&Token::Esac)
            && !self.match_token(&Token::DoubleSemicolon)
            && !self.match_token(&Token::RightBrace)
        {
            match self.peek() {
                Some(Token::GreaterThan) => {
                    self.advance();
                    let target = self.parse_redirect_target()?;
                    redirects.push(Redirect {
                        kind: RedirectKind::Stdout,
                        target: Some(target),
                    });
                }
                Some(Token::StdoutAppend) => {
                    self.advance();
                    let target = self.parse_redirect_target()?;
                    redirects.push(Redirect {
                        kind: RedirectKind::StdoutAppend,
                        target: Some(target),
                    });
                }
                Some(Token::StdinRedirect) => {
                    self.advance();
                    let target = self.parse_redirect_target()?;
                    redirects.push(Redirect {
                        kind: RedirectKind::Stdin,
                        target: Some(target),
                    });
                }
                Some(Token::StderrRedirect) => {
                    self.advance();
                    // Check if next token is >&1 (for 2>&1)
                    // Note: 2>&1 is handled as a single token StderrToStdout
                    let target = self.parse_redirect_target()?;
                    redirects.push(Redirect {
                        kind: RedirectKind::Stderr,
                        target: Some(target),
                    });
                }
                Some(Token::StderrToStdout) => {
                    self.advance();
                    redirects.push(Redirect {
                        kind: RedirectKind::StderrToStdout,
                        target: None,
                    });
                }
                Some(Token::BothRedirect) => {
                    self.advance();
                    let target = self.parse_redirect_target()?;
                    redirects.push(Redirect {
                        kind: RedirectKind::Both,
                        target: Some(target),
                    });
                }
                Some(Token::FdDup(..)) => {
                    if let Some(Token::FdDup((src, dst))) = self.peek().cloned() {
                        self.advance();
                        redirects.push(Redirect {
                            kind: RedirectKind::FdDup { src, dst },
                            target: None,
                        });
                    }
                }
                Some(Token::HereDocBody(..)) => {
                    let token = self.advance().cloned();
                    if let Some(Token::HereDocBody(data)) = token {
                        let kind = if data.expand_vars {
                            RedirectKind::HereDoc
                        } else {
                            RedirectKind::HereDocLiteral
                        };
                        redirects.push(Redirect {
                            kind,
                            target: Some(data.body),
                        });
                    }
                }
                Some(Token::HereString) => {
                    self.advance();
                    let target = self.parse_redirect_target()?;
                    redirects.push(Redirect {
                        kind: RedirectKind::HereString,
                        target: Some(target),
                    });
                }
                _ => {
                    args.push(self.parse_argument()?);
                }
            }
        }

        Ok(Command {
            name,
            args,
            redirects,
            prefix_env: vec![],
        })
    }

    pub(crate) fn parse_argument(&mut self) -> Result<Argument> {
        match self.advance() {
            Some(Token::String(s)) => {
                // Double-quoted string: remove outer quotes and process escape sequences
                let unquoted = Self::strip_outer_quotes(s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                Ok(Argument::Literal(processed))
            }
            Some(Token::SingleQuotedString(s)) => {
                // Single-quoted string: remove outer quotes, keep content literal (no escape processing)
                let unquoted = Self::strip_outer_quotes(s, '\'');
                Ok(Argument::Literal(unquoted))
            }
            Some(Token::AnsiCString(s)) => {
                // ANSI-C string: already processed by lexer
                Ok(Argument::Literal(s.clone()))
            }
            Some(Token::Identifier(s)) => {
                // Check if this is NAME=VALUE pattern (e.g., for `export FOO=bar`)
                let s = s.clone();
                if self.match_token(&Token::Equals)
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                {
                    self.advance(); // consume '='
                    let value = self.parse_assignment_value()?;
                    Ok(Argument::Literal(format!("{}={}", s, value)))
                } else {
                    Ok(Argument::Literal(s))
                }
            }
            Some(Token::GlobPattern(s)) => Ok(Argument::Glob(s.clone())),
            Some(Token::Variable(s)) | Some(Token::SpecialVariable(s)) => {
                Ok(Argument::Variable(s.clone()))
            }
            Some(Token::BracedVariable(s)) => Ok(Argument::BracedVariable(s.clone())),
            Some(Token::CommandSubstitution(s)) => Ok(Argument::CommandSubstitution(s.clone())),
            Some(Token::BacktickSubstitution(s)) => Ok(Argument::CommandSubstitution(s.clone())),
            Some(Token::ProcessSubIn(s)) => Ok(Argument::ProcessSubIn(s.clone())),
            Some(Token::ProcessSubOut(s)) => Ok(Argument::ProcessSubOut(s.clone())),
            Some(Token::ShortFlag(s)) | Some(Token::LongFlag(s)) | Some(Token::PlusFlag(s)) => {
                Ok(Argument::Flag(s.clone()))
            }
            Some(Token::Path(s)) => Ok(Argument::Path(s.clone())),
            Some(Token::Tilde) => Ok(Argument::Path("~".to_string())),
            Some(Token::Integer(n)) => Ok(Argument::Literal(n.to_string())),
            Some(Token::Dot) => Ok(Argument::Path(".".to_string())),
            Some(Token::RightBracket) => Ok(Argument::Literal("]".to_string())),
            Some(Token::DoubleRightBracket) => Ok(Argument::Literal("]]".to_string())),
            // Allow operators as arguments for test builtin
            Some(Token::Equals) => Ok(Argument::Literal("=".to_string())),
            Some(Token::DoubleEquals) => Ok(Argument::Literal("==".to_string())),
            Some(Token::NotEquals) => Ok(Argument::Literal("!=".to_string())),
            Some(Token::GreaterThanOrEqual) => Ok(Argument::Literal(">=".to_string())),
            Some(Token::LessThanOrEqual) => Ok(Argument::Literal("<=".to_string())),
            Some(Token::GreaterThan) => Ok(Argument::Literal(">".to_string())),
            Some(Token::Bang) => Ok(Argument::Literal("!".to_string())),
            Some(Token::Dash) => Ok(Argument::Literal("-".to_string())),
            Some(Token::DoubleDash) => Ok(Argument::Literal("--".to_string())),
            Some(Token::Float(f)) => Ok(Argument::Literal(f.to_string())),
            // Keywords used as arguments (e.g., `echo match`, `echo case`)
            Some(Token::Match) => Ok(Argument::Literal("match".to_string())),
            Some(Token::Case) => Ok(Argument::Literal("case".to_string())),
            Some(Token::Esac) => Ok(Argument::Literal("esac".to_string())),
            Some(Token::In) => Ok(Argument::Literal("in".to_string())),
            Some(Token::Function) => Ok(Argument::Literal("function".to_string())),
            _ => Err(anyhow!("Expected argument")),
        }
    }

    fn parse_subshell(&mut self) -> Result<Statement> {
        self.expect_token(&Token::LeftParen)?;

        let mut statements = Vec::new();

        // Skip leading newlines
        while self.match_token(&Token::Newline) || self.match_token(&Token::CrLf) {
            self.advance();
        }

        // Parse statements until we hit a closing paren
        while !self.match_token(&Token::RightParen) && !self.is_at_end() {
            // Skip newlines between statements
            while self.match_token(&Token::Newline) || self.match_token(&Token::CrLf) {
                self.advance();
            }

            if self.match_token(&Token::RightParen) {
                break;
            }

            statements.push(self.parse_statement()?);

            // Handle statement separators (&&, semicolon)
            if self.match_token(&Token::And) || self.match_token(&Token::Semicolon) {
                self.advance();
            }
        }

        self.expect_token(&Token::RightParen)?;

        Ok(Statement::Subshell(statements))
    }

    fn parse_redirect_target(&mut self) -> Result<String> {
        match self.advance() {
            Some(Token::Path(s)) | Some(Token::Identifier(s)) => Ok(s.clone()),
            Some(Token::String(s)) => {
                let unquoted = Self::strip_outer_quotes(s, '"');
                Ok(Self::process_double_quote_escapes(&unquoted))
            }
            Some(Token::SingleQuotedString(s)) => Ok(Self::strip_outer_quotes(s, '\'')),
            Some(Token::AnsiCString(s)) => Ok(s.clone()),
            _ => Err(anyhow!("Expected redirect target")),
        }
    }
}
