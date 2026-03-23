use super::*;

impl Parser {
    pub(crate) fn parse_if_statement(&mut self) -> Result<Statement> {
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

    fn parse_shell_if_body(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();

        loop {
            // Skip newlines
            while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                self.advance();
            }

            // Stop at elif, else, or fi
            if matches!(self.peek(), Some(Token::Elif) | Some(Token::Else) | Some(Token::Fi)) {
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

    pub(crate) fn parse_for_loop(&mut self) -> Result<Statement> {
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
        while matches!(self.peek(), Some(Token::Semicolon) | Some(Token::Newline) | Some(Token::CrLf)) {
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

    pub(crate) fn parse_while_loop(&mut self) -> Result<Statement> {
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

    pub(crate) fn parse_until_loop(&mut self) -> Result<Statement> {
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

    pub(crate) fn parse_match_expression(&mut self) -> Result<Statement> {
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

    pub(crate) fn parse_case_statement(&mut self) -> Result<Statement> {
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
            while !matches!(self.peek(), Some(Token::DoubleSemicolon) | Some(Token::Esac))
                && !self.is_at_end()
            {
                // Skip newlines in body
                while matches!(self.peek(), Some(Token::Newline) | Some(Token::CrLf)) {
                    self.advance();
                }

                if matches!(self.peek(), Some(Token::DoubleSemicolon) | Some(Token::Esac)) {
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
                let unquoted = Self::strip_outer_quotes(s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                self.advance();
                Ok(processed)
            }
            Some(Token::SingleQuotedString(s)) => {
                let s = Self::strip_outer_quotes(s, '\'');
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
                let unquoted = Self::strip_outer_quotes(s, '"');
                let processed = Self::process_double_quote_escapes(&unquoted);
                Ok(Pattern::Literal(Literal::String(processed)))
            }
            Some(Token::SingleQuotedString(s)) => {
                Ok(Pattern::Literal(Literal::String(Self::strip_outer_quotes(s, '\''))))
            }
            Some(Token::AnsiCString(s)) => {
                Ok(Pattern::Literal(Literal::String(s.clone())))
            }
            Some(Token::Integer(n)) => Ok(Pattern::Literal(Literal::Integer(*n))),
            _ => Ok(Pattern::Wildcard),
        }
    }
}
