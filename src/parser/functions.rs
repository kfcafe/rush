use super::*;

impl Parser {
    pub(crate) fn parse_assignment(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Let)?;

        let name = match self.advance() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Err(anyhow!("Expected variable name")),
        };

        self.expect_token(&Token::Equals)?;

        let value = self.parse_expression()?;

        Ok(Statement::Assignment(Assignment { name, value }))
    }

    pub(crate) fn parse_expression(&mut self) -> Result<Expression> {
        // For now, simple expression parsing
        match self.peek() {
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                let unquoted = Self::strip_outer_quotes(&s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                Ok(Expression::Literal(Literal::String(processed)))
            }
            Some(Token::SingleQuotedString(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::Literal(Literal::String(
                    Self::strip_outer_quotes(&s, '\''),
                )))
            }
            Some(Token::AnsiCString(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::Literal(Literal::String(s)))
            }
            Some(Token::Integer(n)) => {
                let n = *n;
                self.advance();
                Ok(Expression::Literal(Literal::Integer(n)))
            }
            Some(Token::Float(f)) => {
                let f = *f;
                self.advance();
                Ok(Expression::Literal(Literal::Float(f)))
            }
            Some(Token::Variable(v)) | Some(Token::SpecialVariable(v)) => {
                let v = v.clone();
                self.advance();
                Ok(Expression::Variable(v))
            }
            Some(Token::CommandSubstitution(cmd)) => {
                let cmd = cmd.clone();
                self.advance();
                Ok(Expression::CommandSubstitution(cmd))
            }
            Some(Token::BacktickSubstitution(cmd)) => {
                let cmd = cmd.clone();
                self.advance();
                Ok(Expression::CommandSubstitution(cmd))
            }
            Some(Token::BracedVariable(braced_var)) => {
                let braced_var = braced_var.clone();
                self.advance();
                let expansion = self.parse_var_expansion(&braced_var)?;
                Ok(Expression::VariableExpansion(expansion))
            }
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::Literal(Literal::String(s)))
            }
            _ => Err(anyhow!("Expected expression")),
        }
    }

    pub(crate) fn parse_function_def(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Fn)?;

        let name = match self.advance() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Err(anyhow!("Expected function name")),
        };

        self.expect_token(&Token::LeftParen)?;

        let params = self.parse_parameters()?;

        self.expect_token(&Token::RightParen)?;
        self.expect_token(&Token::LeftBrace)?;

        let body = self.parse_block()?;

        self.expect_token(&Token::RightBrace)?;

        Ok(Statement::FunctionDef(FunctionDef { name, params, body }))
    }

    pub(crate) fn parse_bash_function_def(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Function)?;

        let name = match self.advance() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Err(anyhow!("Expected function name after 'function'")),
        };

        // Optional () after function name
        if self.match_token(&Token::LeftParen) {
            self.advance();
            self.expect_token(&Token::RightParen)?;
        }

        // Skip optional newlines between name/() and {
        while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        self.expect_token(&Token::LeftBrace)?;

        let body = self.parse_block()?;

        self.expect_token(&Token::RightBrace)?;

        Ok(Statement::FunctionDef(FunctionDef {
            name,
            params: vec![],
            body,
        }))
    }

    pub(crate) fn parse_posix_function_def(&mut self) -> Result<Statement> {
        let name = match self.advance() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Err(anyhow!("Expected function name")),
        };

        self.expect_token(&Token::LeftParen)?;
        self.expect_token(&Token::RightParen)?;

        // Skip optional newlines between () and {
        while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        self.expect_token(&Token::LeftBrace)?;

        let body = self.parse_block()?;

        self.expect_token(&Token::RightBrace)?;

        Ok(Statement::FunctionDef(FunctionDef {
            name,
            params: vec![],
            body,
        }))
    }

    pub(crate) fn is_posix_function_def(&self) -> bool {
        if let Some(Token::Identifier(name)) = self.tokens.get(self.position) {
            // Must be a valid variable-like name (no dots/dashes)
            let valid_name = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
            valid_name
                && self.tokens.get(self.position + 1) == Some(&Token::LeftParen)
                && self.tokens.get(self.position + 2) == Some(&Token::RightParen)
        } else {
            false
        }
    }

    fn parse_parameters(&mut self) -> Result<Vec<Parameter>> {
        let mut params = Vec::new();

        while !self.match_token(&Token::RightParen) {
            let name = match self.advance() {
                Some(Token::Identifier(s)) => s.clone(),
                _ => return Err(anyhow!("Expected parameter name")),
            };

            let type_hint = if self.match_token(&Token::Colon) {
                self.advance();
                match self.advance() {
                    Some(Token::Identifier(s)) => Some(s.clone()),
                    _ => None,
                }
            } else {
                None
            };

            params.push(Parameter { name, type_hint });

            if self.match_token(&Token::Comma) {
                self.advance();
            }
        }

        Ok(params)
    }

    pub(crate) fn is_bare_assignment(&self) -> bool {
        if let Some(Token::Identifier(name)) = self.tokens.get(self.position) {
            if self.tokens.get(self.position + 1) == Some(&Token::Equals) {
                // Ensure it's a valid shell variable name (starts with letter/underscore,
                // contains only alphanumeric/underscore). The lexer already enforces this
                // for Identifier tokens (regex: [a-zA-Z_][a-zA-Z0-9_.\-]*), but we should
                // also exclude names with dots/dashes (those are filenames, not variables).
                name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            } else {
                false
            }
        } else {
            false
        }
    }

    pub(crate) fn parse_bare_assignment_or_command(&mut self) -> Result<Statement> {
        let mut assignments: Vec<(String, String)> = Vec::new();

        // Collect all leading NAME=VALUE pairs
        while self.is_bare_assignment() {
            let name = match self.advance() {
                Some(Token::Identifier(s)) => s.clone(),
                _ => unreachable!(),
            };
            self.expect_token(&Token::Equals)?;

            // Parse the value: can be an identifier, string, integer, variable, path, or empty
            let value = self.parse_assignment_value()?;
            assignments.push((name, value));
        }

        // Check if there's a command following the assignments
        let has_command = !self.is_at_end()
            && !self.match_token(&Token::Semicolon)
            && !self.match_token(&Token::Newline)
            && !self.match_token(&Token::CrLf)
            && !self.match_token(&Token::Pipe)
            && !self.match_token(&Token::ParallelPipe)
            && !self.match_token(&Token::And)
            && !self.match_token(&Token::Or)
            && !self.match_token(&Token::Ampersand)
            && !self.match_token(&Token::RightParen);

        if has_command {
            // FOO=bar cmd args -- parse as command with prefix env
            let mut cmd = self.parse_command()?;
            cmd.prefix_env = assignments;
            Ok(Statement::Command(cmd))
        } else {
            // Standalone assignment(s) with no command following.
            // Return the last assignment. For `A=1 B=2` without a command,
            // the first assignments are consumed but not returned as statements.
            // This is acceptable since multi-assignment without command is rare;
            // the primary use case is `A=1 B=2 cmd` which uses prefix_env.
            let (name, value) = assignments.into_iter().last().unwrap();
            Ok(Statement::Assignment(Assignment {
                name,
                value: Expression::Literal(Literal::String(value)),
            }))
        }
    }

    pub(crate) fn parse_assignment_value(&mut self) -> Result<String> {
        match self.peek() {
            // Empty value: FOO= (followed by space/semicolon/newline/end)
            None
            | Some(Token::Semicolon)
            | Some(Token::Newline)
            | Some(Token::CrLf)
            | Some(Token::Pipe)
            | Some(Token::And)
            | Some(Token::Or)
            | Some(Token::Ampersand) => Ok(String::new()),
            // Check if next token is another assignment (FOO= BAR=baz)
            Some(Token::Identifier(_)) => {
                // Could be: FOO=value or FOO= BAR=...
                // If the identifier is followed by =, this is an empty assignment value
                // and the identifier starts the next assignment
                if self.tokens.get(self.position + 1) == Some(&Token::Equals) {
                    // Check if it's a valid variable name (for the next assignment)
                    if let Some(Token::Identifier(name)) = self.tokens.get(self.position) {
                        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                            // This is the start of the next assignment, current value is empty
                            return Ok(String::new());
                        }
                    }
                }
                // Otherwise, consume as value
                match self.advance() {
                    Some(Token::Identifier(s)) => Ok(s.clone()),
                    _ => unreachable!(),
                }
            }
            Some(Token::String(_)) => {
                match self.advance() {
                    Some(Token::String(s)) => {
                        let unquoted = Self::strip_outer_quotes(s, '"');
                        Ok(Self::process_double_quote_escapes(&unquoted))
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::SingleQuotedString(_)) => {
                match self.advance() {
                    Some(Token::SingleQuotedString(s)) => {
                        Ok(Self::strip_outer_quotes(s, '\''))
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::AnsiCString(_)) => {
                match self.advance() {
                    Some(Token::AnsiCString(s)) => {
                        // Already processed by lexer, return as-is
                        Ok(s.clone())
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::Integer(_)) => {
                match self.advance() {
                    Some(Token::Integer(n)) => Ok(n.to_string()),
                    _ => unreachable!(),
                }
            }
            Some(Token::Variable(_)) | Some(Token::SpecialVariable(_)) => {
                match self.advance() {
                    Some(Token::Variable(s)) | Some(Token::SpecialVariable(s)) => {
                        // Keep the $ prefix -- the executor will expand it
                        Ok(s.clone())
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::Path(_)) => {
                match self.advance() {
                    Some(Token::Path(s)) => Ok(s.clone()),
                    _ => unreachable!(),
                }
            }
            Some(Token::CommandSubstitution(_)) | Some(Token::BacktickSubstitution(_)) => {
                match self.advance() {
                    Some(Token::CommandSubstitution(s)) | Some(Token::BacktickSubstitution(s)) => {
                        Ok(s.clone())
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::BracedVariable(_)) => {
                match self.advance() {
                    Some(Token::BracedVariable(s)) => Ok(s.clone()),
                    _ => unreachable!(),
                }
            }
            Some(Token::Float(_)) => {
                match self.advance() {
                    Some(Token::Float(f)) => Ok(f.to_string()),
                    _ => unreachable!(),
                }
            }
            Some(Token::Tilde) => {
                self.advance();
                Ok("~".to_string())
            }
            Some(Token::Dash) => {
                self.advance();
                Ok("-".to_string())
            }
            Some(Token::DoubleDash) => {
                self.advance();
                Ok("--".to_string())
            }
            Some(Token::ShortFlag(_)) => {
                match self.advance() {
                    Some(Token::ShortFlag(s)) => Ok(s.clone()),
                    _ => unreachable!(),
                }
            }
            Some(Token::Dot) => {
                self.advance();
                Ok(".".to_string())
            }
            _ => Ok(String::new()),
        }
    }
}
