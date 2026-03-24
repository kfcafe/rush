pub mod ast;

use crate::lexer::Token;
use anyhow::{anyhow, Result};
use ast::*;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            // Skip newlines between statements
            while self.match_token(&Token::Newline) || self.match_token(&Token::CrLf) {
                self.advance();
            }

            if self.is_at_end() {
                break;
            }

            statements.push(self.parse_conditional_statement()?);

            // Handle semicolon as statement separator
            if self.match_token(&Token::Semicolon) {
                self.advance();
            }
        }

        Ok(statements)
    }

    fn parse_conditional_statement(&mut self) -> Result<Statement> {
        let mut left = self.parse_statement()?;

        loop {
            if self.match_token(&Token::And) {
                self.advance();
                let right = self.parse_statement()?;
                left = Statement::ConditionalAnd(ConditionalAnd {
                    left: Box::new(left),
                    right: Box::new(right),
                });
            } else if self.match_token(&Token::Or) {
                self.advance();
                let right = self.parse_statement()?;
                left = Statement::ConditionalOr(ConditionalOr {
                    left: Box::new(left),
                    right: Box::new(right),
                });
            } else {
                break;
            }
        }

        // Check for background operator & at the end
        if self.match_token(&Token::Ampersand) {
            self.advance();
            left = Statement::BackgroundCommand(Box::new(left));
        }

        Ok(left)
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        // Check for keywords first
        match self.peek() {
            Some(Token::Let) => self.parse_assignment(),
            Some(Token::Fn) => self.parse_function_def(),
            Some(Token::Function) => self.parse_bash_function_def(),
            Some(Token::If) => self.parse_if_statement(),
            Some(Token::For) => self.parse_for_loop(),
            Some(Token::While) => self.parse_while_loop(),
            Some(Token::Until) => self.parse_until_loop(),
            Some(Token::Match) => self.parse_match_expression(),
            Some(Token::Case) => self.parse_case_statement(),
            Some(Token::LeftParen) => {
                // Route through parse_command_or_pipeline so subshells
                // can participate in pipelines: (echo hello) | (cat)
                self.parse_command_or_pipeline()
            }
            _ => {
                // Check for POSIX function definition: NAME() { ... }
                if self.is_posix_function_def() {
                    self.parse_posix_function_def()
                } else {
                    self.parse_command_or_pipeline()
                }
            }
        }
    }

    fn parse_command_or_pipeline(&mut self) -> Result<Statement> {
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
            // Build elements list supporting commands, subshells, compound commands, and structured ops
            let first_element = Self::statement_to_pipeline_element(first_statement)?;

            self.advance();
            let mut elements = vec![first_element];

            loop {
                // Try to parse a structured pipeline operator keyword first.
                // This must be checked before falling through to generic command parsing so that
                // identifiers like `where`, `sort`, `count` etc. are treated as operators, not commands.
                let elem = if let Some(op) = self.try_parse_structured_op()? {
                    PipelineElement::StructuredOp(op)
                } else {
                    let stmt = self.parse_pipeline_element()?;
                    Self::statement_to_pipeline_element(stmt)?
                };
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
                    PipelineElement::Subshell(_)
                    | PipelineElement::CompoundCommand(_)
                    | PipelineElement::StructuredOp(_) => None,
                })
                .collect();

            Statement::Pipeline(Pipeline {
                commands,
                elements,
                negated,
            })
        } else if negated {
            // Single command with negation - wrap in a Pipeline with negated=true
            let element = Self::statement_to_pipeline_element(first_statement)?;
            let commands = match &element {
                PipelineElement::Command(cmd) => vec![cmd.clone()],
                _ => vec![],
            };
            Statement::Pipeline(Pipeline {
                commands,
                elements: vec![element],
                negated: true,
            })
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

    /// Parse the prompt for a |? operator.
    /// Can be a quoted string, a word, or omitted (returns empty string).
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

    /// Check if the current token is a redirect token
    fn match_redirect_token(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::GreaterThan)
                | Some(Token::StdoutAppend)
                | Some(Token::StdinRedirect)
                | Some(Token::StderrRedirect)
                | Some(Token::StderrToStdout)
                | Some(Token::BothRedirect)
        )
    }

    /// Parse a single redirect token and its target
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

    /// Parse a brace group: { commands; }
    /// Executes in current shell context (unlike subshell which forks)
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

    /// Try to parse a structured pipeline operator at the current position.
    ///
    /// Returns `Some(op)` if the current token is a recognized structured-op keyword
    /// (`where`, `sort`, `select`, `count`, `first`, `last`, `uniq`) and the parse succeeds,
    /// or `None` if the current token is not a structured-op keyword.
    ///
    /// This is called inside the pipeline loop so that these keywords are consumed
    /// as native operators rather than dispatched to external commands.
    fn try_parse_structured_op(&mut self) -> Result<Option<StructuredOp>> {
        let keyword = match self.peek() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Ok(None),
        };

        match keyword.as_str() {
            "where" => {
                self.advance(); // consume "where"
                let field = self.expect_identifier("where: expected field name")?;
                let op = self.parse_compare_op()?;
                let value = self.parse_op_value()?;
                Ok(Some(StructuredOp::Where { field, op, value }))
            }
            "sort" => {
                self.advance(); // consume "sort"
                let mut reverse = false;
                if matches!(self.peek(), Some(Token::ShortFlag(f)) if f == "-r")
                    || matches!(self.peek(), Some(Token::LongFlag(f)) if f == "--reverse")
                {
                    self.advance();
                    reverse = true;
                }
                let field = match self.peek() {
                    Some(Token::Identifier(_)) => {
                        Some(self.expect_identifier("sort: expected field name")?)
                    }
                    _ => None,
                };
                Ok(Some(StructuredOp::Sort { field, reverse }))
            }
            "select" => {
                self.advance(); // consume "select"
                let mut fields = Vec::new();
                while let Some(Token::Identifier(_)) = self.peek() {
                    fields.push(self.expect_identifier("select: expected field name")?);
                }
                if fields.is_empty() {
                    return Err(anyhow!("select: at least one field name required"));
                }
                Ok(Some(StructuredOp::Select { fields }))
            }
            "count" => {
                self.advance();
                Ok(Some(StructuredOp::Count))
            }
            "first" => {
                self.advance();
                let n = self.parse_optional_count(1)?;
                Ok(Some(StructuredOp::First(n)))
            }
            "last" => {
                self.advance();
                let n = self.parse_optional_count(1)?;
                Ok(Some(StructuredOp::Last(n)))
            }
            "uniq" => {
                self.advance();
                let field = match self.peek() {
                    Some(Token::Identifier(_)) => {
                        Some(self.expect_identifier("uniq: expected field name")?)
                    }
                    _ => None,
                };
                Ok(Some(StructuredOp::Uniq { field }))
            }
            _ => Ok(None),
        }
    }

    /// Parse a comparison operator for `where` expressions.
    fn parse_compare_op(&mut self) -> Result<CompareOp> {
        match self.peek() {
            Some(Token::DoubleEquals) => {
                self.advance();
                Ok(CompareOp::Eq)
            }
            Some(Token::Equals) => {
                self.advance();
                // `=~` — `=` followed by `~` (Tilde) — is the regex/glob match operator
                if matches!(self.peek(), Some(Token::Tilde)) {
                    self.advance();
                    Ok(CompareOp::Match)
                } else {
                    Ok(CompareOp::Eq)
                }
            }
            Some(Token::NotEquals) => {
                self.advance();
                Ok(CompareOp::Ne)
            }
            Some(Token::Bang) => {
                // `!~` — Bang followed by Tilde
                self.advance();
                if matches!(self.peek(), Some(Token::Tilde)) {
                    self.advance();
                    Ok(CompareOp::NotMatch)
                } else {
                    Err(anyhow!("where: expected `!~` operator"))
                }
            }
            Some(Token::GreaterThanOrEqual) => {
                self.advance();
                Ok(CompareOp::Ge)
            }
            Some(Token::LessThanOrEqual) => {
                self.advance();
                Ok(CompareOp::Le)
            }
            Some(Token::GreaterThan) => {
                self.advance();
                Ok(CompareOp::Gt)
            }
            // `<` is StdinRedirect in the lexer — treat as less-than in operator position
            Some(Token::StdinRedirect) => {
                self.advance();
                Ok(CompareOp::Lt)
            }
            Some(Token::Identifier(s)) if s == "=~" => {
                self.advance();
                Ok(CompareOp::Match)
            }
            Some(Token::Identifier(s)) if s == "!~" => {
                self.advance();
                Ok(CompareOp::NotMatch)
            }
            other => Err(anyhow!(
                "where: expected comparison operator, got {:?}",
                other
            )),
        }
    }

    /// Parse a comparison value (identifier, string literal, or integer).
    fn parse_op_value(&mut self) -> Result<String> {
        match self.peek() {
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::SingleQuotedString(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::Integer(n)) => {
                let s = n.to_string();
                self.advance();
                Ok(s)
            }
            other => Err(anyhow!("where: expected value, got {:?}", other)),
        }
    }

    /// Parse an optional integer count (used by `first N` and `last N`).
    fn parse_optional_count(&mut self, default: usize) -> Result<usize> {
        if let Some(Token::Integer(n)) = self.peek() {
            let n = *n as usize;
            self.advance();
            Ok(n)
        } else {
            Ok(default)
        }
    }

    /// Consume and return an Identifier token, or return an error with `context`.
    fn expect_identifier(&mut self, context: &str) -> Result<String> {
        match self.peek() {
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            other => Err(anyhow!("{}: expected identifier, got {:?}", context, other)),
        }
    }

    /// Convert a parsed statement into a pipeline element
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

    /// Check if current position has a `NAME=VALUE` pattern (bare assignment).
    /// Returns true if we see Identifier followed by Equals at current position.
    fn is_bare_assignment(&self) -> bool {
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

    /// Parse bare assignment(s) like `FOO=bar` or `FOO=bar BAZ=qux cmd args`.
    /// If only assignments with no command following, returns Assignment statement(s).
    /// If assignments are followed by a command, returns a Command with prefix_env.
    fn parse_bare_assignment_or_command(&mut self) -> Result<Statement> {
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

    /// Parse the value part of a bare assignment (after the `=`).
    /// Returns the value as a string. Handles identifiers, strings, integers,
    /// variables, paths, or empty values.
    fn parse_assignment_value(&mut self) -> Result<String> {
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
            Some(Token::String(_)) => match self.advance() {
                Some(Token::String(s)) => {
                    let unquoted = Self::strip_outer_quotes(&s, '"');
                    Ok(Self::process_double_quote_escapes(&unquoted))
                }
                _ => unreachable!(),
            },
            Some(Token::SingleQuotedString(_)) => match self.advance() {
                Some(Token::SingleQuotedString(s)) => Ok(Self::strip_outer_quotes(&s, '\'')),
                _ => unreachable!(),
            },
            Some(Token::AnsiCString(_)) => {
                match self.advance() {
                    Some(Token::AnsiCString(s)) => {
                        // Already processed by lexer, return as-is
                        Ok(s.clone())
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::Integer(_)) => match self.advance() {
                Some(Token::Integer(n)) => Ok(n.to_string()),
                _ => unreachable!(),
            },
            Some(Token::Variable(_)) | Some(Token::SpecialVariable(_)) => {
                match self.advance() {
                    Some(Token::Variable(s)) | Some(Token::SpecialVariable(s)) => {
                        // Keep the $ prefix -- the executor will expand it
                        Ok(s.clone())
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::Path(_)) => match self.advance() {
                Some(Token::Path(s)) => Ok(s.clone()),
                _ => unreachable!(),
            },
            Some(Token::CommandSubstitution(_)) | Some(Token::BacktickSubstitution(_)) => {
                match self.advance() {
                    Some(Token::CommandSubstitution(s)) | Some(Token::BacktickSubstitution(s)) => {
                        Ok(s.clone())
                    }
                    _ => unreachable!(),
                }
            }
            Some(Token::BracedVariable(_)) => match self.advance() {
                Some(Token::BracedVariable(s)) => Ok(s.clone()),
                _ => unreachable!(),
            },
            Some(Token::Float(_)) => match self.advance() {
                Some(Token::Float(f)) => Ok(f.to_string()),
                _ => unreachable!(),
            },
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
            Some(Token::ShortFlag(_)) => match self.advance() {
                Some(Token::ShortFlag(s)) => Ok(s.clone()),
                _ => unreachable!(),
            },
            Some(Token::Dot) => {
                self.advance();
                Ok(".".to_string())
            }
            _ => Ok(String::new()),
        }
    }

    fn parse_command(&mut self) -> Result<Command> {
        let name = match self.advance() {
            Some(Token::Identifier(s)) | Some(Token::Path(s)) | Some(Token::GlobPattern(s)) => {
                s.clone()
            }
            Some(Token::LeftBracket) => "[".to_string(),
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

    fn parse_argument(&mut self) -> Result<Argument> {
        match self.advance() {
            Some(Token::String(s)) => {
                // Double-quoted string: remove outer quotes and process escape sequences
                let unquoted = Self::strip_outer_quotes(&s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                Ok(Argument::Literal(processed))
            }
            Some(Token::SingleQuotedString(s)) => {
                // Single-quoted string: remove outer quotes, keep content literal (no escape processing)
                let unquoted = Self::strip_outer_quotes(&s, '\'');
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
            Some(Token::ShortFlag(s)) | Some(Token::LongFlag(s)) | Some(Token::PlusFlag(s)) => {
                Ok(Argument::Flag(s.clone()))
            }
            Some(Token::Path(s)) => Ok(Argument::Path(s.clone())),
            Some(Token::Tilde) => Ok(Argument::Path("~".to_string())),
            Some(Token::Integer(n)) => Ok(Argument::Literal(n.to_string())),
            Some(Token::Dot) => Ok(Argument::Path(".".to_string())),
            Some(Token::RightBracket) => Ok(Argument::Literal("]".to_string())),
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
            // Shell keywords used as arguments (e.g., `echo done`, `echo if`) are valid words.
            // Keywords only have special meaning when they start a command or follow specific
            // constructs (e.g., `then`, `do`). In argument position they are plain strings.
            Some(Token::Done) => Ok(Argument::Literal("done".to_string())),
            Some(Token::Do) => Ok(Argument::Literal("do".to_string())),
            Some(Token::Then) => Ok(Argument::Literal("then".to_string())),
            Some(Token::Fi) => Ok(Argument::Literal("fi".to_string())),
            Some(Token::Elif) => Ok(Argument::Literal("elif".to_string())),
            Some(Token::Else) => Ok(Argument::Literal("else".to_string())),
            Some(Token::Esac) => Ok(Argument::Literal("esac".to_string())),
            Some(Token::If) => Ok(Argument::Literal("if".to_string())),
            Some(Token::For) => Ok(Argument::Literal("for".to_string())),
            Some(Token::While) => Ok(Argument::Literal("while".to_string())),
            Some(Token::Until) => Ok(Argument::Literal("until".to_string())),
            Some(Token::In) => Ok(Argument::Literal("in".to_string())),
            Some(Token::Case) => Ok(Argument::Literal("case".to_string())),
            Some(Token::Function) => Ok(Argument::Literal("function".to_string())),
            _ => Err(anyhow!("Expected argument")),
        }
    }

    fn parse_redirect_target(&mut self) -> Result<String> {
        match self.advance() {
            Some(Token::Path(s)) | Some(Token::Identifier(s)) => Ok(s.clone()),
            Some(Token::String(s)) => {
                let unquoted = Self::strip_outer_quotes(&s, '"');
                Ok(Self::process_double_quote_escapes(&unquoted))
            }
            Some(Token::SingleQuotedString(s)) => Ok(Self::strip_outer_quotes(&s, '\'')),
            Some(Token::AnsiCString(s)) => Ok(s.clone()),
            _ => Err(anyhow!("Expected redirect target")),
        }
    }

    fn parse_assignment(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Let)?;

        let name = match self.advance() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Err(anyhow!("Expected variable name")),
        };

        self.expect_token(&Token::Equals)?;

        let value = self.parse_expression()?;

        Ok(Statement::Assignment(Assignment { name, value }))
    }

    fn parse_expression(&mut self) -> Result<Expression> {
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

    fn parse_function_def(&mut self) -> Result<Statement> {
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

    /// Check if current position has POSIX function definition: NAME() { ... }
    /// Looks ahead for Identifier followed by LeftParen RightParen
    fn is_posix_function_def(&self) -> bool {
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

    /// Parse POSIX-style function definition: NAME() { body }
    fn parse_posix_function_def(&mut self) -> Result<Statement> {
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

    /// Parse bash-style function definition: function NAME { body } or function NAME() { body }
    fn parse_bash_function_def(&mut self) -> Result<Statement> {
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

    fn parse_block(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();

        while !self.match_token(&Token::RightBrace) && !self.is_at_end() {
            // Skip newlines and semicolons
            while matches!(
                self.peek(),
                Some(Token::Newline) | Some(Token::CrLf) | Some(Token::Semicolon)
            ) {
                self.advance();
            }

            if self.match_token(&Token::RightBrace) || self.is_at_end() {
                break;
            }

            statements.push(self.parse_conditional_statement()?);
        }

        Ok(statements)
    }

    fn parse_if_statement(&mut self) -> Result<Statement> {
        self.expect_token(&Token::If)?;

        // Parse condition commands until we hit 'then' or '{'
        // This determines shell-style vs Rust-style
        let mut condition_stmts = Vec::new();

        // Check if the next token is '{' (Rust-style: if expr { ... })
        // or if we need to parse commands until 'then' (shell-style)
        let is_shell_style = !self.match_token(&Token::LeftBrace) && {
            // Peek ahead: we need to parse the condition and check if 'then' follows
            // Shell-style if the condition is followed by 'then'
            true
        };

        if is_shell_style {
            // Parse condition statements until 'then'
            loop {
                // Skip newlines
                while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                    self.advance();
                }

                if matches!(self.peek(), Some(Token::Then)) {
                    break;
                }

                if self.is_at_end() {
                    return Err(anyhow!("Expected 'then' in if statement"));
                }

                condition_stmts.push(self.parse_conditional_statement()?);

                // Handle optional semicolons between condition statements
                if matches!(self.peek(), Some(Token::Semicolon)) {
                    self.advance();
                }
            }

            if condition_stmts.is_empty() {
                return Err(anyhow!("if statement must have a condition"));
            }

            self.expect_token(&Token::Then)?;

            // Skip newline after 'then'
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }

            // Parse then-block until elif/else/fi
            let then_block = self.parse_shell_if_body()?;

            // Parse elif clauses
            let mut elif_clauses = Vec::new();
            while matches!(self.peek(), Some(Token::Elif)) {
                self.advance(); // consume 'elif'

                // Parse elif condition until 'then'
                let mut elif_condition = Vec::new();
                loop {
                    while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                        self.advance();
                    }

                    if matches!(self.peek(), Some(Token::Then)) {
                        break;
                    }

                    if self.is_at_end() {
                        return Err(anyhow!("Expected 'then' after elif condition"));
                    }

                    elif_condition.push(self.parse_conditional_statement()?);

                    if matches!(self.peek(), Some(Token::Semicolon)) {
                        self.advance();
                    }
                }

                if elif_condition.is_empty() {
                    return Err(anyhow!("elif must have a condition"));
                }

                self.expect_token(&Token::Then)?;

                while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                    self.advance();
                }

                let elif_body = self.parse_shell_if_body()?;
                elif_clauses.push(ElifClause {
                    condition: elif_condition,
                    body: elif_body,
                });
            }

            // Parse optional else block
            let else_block = if matches!(self.peek(), Some(Token::Else)) {
                self.advance(); // consume 'else'

                while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                    self.advance();
                }

                let block = self.parse_shell_if_body()?;
                Some(block)
            } else {
                None
            };

            self.expect_token(&Token::Fi)?;

            Ok(Statement::IfStatement(IfStatement {
                condition: IfCondition::Commands(condition_stmts),
                then_block,
                elif_clauses,
                else_block,
            }))
        } else {
            // Rust-style: if expr { ... } else { ... }
            // We need to backtrack - actually parse expression first
            // Since we already checked it's not a LeftBrace and set is_shell_style=true,
            // this branch won't be reached. But for completeness, handle Rust-style here.
            let condition = self.parse_expression()?;

            self.expect_token(&Token::LeftBrace)?;
            let then_block = self.parse_block()?;
            self.expect_token(&Token::RightBrace)?;

            let else_block = if self.match_token(&Token::Else) {
                self.advance();
                self.expect_token(&Token::LeftBrace)?;
                let block = self.parse_block()?;
                self.expect_token(&Token::RightBrace)?;
                Some(block)
            } else {
                None
            };

            Ok(Statement::IfStatement(IfStatement {
                condition: IfCondition::Expression(condition),
                then_block,
                elif_clauses: Vec::new(),
                else_block,
            }))
        }
    }

    /// Parse the body of a shell-style if/elif/else block.
    /// Stops at elif, else, or fi.
    fn parse_shell_if_body(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();

        loop {
            // Skip newlines
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }

            // Stop at elif, else, or fi
            if matches!(
                self.peek(),
                Some(Token::Elif) | Some(Token::Else) | Some(Token::Fi)
            ) {
                break;
            }

            if self.is_at_end() {
                return Err(anyhow!("Expected 'fi' to close if statement"));
            }

            statements.push(self.parse_conditional_statement()?);

            // Handle semicolons between statements
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        Ok(statements)
    }

    fn parse_for_loop(&mut self) -> Result<Statement> {
        self.expect_token(&Token::For)?;

        let variable = match self.advance() {
            Some(Token::Identifier(s)) => s.clone(),
            _ => return Err(anyhow!("Expected variable name after 'for'")),
        };

        // Parse word list: `for VAR in WORDS; do BODY; done`
        // or `for VAR; do BODY; done` (iterate over positional params)
        // or `for VAR do BODY; done` (iterate over positional params)
        let words = if self.match_token(&Token::In) {
            self.advance(); // consume 'in'
            self.parse_for_word_list()?
        } else {
            // No 'in' clause: iterate over positional params (empty word list)
            vec![]
        };

        // Skip optional semicolons/newlines before 'do'
        while matches!(
            self.peek(),
            Some(Token::Semicolon) | Some(Token::Newline) | Some(Token::CrLf)
        ) {
            self.advance();
        }

        self.expect_token(&Token::Do)?;

        // Skip newlines after 'do'
        while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        // Parse body statements until 'done'
        let mut body = Vec::new();
        while !matches!(self.peek(), Some(Token::Done)) {
            if self.is_at_end() {
                return Err(anyhow!("Expected 'done' to close for loop"));
            }

            // Skip newlines in body
            if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
                continue;
            }

            body.push(self.parse_conditional_statement()?);

            // Handle optional semicolons between body statements
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        self.expect_token(&Token::Done)?;

        Ok(Statement::ForLoop(ForLoop {
            variable,
            words,
            body,
        }))
    }

    /// Parse the word list for a for loop (tokens between 'in' and ';'/newline/do).
    /// Each word becomes an Argument that will be individually expanded at execution time.
    fn parse_for_word_list(&mut self) -> Result<Vec<Argument>> {
        let mut words = Vec::new();

        while !self.is_at_end()
            && !self.match_token(&Token::Semicolon)
            && !self.match_token(&Token::Newline)
            && !self.match_token(&Token::CrLf)
            && !self.match_token(&Token::Do)
        {
            words.push(self.parse_argument()?);
        }

        Ok(words)
    }

    fn parse_while_loop(&mut self) -> Result<Statement> {
        self.expect_token(&Token::While)?;

        // Parse condition statements until 'do'
        let mut condition = Vec::new();
        while !matches!(self.peek(), Some(Token::Do)) {
            // Skip newlines in condition
            if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
                continue;
            }

            // Parse a statement in the condition
            condition.push(self.parse_statement()?);

            // Handle optional semicolons or newlines between condition statements
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        if condition.is_empty() {
            return Err(anyhow!("While loop must have a condition"));
        }

        self.expect_token(&Token::Do)?;

        // Skip newline after 'do'
        if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        // Parse body statements until 'done'
        let mut body = Vec::new();
        while !matches!(self.peek(), Some(Token::Done)) {
            // Skip newlines in body
            if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
                continue;
            }

            body.push(self.parse_statement()?);

            // Handle optional semicolons or newlines between body statements
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        self.expect_token(&Token::Done)?;

        Ok(Statement::WhileLoop(WhileLoop { condition, body }))
    }

    fn parse_until_loop(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Until)?;

        // Parse condition statements until 'do'
        let mut condition = Vec::new();
        while !matches!(self.peek(), Some(Token::Do)) {
            // Skip newlines in condition
            if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
                continue;
            }

            // Parse a statement in the condition
            condition.push(self.parse_statement()?);

            // Handle optional semicolons or newlines between condition statements
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        if condition.is_empty() {
            return Err(anyhow!("Until loop must have a condition"));
        }

        self.expect_token(&Token::Do)?;

        // Skip newline after 'do'
        if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        // Parse body statements until 'done'
        let mut body = Vec::new();
        while !matches!(self.peek(), Some(Token::Done)) {
            // Skip newlines in body
            if matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
                continue;
            }

            body.push(self.parse_statement()?);

            // Handle optional semicolons or newlines between body statements
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        self.expect_token(&Token::Done)?;

        Ok(Statement::UntilLoop(UntilLoop { condition, body }))
    }

    fn parse_match_expression(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Match)?;

        let value = self.parse_expression()?;

        self.expect_token(&Token::LeftBrace)?;

        let mut arms = Vec::new();
        while !self.match_token(&Token::RightBrace) && !self.is_at_end() {
            let pattern = self.parse_pattern()?;
            self.expect_token(&Token::FatArrow)?;
            self.expect_token(&Token::LeftBrace)?;
            let body = self.parse_block()?;
            self.expect_token(&Token::RightBrace)?;

            arms.push(MatchArm { pattern, body });

            if self.match_token(&Token::Comma) {
                self.advance();
            }
        }

        self.expect_token(&Token::RightBrace)?;

        Ok(Statement::MatchExpression(MatchExpression { value, arms }))
    }

    /// Parse a POSIX case statement: case WORD in PATTERN) BODY;; ... esac
    fn parse_case_statement(&mut self) -> Result<Statement> {
        self.expect_token(&Token::Case)?;

        // Parse the word to match against
        let word = self.parse_expression()?;

        // Skip optional newlines
        while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        // Expect 'in' keyword
        self.expect_token(&Token::In)?;

        // Skip optional newlines after 'in'
        while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
            self.advance();
        }

        let mut arms = Vec::new();

        // Parse case arms until 'esac'
        while !matches!(self.peek(), Some(Token::Esac)) && !self.is_at_end() {
            // Skip newlines between arms
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }

            if matches!(self.peek(), Some(Token::Esac)) {
                break;
            }

            // Skip optional leading '(' before pattern (POSIX allows it)
            if matches!(self.peek(), Some(Token::LeftParen)) {
                self.advance();
            }

            // Parse patterns separated by '|'
            let mut patterns = Vec::new();
            loop {
                let pattern = self.parse_case_pattern()?;
                patterns.push(pattern);

                // Check for '|' to separate multiple patterns
                if self.match_token(&Token::Pipe) {
                    self.advance();
                } else {
                    break;
                }
            }

            // Expect ')' after patterns
            self.expect_token(&Token::RightParen)?;

            // Skip optional newlines after ')'
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }

            // Parse body statements until ';;' or 'esac'
            let mut body = Vec::new();
            while !matches!(
                self.peek(),
                Some(Token::DoubleSemicolon) | Some(Token::Esac)
            ) && !self.is_at_end()
            {
                // Skip newlines in body
                while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                    self.advance();
                }

                if matches!(
                    self.peek(),
                    Some(Token::DoubleSemicolon) | Some(Token::Esac)
                ) {
                    break;
                }

                body.push(self.parse_conditional_statement()?);

                // Handle optional semicolons between body statements
                if matches!(self.peek(), Some(Token::Semicolon)) {
                    self.advance();
                }
            }

            arms.push(CaseArm { patterns, body });

            // Consume ';;' if present (last arm before esac may not have it)
            if matches!(self.peek(), Some(Token::DoubleSemicolon)) {
                self.advance();
            }

            // Skip newlines after ';;'
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }
        }

        self.expect_token(&Token::Esac)?;

        Ok(Statement::CaseStatement(CaseStatement { word, arms }))
    }

    /// Parse a single case pattern (handles identifiers, *, strings, variables, globs)
    fn parse_case_pattern(&mut self) -> Result<String> {
        match self.peek() {
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::GlobPattern(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::String(s)) => {
                let unquoted = Self::strip_outer_quotes(&s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                self.advance();
                Ok(processed)
            }
            Some(Token::SingleQuotedString(s)) => {
                let s = Self::strip_outer_quotes(&s, '\'');
                self.advance();
                Ok(s)
            }
            Some(Token::AnsiCString(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::Integer(n)) => {
                let s = n.to_string();
                self.advance();
                Ok(s)
            }
            Some(Token::Variable(v)) => {
                let v = v.clone();
                self.advance();
                Ok(v)
            }
            Some(Token::ShortFlag(f)) => {
                // Patterns like -e, -f etc.
                let f = f.clone();
                self.advance();
                Ok(f)
            }
            Some(Token::Path(p)) => {
                let p = p.clone();
                self.advance();
                Ok(p)
            }
            Some(Token::Dot) => {
                self.advance();
                Ok(".".to_string())
            }
            Some(Token::Dash) => {
                self.advance();
                Ok("-".to_string())
            }
            _ => Err(anyhow!("Expected case pattern, found {:?}", self.peek())),
        }
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        match self.advance() {
            Some(Token::Identifier(s)) => Ok(Pattern::Identifier(s.clone())),
            Some(Token::String(s)) => {
                let unquoted = Self::strip_outer_quotes(&s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                Ok(Pattern::Literal(Literal::String(processed)))
            }
            Some(Token::SingleQuotedString(s)) => Ok(Pattern::Literal(Literal::String(
                Self::strip_outer_quotes(&s, '\''),
            ))),
            Some(Token::AnsiCString(s)) => Ok(Pattern::Literal(Literal::String(s.clone()))),
            Some(Token::Integer(n)) => Ok(Pattern::Literal(Literal::Integer(*n))),
            _ => Ok(Pattern::Wildcard),
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

    // Helper methods
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            let token = &self.tokens[self.position];
            self.position += 1;
            Some(token)
        } else {
            None
        }
    }

    fn match_token(&self, expected: &Token) -> bool {
        if let Some(token) = self.peek() {
            std::mem::discriminant(token) == std::mem::discriminant(expected)
        } else {
            false
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<()> {
        if self.match_token(expected) {
            self.advance();
            Ok(())
        } else {
            Err(anyhow!("Expected {:?}, found {:?}", expected, self.peek()))
        }
    }

    fn parse_var_expansion(&self, braced_var: &str) -> Result<VarExpansion> {
        // Remove ${ and } from the string
        let inner = braced_var.trim_start_matches("${").trim_end_matches('}');

        // Check for different operators in order
        if let Some(pos) = inner.find(":-") {
            let (name, default) = inner.split_at(pos);
            let default = &default[2..]; // Skip :-
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::UseDefault(default.to_string()),
            });
        }

        if let Some(pos) = inner.find(":=") {
            let (name, default) = inner.split_at(pos);
            let default = &default[2..]; // Skip :=
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::AssignDefault(default.to_string()),
            });
        }

        if let Some(pos) = inner.find(":?") {
            let (name, error_msg) = inner.split_at(pos);
            let error_msg = &error_msg[2..]; // Skip :?
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::ErrorIfUnset(error_msg.to_string()),
            });
        }

        if let Some(pos) = inner.find("##") {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[2..]; // Skip ##
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveLongestPrefix(pattern.to_string()),
            });
        }

        if let Some(pos) = inner.find('#') {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[1..]; // Skip #
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveShortestPrefix(pattern.to_string()),
            });
        }

        if let Some(pos) = inner.find("%%") {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[2..]; // Skip %%
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveLongestSuffix(pattern.to_string()),
            });
        }

        if let Some(pos) = inner.find('%') {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[1..]; // Skip %
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveShortestSuffix(pattern.to_string()),
            });
        }

        // No operator, just simple expansion
        Ok(VarExpansion {
            name: inner.to_string(),
            operator: VarExpansionOp::Simple,
        })
    }

    /// Strip only the first and last character if they match the quote character.
    /// Unlike trim_matches, this only removes one quote from each end.
    fn strip_outer_quotes(s: &str, quote: char) -> String {
        if s.len() >= 2 && s.starts_with(quote) && s.ends_with(quote) {
            s[1..s.len() - 1].to_string()
        } else {
            s.to_string()
        }
    }

    /// Process escape sequences in double-quoted strings.
    /// Per POSIX, only `\"`, `\\`, `\$`, `\``, and `\<newline>` have special meaning.
    /// Other `\X` sequences preserve the backslash.
    fn process_double_quote_escapes(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    match next {
                        '"' | '\\' | '$' | '`' => {
                            // These characters are escaped - consume the backslash
                            result.push(next);
                            chars.next();
                        }
                        '\n' => {
                            // Line continuation - skip both backslash and newline
                            chars.next();
                        }
                        _ => {
                            // Per POSIX, backslash before other chars is preserved
                            result.push('\\');
                            // Don't consume next - it will be processed in next iteration
                        }
                    }
                } else {
                    // Trailing backslash - preserve it
                    result.push('\\');
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_simple_command() {
        let tokens = Lexer::tokenize("ls -la").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Command(cmd) => {
                assert_eq!(cmd.name, "ls");
                assert_eq!(cmd.args.len(), 1);
            }
            _ => panic!("Expected command"),
        }
    }

    #[test]
    fn test_parse_pipeline() {
        let tokens = Lexer::tokenize("ls | grep foo").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Pipeline(pipeline) => {
                assert_eq!(pipeline.commands.len(), 2);
            }
            _ => panic!("Expected pipeline"),
        }
    }

    #[test]
    fn test_parse_assignment() {
        let tokens = Lexer::tokenize("let x = 42").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Assignment(assignment) => {
                assert_eq!(assignment.name, "x");
            }
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_while_loop() {
        let tokens = Lexer::tokenize("while true; do echo hi; done").unwrap();
        println!("Tokens: {:?}", tokens);
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        match result {
            Ok(statements) => {
                println!("Parsed successfully: {:?}", statements);
                assert_eq!(statements.len(), 1);
                match &statements[0] {
                    Statement::WhileLoop(_) => {}
                    _ => panic!("Expected while loop"),
                }
            }
            Err(e) => {
                panic!("Parse error: {}", e);
            }
        }
    }

    #[test]
    fn test_parse_if_then_fi() {
        let tokens = Lexer::tokenize("if true; then echo yes; fi").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::IfStatement(if_stmt) => {
                assert!(matches!(&if_stmt.condition, IfCondition::Commands(_)));
                assert_eq!(if_stmt.then_block.len(), 1);
                assert!(if_stmt.elif_clauses.is_empty());
                assert!(if_stmt.else_block.is_none());
            }
            _ => panic!("Expected if statement"),
        }
    }

    #[test]
    fn test_parse_if_then_else_fi() {
        let tokens = Lexer::tokenize("if false; then echo yes; else echo no; fi").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::IfStatement(if_stmt) => {
                assert!(matches!(&if_stmt.condition, IfCondition::Commands(_)));
                assert_eq!(if_stmt.then_block.len(), 1);
                assert!(if_stmt.elif_clauses.is_empty());
                assert!(if_stmt.else_block.is_some());
            }
            _ => panic!("Expected if statement"),
        }
    }

    #[test]
    fn test_parse_if_elif_fi() {
        let tokens = Lexer::tokenize("if false; then echo 1; elif true; then echo 2; fi").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::IfStatement(if_stmt) => {
                assert!(matches!(&if_stmt.condition, IfCondition::Commands(_)));
                assert_eq!(if_stmt.then_block.len(), 1);
                assert_eq!(if_stmt.elif_clauses.len(), 1);
                assert!(if_stmt.else_block.is_none());
            }
            _ => panic!("Expected if statement"),
        }
    }

    #[test]
    fn test_parse_if_elif_else_fi() {
        let tokens =
            Lexer::tokenize("if false; then echo 1; elif false; then echo 2; else echo 3; fi")
                .unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::IfStatement(if_stmt) => {
                assert_eq!(if_stmt.elif_clauses.len(), 1);
                assert!(if_stmt.else_block.is_some());
            }
            _ => panic!("Expected if statement"),
        }
    }

    #[test]
    fn test_parse_nested_if() {
        let tokens = Lexer::tokenize("if true; then if true; then echo nested; fi; fi").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::IfStatement(if_stmt) => {
                assert_eq!(if_stmt.then_block.len(), 1);
                assert!(matches!(&if_stmt.then_block[0], Statement::IfStatement(_)));
            }
            _ => panic!("Expected if statement"),
        }
    }

    #[test]
    fn test_parse_bare_assignment() {
        let tokens = Lexer::tokenize("FOO=bar").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Assignment(assignment) => {
                assert_eq!(assignment.name, "FOO");
                match &assignment.value {
                    Expression::Literal(Literal::String(s)) => assert_eq!(s, "bar"),
                    _ => panic!("Expected string literal value"),
                }
            }
            _ => panic!("Expected assignment, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_bare_assignment_quoted() {
        let tokens = Lexer::tokenize(r#"FOO="hello world""#).unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Assignment(assignment) => {
                assert_eq!(assignment.name, "FOO");
                match &assignment.value {
                    Expression::Literal(Literal::String(s)) => assert_eq!(s, "hello world"),
                    _ => panic!("Expected string literal value"),
                }
            }
            _ => panic!("Expected assignment, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_assignment_with_command() {
        let tokens = Lexer::tokenize("FOO=bar echo hello").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Command(cmd) => {
                assert_eq!(cmd.name, "echo");
                assert_eq!(cmd.prefix_env, vec![("FOO".to_string(), "bar".to_string())]);
                assert_eq!(cmd.args.len(), 1);
            }
            _ => panic!("Expected command with prefix env, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_export_assignment() {
        let tokens = Lexer::tokenize("export FOO=bar").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Command(cmd) => {
                assert_eq!(cmd.name, "export");
                // The argument should be merged as "FOO=bar"
                assert_eq!(cmd.args.len(), 1);
                match &cmd.args[0] {
                    Argument::Literal(s) => assert_eq!(s, "FOO=bar"),
                    _ => panic!("Expected literal argument, got {:?}", cmd.args[0]),
                }
            }
            _ => panic!("Expected command, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_bare_assignment_integer() {
        let tokens = Lexer::tokenize("COUNT=42").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Assignment(assignment) => {
                assert_eq!(assignment.name, "COUNT");
                match &assignment.value {
                    Expression::Literal(Literal::String(s)) => assert_eq!(s, "42"),
                    _ => panic!("Expected string literal value"),
                }
            }
            _ => panic!("Expected assignment, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_bare_assignment_empty() {
        let tokens = Lexer::tokenize("FOO=").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Assignment(assignment) => {
                assert_eq!(assignment.name, "FOO");
                match &assignment.value {
                    Expression::Literal(Literal::String(s)) => assert_eq!(s, ""),
                    _ => panic!("Expected empty string literal value"),
                }
            }
            _ => panic!("Expected assignment, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_multiple_assignments_with_command() {
        let tokens = Lexer::tokenize("A=1 B=2 cmd").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Command(cmd) => {
                assert_eq!(cmd.name, "cmd");
                assert_eq!(cmd.prefix_env.len(), 2);
                assert_eq!(cmd.prefix_env[0], ("A".to_string(), "1".to_string()));
                assert_eq!(cmd.prefix_env[1], ("B".to_string(), "2".to_string()));
            }
            _ => panic!("Expected command with prefix env, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_while_loop_with_newlines() {
        let code = r#"
        i=0
        while test $i -lt 5; do
            echo $i
            i=$((i+1))
        done
    "#;
        let tokens = Lexer::tokenize(code).unwrap();
        println!("Tokens: {:?}", tokens);
        let mut parser = Parser::new(tokens);
        match parser.parse() {
            Ok(statements) => {
                println!("Parsed successfully!");
                for stmt in &statements {
                    println!("  {:?}", stmt);
                }
            }
            Err(e) => {
                panic!("Parse error: {}", e);
            }
        }
    }

    #[test]
    fn test_parse_for_loop() {
        let tokens = Lexer::tokenize("for x in a b c; do echo $x; done").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::ForLoop(for_loop) => {
                assert_eq!(for_loop.variable, "x");
                assert_eq!(for_loop.words.len(), 3);
                assert_eq!(for_loop.body.len(), 1);
            }
            _ => panic!("Expected ForLoop, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_for_loop_no_in_clause() {
        let tokens = Lexer::tokenize("for x; do echo $x; done").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::ForLoop(for_loop) => {
                assert_eq!(for_loop.variable, "x");
                assert!(for_loop.words.is_empty()); // no word list = positional params
                assert_eq!(for_loop.body.len(), 1);
            }
            _ => panic!("Expected ForLoop, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_nested_for_loop() {
        let tokens =
            Lexer::tokenize("for i in 1 2; do for j in a b; do echo $i $j; done; done").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::ForLoop(for_loop) => {
                assert_eq!(for_loop.variable, "i");
                assert_eq!(for_loop.words.len(), 2);
                assert_eq!(for_loop.body.len(), 1);
                // Body should contain another ForLoop
                match &for_loop.body[0] {
                    Statement::ForLoop(inner) => {
                        assert_eq!(inner.variable, "j");
                        assert_eq!(inner.words.len(), 2);
                    }
                    _ => panic!("Expected inner ForLoop"),
                }
            }
            _ => panic!("Expected ForLoop"),
        }
    }

    #[test]
    fn test_parse_posix_function_def() {
        let tokens = Lexer::tokenize("foo() { echo hello; }").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::FunctionDef(func) => {
                assert_eq!(func.name, "foo");
                assert!(func.params.is_empty());
                assert_eq!(func.body.len(), 1);
            }
            _ => panic!("Expected FunctionDef, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_posix_function_def_and_call() {
        let tokens = Lexer::tokenize("foo() { echo hello; }; foo").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 2);
        assert!(matches!(&statements[0], Statement::FunctionDef(_)));
        assert!(matches!(&statements[1], Statement::Command(_)));
    }

    #[test]
    fn test_parse_bash_function_keyword() {
        let tokens = Lexer::tokenize("function bar { echo hi; }").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::FunctionDef(func) => {
                assert_eq!(func.name, "bar");
                assert!(func.params.is_empty());
                assert_eq!(func.body.len(), 1);
            }
            _ => panic!("Expected FunctionDef, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_bash_function_keyword_with_parens() {
        let tokens = Lexer::tokenize("function baz() { echo hi; }").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::FunctionDef(func) => {
                assert_eq!(func.name, "baz");
                assert!(func.params.is_empty());
            }
            _ => panic!("Expected FunctionDef, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_function_with_multiple_statements() {
        let tokens = Lexer::tokenize("f() { echo one; echo two; }").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::FunctionDef(func) => {
                assert_eq!(func.name, "f");
                assert_eq!(func.body.len(), 2);
            }
            _ => panic!("Expected FunctionDef, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_case_basic() {
        let tokens = Lexer::tokenize("case $x in foo) echo matched;; esac").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::CaseStatement(case_stmt) => {
                assert_eq!(case_stmt.arms.len(), 1);
                assert_eq!(case_stmt.arms[0].patterns, vec!["foo"]);
                assert_eq!(case_stmt.arms[0].body.len(), 1);
            }
            _ => panic!("Expected CaseStatement, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_case_multiple_patterns() {
        let tokens = Lexer::tokenize("case $x in a|b) echo ab;; *) echo other;; esac").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::CaseStatement(case_stmt) => {
                assert_eq!(case_stmt.arms.len(), 2);
                assert_eq!(case_stmt.arms[0].patterns, vec!["a", "b"]);
                assert_eq!(case_stmt.arms[1].patterns, vec!["*"]);
            }
            _ => panic!("Expected CaseStatement, got {:?}", statements[0]),
        }
    }

    #[test]
    fn test_parse_case_with_variable_assignment() {
        let tokens = Lexer::tokenize("x=foo; case $x in foo) echo matched;; esac").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 2);
        match &statements[1] {
            Statement::CaseStatement(_) => {} // ok
            _ => panic!("Expected CaseStatement, got {:?}", statements[1]),
        }
    }

    #[test]
    fn test_process_double_quote_escapes() {
        // Test escaped quote
        assert_eq!(Parser::process_double_quote_escapes(r#"test\""#), "test\"");
        // Test escaped backslash
        assert_eq!(Parser::process_double_quote_escapes(r"test\\"), "test\\");
        // Test escaped dollar
        assert_eq!(Parser::process_double_quote_escapes(r"test\$"), "test$");
        // Test escaped backtick
        assert_eq!(Parser::process_double_quote_escapes(r"test\`"), "test`");
        // Test backslash before regular char (preserved)
        assert_eq!(Parser::process_double_quote_escapes(r"test\n"), "test\\n");
        // Test multiple escapes
        assert_eq!(
            Parser::process_double_quote_escapes(r#"\"hello\""#),
            "\"hello\""
        );
    }

    #[test]
    fn test_parse_escaped_quote_in_string() {
        let input = r#"echo "test\"""#;
        println!("Input: {:?}", input);
        let tokens = Lexer::tokenize(input).unwrap();
        println!("Tokens: {:?}", tokens);
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();
        println!("Statements: {:?}", statements);

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::Command(cmd) => {
                assert_eq!(cmd.name, "echo");
                assert_eq!(cmd.args.len(), 1, "Expected 1 arg, got {:?}", cmd.args);
                match &cmd.args[0] {
                    Argument::Literal(s) => {
                        assert_eq!(s, "test\"", "Expected 'test\"', got '{}'", s);
                    }
                    other => panic!("Expected Literal argument, got {:?}", other),
                }
            }
            other => panic!("Expected Command, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pipe_ask_simple() {
        let tokens = Lexer::tokenize(r#"echo hello |? "summarize""#).unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::PipeAsk(pipe_ask) => {
                assert_eq!(pipe_ask.prompt, "summarize");
                // The command should be "echo hello"
                match pipe_ask.command.as_ref() {
                    Statement::Command(cmd) => {
                        assert_eq!(cmd.name, "echo");
                    }
                    other => panic!("Expected Command inside PipeAsk, got {:?}", other),
                }
            }
            other => panic!("Expected PipeAsk, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pipe_ask_in_pipeline() {
        let tokens =
            Lexer::tokenize(r#"cat file.txt | grep error |? "explain these errors""#).unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::PipeAsk(pipe_ask) => {
                assert_eq!(pipe_ask.prompt, "explain these errors");
                // The command should be a pipeline
                match pipe_ask.command.as_ref() {
                    Statement::Pipeline(pipeline) => {
                        assert_eq!(pipeline.commands.len(), 2);
                        assert_eq!(pipeline.commands[0].name, "cat");
                        assert_eq!(pipeline.commands[1].name, "grep");
                    }
                    other => panic!("Expected Pipeline inside PipeAsk, got {:?}", other),
                }
            }
            other => panic!("Expected PipeAsk, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pipe_ask_no_prompt() {
        let tokens = Lexer::tokenize("echo hello |?").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::PipeAsk(pipe_ask) => {
                assert_eq!(pipe_ask.prompt, ""); // Empty prompt when omitted
            }
            other => panic!("Expected PipeAsk, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pipe_ask_unquoted_prompt() {
        let tokens = Lexer::tokenize("echo hello |? summarize").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::PipeAsk(pipe_ask) => {
                assert_eq!(pipe_ask.prompt, "summarize");
            }
            other => panic!("Expected PipeAsk, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pipe_ask_single_quoted_prompt() {
        let tokens = Lexer::tokenize("git diff |? 'write a commit message'").unwrap();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().unwrap();

        assert_eq!(statements.len(), 1);
        match &statements[0] {
            Statement::PipeAsk(pipe_ask) => {
                assert_eq!(pipe_ask.prompt, "write a commit message");
            }
            other => panic!("Expected PipeAsk, got {:?}", other),
        }
    }
}
