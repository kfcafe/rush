use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    // Keywords
    #[token("let")]
    Let,

    #[token("if")]
    If,

    #[token("else")]
    Else,

    #[token("then")]
    Then,

    #[token("elif")]
    Elif,

    #[token("fi")]
    Fi,

    #[token("fn")]
    Fn,

    #[token("match")]
    Match,

    #[token("for")]
    For,

    #[token("in")]
    In,

    #[token("while")]
    While,

    #[token("do")]
    Do,

    #[token("done")]
    Done,

    #[token("until")]
    Until,

    #[token("function")]
    Function,

    #[token("case")]
    Case,

    #[token("esac")]
    Esac,

    // Operators and punctuation
    #[token("=")]
    Equals,

    #[token("==")]
    DoubleEquals,

    #[token("!=")]
    NotEquals,

    #[token(">=")]
    GreaterThanOrEqual,

    #[token("<=")]
    LessThanOrEqual,

    #[token(">")]
    GreaterThan,

    #[token("|||")]
    ParallelPipe,

    #[token("|?")]
    PipeAsk,

    #[token("|")]
    Pipe,

    #[token("&&")]
    And,

    #[token("&")]
    Ampersand,

    #[token("||")]
    Or,

    #[token("!")]
    Bang,

    #[token("(")]
    LeftParen,

    #[token(")")]
    RightParen,

    #[token("{")]
    LeftBrace,

    #[token("}")]
    RightBrace,

    #[token("[")]
    LeftBracket,

    #[token("]")]
    RightBracket,

    #[token(";;")]
    DoubleSemicolon,

    #[token(";")]
    Semicolon,

    #[token(":")]
    Colon,

    #[token(",")]
    Comma,

    #[token(".")]
    Dot,

    #[token("->")]
    Arrow,

    #[token("=>")]
    FatArrow,

    // String literals
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    String(String),

    #[regex(r"'([^'\\]|\\.)*'", |lex| lex.slice().to_string())]
    SingleQuotedString(String),

    // ANSI-C quoted strings $'...' - escape sequences are processed
    #[regex(r"\$'([^'\\]|\\.)*'", parse_ansi_c_string)]
    AnsiCString(String),

    // Numbers
    #[regex(r"-?[0-9]+", priority = 3, callback = |lex| lex.slice().parse().ok())]
    Integer(i64),

    #[regex(r"-?[0-9]+\.[0-9]+", priority = 4, callback = |lex| lex.slice().parse().ok())]
    Float(f64),

    // Glob patterns (*, ?, [...] wildcards in filename context)
    // Patterns with * or ? (e.g., *.rs, file?.txt, src/**/*.rs)
    #[regex(r"[a-zA-Z0-9_.\-/]*[*?][a-zA-Z0-9_.*?\-/\[\]]*", |lex| lex.slice().to_string())]
    // Bracket glob patterns (e.g., [abc].txt, file[0-9].txt)
    // Must have content after ] to distinguish from test builtin [ ]
    #[regex(r"[a-zA-Z0-9_.\-/]*\[[^\]]+\][a-zA-Z0-9_.*?\-/]+", |lex| lex.slice().to_string())]
    GlobPattern(String),

    // Flags — defined before Identifier so flag tokens win on same-length ties
    // (e.g., +e matches PlusFlag not Identifier, -g matches ShortFlag not Identifier)

    // Bare dash (used in cd - for previous directory)
    #[token("-")]
    Dash,

    // Double dash alone (end of options marker, e.g., set -- args)
    #[token("--")]
    DoubleDash,

    #[regex(r"-[a-zA-Z0-9]+", |lex| lex.slice().to_string())]
    ShortFlag(String),

    #[regex(r"--[a-zA-Z0-9][a-zA-Z0-9-]*(=[^\s|;&()<>]+)?", |lex| lex.slice().to_string())]
    LongFlag(String),

    // Plus flags (for unsetting shell options like +e, +u, +x)
    #[regex(r"\+[a-zA-Z0-9]+", |lex| lex.slice().to_string())]
    PlusFlag(String),

    // Identifiers and commands — catch-all for shell words.
    // In POSIX, a word is any sequence of non-metacharacter chars. This covers:
    //   filenames: README.md          paths: src/main.rs
    //   URLs: http://example.com      git refs: HEAD~1, HEAD^, HEAD^^
    //   npm scopes: @scope/pkg        compilers: g++, c++
    //   format args: +%Y-%m-%d        job IDs: %1
    //   mid-word: foo#bar, a,b        emails: user@host
    //   digit-start: 7z, 2to3         UUIDs: 9a6c...
    //
    // Priority notes (logos: longest match wins; same length → first defined):
    //   +e → PlusFlag (same length, PlusFlag defined first)
    //   +%Y → Identifier (PlusFlag can't match %, Identifier wins by length)
    //   42 → Integer (same length, Integer defined first)
    //   42abc → Identifier (5 chars beats Integer's 2)
    //   3.14 → Float (same length, Float defined first)
    #[regex(r"[a-zA-Z0-9_@+%^][a-zA-Z0-9_.\-:~@/+%^,#]*", |lex| lex.slice().to_string())]
    Identifier(String),

    // Command substitution - needs custom parsing for nested cases
    #[regex(r"\$\(", parse_command_substitution)]
    CommandSubstitution(String),

    // Backtick command substitution
    #[regex(r"`", parse_backtick_substitution)]
    BacktickSubstitution(String),

    // Braced variables - must come before Special and Regular variables
    #[regex(r"\$\{[^}]+\}", |lex| lex.slice().to_string())]
    BracedVariable(String),

    // Special variables ($?, $!, $$, $#, $@, $*, $0-9, $-, $_)
    // Includes both single and special multi-char patterns
    #[regex(r"\$[?!$#@*\-_0-9]", |lex| lex.slice().to_string())]
    SpecialVariable(String),

    // Regular variables (at least 2 chars after $, or single letter)
    // This ensures $_ is matched as SpecialVariable, not Variable
    #[regex(r"\$[a-zA-Z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Variable(String),

    // Standalone tilde (for tilde expansion: ~ expands to $HOME)
    #[token("~")]
    Tilde,

    // File paths and arguments
    #[regex(r"[.~/][^\s|;&(){}]+", |lex| lex.slice().to_string())]
    Path(String),

    // Redirects
    #[token(">>")]
    StdoutAppend,

    #[token("2>&1")]
    StderrToStdout,

    #[token("2>")]
    StderrRedirect,

    #[token("&>")]
    BothRedirect,

    // Here-document operators (must come before StdinRedirect for precedence)
    #[token("<<-")]
    HereDocStrip,

    #[token("<<")]
    HereDoc,

    #[token("<")]
    StdinRedirect,

    // Synthesized token: here-document body (not matched by lexer directly)
    HereDocBody(HereDocData),

    // Newline and EOF
    #[regex(r"\n")]
    Newline,

    #[token("\r\n")]
    CrLf,

    // Comments
    #[regex(r"#[^\n]*", logos::skip)]
    Comment,
}

/// Data for a synthesized here-document body token
#[derive(Debug, Clone, PartialEq)]
pub struct HereDocData {
    pub body: String,
    pub expand_vars: bool,
    pub strip_tabs: bool,
}

// Parse ANSI-C quoted string $'...' and process escape sequences
fn parse_ansi_c_string(lex: &mut logos::Lexer<Token>) -> Option<String> {
    let slice = lex.slice();
    // Remove $' prefix and ' suffix
    let content = &slice[2..slice.len() - 1];

    let mut result = String::new();
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('a') => result.push('\x07'),             // alert/bell
                Some('b') => result.push('\x08'),             // backspace
                Some('e') | Some('E') => result.push('\x1b'), // escape
                Some('f') => result.push('\x0c'),             // form feed
                Some('n') => result.push('\n'),               // newline
                Some('r') => result.push('\r'),               // carriage return
                Some('t') => result.push('\t'),               // horizontal tab
                Some('v') => result.push('\x0b'),             // vertical tab
                Some('\\') => result.push('\\'),              // backslash
                Some('\'') => result.push('\''),              // single quote
                Some('"') => result.push('"'),                // double quote
                Some('x') => {
                    // Hexadecimal: \xNN
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if !hex.is_empty() {
                        if let Ok(val) = u8::from_str_radix(&hex, 16) {
                            result.push(val as char);
                        }
                    }
                }
                Some('u') => {
                    // Unicode: \uNNNN
                    let mut hex = String::new();
                    for _ in 0..4 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if !hex.is_empty() {
                        if let Ok(val) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(val) {
                                result.push(ch);
                            }
                        }
                    }
                }
                Some('U') => {
                    // Unicode: \UNNNNNNNN (up to 8 hex digits)
                    let mut hex = String::new();
                    for _ in 0..8 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if !hex.is_empty() {
                        if let Ok(val) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(val) {
                                result.push(ch);
                            }
                        }
                    }
                }
                Some(c) if c.is_ascii_digit() => {
                    // Octal: \NNN (up to 3 octal digits)
                    let mut octal = String::new();
                    octal.push(c);
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c >= '0' && c <= '7' {
                                octal.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if let Ok(val) = u8::from_str_radix(&octal, 8) {
                        result.push(val as char);
                    }
                }
                Some(c) => {
                    // Unknown escape, keep as-is
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    Some(result)
}

// Custom parser for $(...) that handles nesting
fn parse_command_substitution(lex: &mut logos::Lexer<Token>) -> Option<String> {
    let start = lex.span().start;
    let input = lex.source();
    let mut depth = 1; // We've consumed "$(" so one open paren
    let mut pos = lex.span().end;

    while pos < input.len() && depth > 0 {
        let ch = input.as_bytes()[pos] as char;
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' => {
                // Skip single-quoted string
                pos += 1;
                while pos < input.len() && input.as_bytes()[pos] as char != '\'' {
                    pos += 1;
                }
            }
            '"' => {
                // Skip double-quoted string
                pos += 1;
                while pos < input.len() {
                    let c = input.as_bytes()[pos] as char;
                    if c == '"' {
                        break;
                    }
                    if c == '\\' {
                        pos += 1; // skip escaped char
                    }
                    pos += 1;
                }
            }
            _ => {}
        }
        pos += 1;
    }

    if depth == 0 {
        // Extract the command including the $() delimiters
        let result = input[start..pos].to_string();
        // Update the lexer position
        lex.bump(pos - lex.span().end);
        Some(result)
    } else {
        None
    }
}

// Custom parser for backtick command substitution
fn parse_backtick_substitution(lex: &mut logos::Lexer<Token>) -> Option<String> {
    let start = lex.span().start;
    let input = lex.source();
    let mut pos = lex.span().end;

    // Find matching backtick
    while pos < input.len() {
        let ch = input.as_bytes()[pos] as char;
        if ch == '`' {
            pos += 1;
            let result = input[start..pos].to_string();
            lex.bump(pos - lex.span().end);
            return Some(result);
        } else if ch == '\\' && pos + 1 < input.len() {
            // Skip escaped character
            pos += 2;
        } else {
            pos += 1;
        }
    }

    None // Unclosed backtick
}

pub struct Lexer<'a> {
    inner: logos::Lexer<'a, Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            inner: Token::lexer(input),
        }
    }

    pub fn tokenize(input: &str) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();
        let mut lexer = Token::lexer(input);

        while let Some(token_result) = lexer.next() {
            match token_result {
                Ok(token) => tokens.push(token),
                Err(_) => {
                    return Err(LexerError::InvalidToken {
                        position: lexer.span().start,
                        text: lexer.slice().to_string(),
                    });
                }
            }
        }

        // Post-process: resolve here-documents
        let tokens = Self::resolve_heredocs(tokens, input);

        Ok(tokens)
    }

    /// Post-process token stream to resolve here-documents.
    fn resolve_heredocs(tokens: Vec<Token>, source: &str) -> Vec<Token> {
        let lines: Vec<&str> = source.lines().collect();
        let mut result: Vec<Token> = Vec::with_capacity(tokens.len());

        let mut i = 0;
        while i < tokens.len() {
            let is_heredoc = matches!(tokens[i], Token::HereDoc);
            let is_heredoc_strip = matches!(tokens[i], Token::HereDocStrip);

            if !is_heredoc && !is_heredoc_strip {
                result.push(tokens[i].clone());
                i += 1;
                continue;
            }

            let strip_tabs = is_heredoc_strip;
            i += 1; // skip << or <<-

            // Collect the delimiter word from subsequent tokens.
            let (delimiter, expand_vars) = if i < tokens.len() {
                match &tokens[i] {
                    Token::Identifier(s) => (s.clone(), true),
                    Token::SingleQuotedString(s) => {
                        let d = s.trim_matches('\'').to_string();
                        (d, false)
                    }
                    Token::String(s) => {
                        let d = s.trim_matches('"').to_string();
                        (d, false)
                    }
                    _ => {
                        if is_heredoc {
                            result.push(Token::HereDoc);
                        } else {
                            result.push(Token::HereDocStrip);
                        }
                        continue;
                    }
                }
            } else {
                if is_heredoc {
                    result.push(Token::HereDoc);
                } else {
                    result.push(Token::HereDocStrip);
                }
                continue;
            };
            i += 1; // skip delimiter token

            // Find which source line the << token is on by counting newlines
            let mut newline_count = 0;
            for t in &result {
                if matches!(t, Token::Newline | Token::CrLf) {
                    newline_count += 1;
                }
            }

            let body_start = newline_count + 1;

            let mut body_lines: Vec<String> = Vec::new();
            let mut body_end_line = body_start;
            let mut found_delimiter = false;

            for line_idx in body_start..lines.len() {
                let line = lines[line_idx];
                let trimmed = if strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    line
                };

                if trimmed.trim() == delimiter {
                    body_end_line = line_idx;
                    found_delimiter = true;
                    break;
                }

                let output_line = if strip_tabs {
                    line.trim_start_matches('\t').to_string()
                } else {
                    line.to_string()
                };
                body_lines.push(output_line);
            }

            if !found_delimiter {
                body_lines.clear();
            }

            let body = if body_lines.is_empty() {
                String::new()
            } else {
                body_lines.join("\n") + "\n"
            };

            result.push(Token::HereDocBody(HereDocData {
                body,
                expand_vars,
                strip_tabs,
            }));

            let lines_to_skip = if found_delimiter {
                body_end_line - newline_count
            } else {
                0
            };

            let mut newlines_skipped = 0;
            while i < tokens.len() && newlines_skipped < lines_to_skip {
                if matches!(tokens[i], Token::Newline | Token::CrLf) {
                    newlines_skipped += 1;
                }
                i += 1;
            }
            // Also skip any remaining tokens on the delimiter line
            // (e.g., the Identifier("EOF") token itself)
            while i < tokens.len() && !matches!(tokens[i], Token::Newline | Token::CrLf) {
                i += 1;
            }
        }

        result
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, LexerError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|result| {
            result.map_err(|_| LexerError::InvalidToken {
                position: self.inner.span().start,
                text: self.inner.slice().to_string(),
            })
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Invalid token at position {position}: '{text}'")]
    InvalidToken { position: usize, text: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_command() {
        let tokens = Lexer::tokenize("ls -la /home").unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], Token::Identifier(_)));
        assert!(matches!(tokens[1], Token::ShortFlag(_)));
        assert!(matches!(tokens[2], Token::Path(_)));
    }

    #[test]
    fn test_pipeline() {
        let tokens = Lexer::tokenize("ls | grep foo").unwrap();
        assert!(tokens.contains(&Token::Pipe));
    }

    #[test]
    fn test_variable() {
        let tokens = Lexer::tokenize("echo $HOME").unwrap();
        assert!(matches!(tokens[1], Token::Variable(_)));
    }

    #[test]
    fn test_string_interpolation() {
        let tokens = Lexer::tokenize(r#"echo "hello world""#).unwrap();
        assert!(matches!(tokens[1], Token::String(_)));
    }

    #[test]
    fn test_let_statement() {
        let tokens = Lexer::tokenize("let x = 42").unwrap();
        assert_eq!(tokens[0], Token::Let);
        assert!(matches!(tokens[1], Token::Identifier(_)));
        assert_eq!(tokens[2], Token::Equals);
        assert!(matches!(tokens[3], Token::Integer(42)));
    }

    #[test]
    fn test_function_definition() {
        let tokens = Lexer::tokenize("fn deploy(env: String) {}").unwrap();
        assert_eq!(tokens[0], Token::Fn);
        assert!(matches!(tokens[1], Token::Identifier(_)));
    }

    #[test]
    fn test_command_substitution_simple() {
        let tokens = Lexer::tokenize("echo $(pwd)").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::CommandSubstitution(cmd) = &tokens[1] {
            assert_eq!(cmd, "$(pwd)");
        } else {
            panic!("Expected CommandSubstitution token");
        }
    }

    #[test]
    fn test_command_substitution_nested() {
        let tokens = Lexer::tokenize("echo $(echo $(pwd))").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::CommandSubstitution(cmd) = &tokens[1] {
            assert_eq!(cmd, "$(echo $(pwd))");
        } else {
            panic!("Expected CommandSubstitution token");
        }
    }

    #[test]
    fn test_backtick_substitution() {
        let tokens = Lexer::tokenize("echo `pwd`").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BacktickSubstitution(cmd) = &tokens[1] {
            assert_eq!(cmd, "`pwd`");
        } else {
            panic!("Expected BacktickSubstitution token");
        }
    }

    #[test]
    fn test_braced_variable_simple() {
        let tokens = Lexer::tokenize("echo ${VAR}").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BracedVariable(var) = &tokens[1] {
            assert_eq!(var, "${VAR}");
        } else {
            panic!("Expected BracedVariable token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_braced_variable_use_default() {
        let tokens = Lexer::tokenize("echo ${VAR:-default}").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BracedVariable(var) = &tokens[1] {
            assert_eq!(var, "${VAR:-default}");
        } else {
            panic!("Expected BracedVariable token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_braced_variable_assign_default() {
        let tokens = Lexer::tokenize("echo ${VAR:=default}").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BracedVariable(var) = &tokens[1] {
            assert_eq!(var, "${VAR:=default}");
        } else {
            panic!("Expected BracedVariable token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_braced_variable_error_if_unset() {
        let tokens = Lexer::tokenize("echo ${VAR:?error}").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BracedVariable(var) = &tokens[1] {
            assert_eq!(var, "${VAR:?error}");
        } else {
            panic!("Expected BracedVariable token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_braced_variable_prefix_removal() {
        let tokens = Lexer::tokenize("echo ${VAR#prefix}").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BracedVariable(var) = &tokens[1] {
            assert_eq!(var, "${VAR#prefix}");
        } else {
            panic!("Expected BracedVariable token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_braced_variable_suffix_removal() {
        let tokens = Lexer::tokenize("echo ${VAR%suffix}").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::BracedVariable(var) = &tokens[1] {
            assert_eq!(var, "${VAR%suffix}");
        } else {
            panic!("Expected BracedVariable token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_special_variable_shell_pid() {
        let tokens = Lexer::tokenize("$$").unwrap();
        assert_eq!(tokens.len(), 1, "Should have 1 token for $$");
        if let Token::SpecialVariable(var) = &tokens[0] {
            assert_eq!(var, "$$", "Should be $$ token");
        } else {
            panic!("Expected SpecialVariable token for $$, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_special_variable_last_bg_pid() {
        let tokens = Lexer::tokenize("$!").unwrap();
        assert_eq!(tokens.len(), 1);
        if let Token::SpecialVariable(var) = &tokens[0] {
            assert_eq!(var, "$!");
        } else {
            panic!("Expected SpecialVariable token for $!, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_special_variable_option_flags() {
        let tokens = Lexer::tokenize("$-").unwrap();
        assert_eq!(tokens.len(), 1);
        if let Token::SpecialVariable(var) = &tokens[0] {
            assert_eq!(var, "$-");
        } else {
            panic!("Expected SpecialVariable token for $-, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_special_variable_last_arg() {
        let tokens = Lexer::tokenize("$_").unwrap();
        assert_eq!(tokens.len(), 1);
        if let Token::SpecialVariable(var) = &tokens[0] {
            assert_eq!(var, "$_");
        } else {
            panic!("Expected SpecialVariable token for $_, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_while_keyword() {
        let tokens = Lexer::tokenize("while").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::While);
    }

    #[test]
    fn test_do_keyword() {
        let tokens = Lexer::tokenize("do").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Do);
    }

    #[test]
    fn test_done_keyword() {
        let tokens = Lexer::tokenize("done").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Done);
    }

    #[test]
    fn test_filename_with_dot() {
        let tokens = Lexer::tokenize("cat README.md").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Identifier(_)));
        assert!(matches!(tokens[1], Token::Identifier(ref s) if s == "README.md"));
    }

    #[test]
    fn test_filename_multiple_dots() {
        let tokens = Lexer::tokenize("echo file.tar.gz").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[1], Token::Identifier(ref s) if s == "file.tar.gz"));
    }

    #[test]
    fn test_dot_alone_is_path() {
        let tokens = Lexer::tokenize("echo .").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[1], Token::Dot);
    }

    #[test]
    fn test_dotdot_is_path() {
        let tokens = Lexer::tokenize("echo ..").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[1], Token::Path(ref s) if s == ".."));
    }

    #[test]
    fn test_dot_slash_path() {
        let tokens = Lexer::tokenize("./script.sh").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Path(ref s) if s == "./script.sh"));
    }

    #[test]
    fn test_tilde_standalone() {
        let tokens = Lexer::tokenize("echo ~").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[1], Token::Tilde);
    }

    #[test]
    fn test_tilde_with_path() {
        let tokens = Lexer::tokenize("cd ~/Documents").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[1], Token::Path(ref s) if s == "~/Documents"));
    }

    #[test]
    fn test_tilde_user() {
        let tokens = Lexer::tokenize("echo ~root").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[1], Token::Path(ref s) if s == "~root"));
    }

    #[test]
    fn test_arithmetic_expansion_simple() {
        let tokens = Lexer::tokenize("echo $((1+2))").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::CommandSubstitution(cmd) = &tokens[1] {
            assert_eq!(cmd, "$((1+2))");
        } else {
            panic!("Expected CommandSubstitution token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_arithmetic_expansion_with_spaces() {
        let tokens = Lexer::tokenize("echo $((5 != 3))").unwrap();
        assert_eq!(tokens.len(), 2);
        if let Token::CommandSubstitution(cmd) = &tokens[1] {
            assert_eq!(cmd, "$((5 != 3))");
        } else {
            panic!("Expected CommandSubstitution token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_escaped_quote_in_double_string() {
        // Test that escaped quotes inside double-quoted strings are handled
        let tokens = Lexer::tokenize(r#"echo "test\"end""#).unwrap();
        assert_eq!(tokens.len(), 2, "Expected 2 tokens, got {:?}", tokens);
        if let Token::String(s) = &tokens[1] {
            assert_eq!(
                s, r#""test\"end""#,
                "String token should contain the escaped quote"
            );
        } else {
            panic!("Expected String token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_escaped_quote_at_end_of_string() {
        // Test that escaped quote at end of string is handled correctly
        let tokens = Lexer::tokenize(r#"echo "test\"""#).unwrap();
        assert_eq!(tokens.len(), 2, "Expected 2 tokens, got {:?}", tokens);
        if let Token::String(s) = &tokens[1] {
            assert_eq!(
                s, r#""test\"""#,
                "String token should contain the escaped quote at end"
            );
        } else {
            panic!("Expected String token, got {:?}", tokens[1]);
        }
    }

    #[test]
    fn test_pipe_ask_token() {
        let tokens = Lexer::tokenize("echo hello |? \"summarize\"").unwrap();
        assert!(tokens.contains(&Token::PipeAsk));
    }

    #[test]
    fn test_digit_starting_word() {
        // UUIDs, hex strings, commands like 7z/2to3 must tokenize
        let tokens = Lexer::tokenize("echo 9a6c9c73-22be-4847-8e43-e8111b5b8836").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(
            matches!(tokens[1], Token::Identifier(ref s) if s == "9a6c9c73-22be-4847-8e43-e8111b5b8836")
        );
    }

    #[test]
    fn test_digit_starting_command() {
        let tokens = Lexer::tokenize("7z x archive.7z").unwrap();
        assert!(matches!(tokens[0], Token::Identifier(ref s) if s == "7z"));
    }

    #[test]
    fn test_pure_integer_still_integer() {
        let tokens = Lexer::tokenize("echo 42").unwrap();
        assert!(matches!(tokens[1], Token::Integer(42)));
    }

    #[test]
    fn test_pipe_ask_with_prompt() {
        let tokens = Lexer::tokenize("git diff |? \"write commit message\"").unwrap();
        // Should tokenize as: Identifier(git), Identifier(diff), PipeAsk, String
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0], Token::Identifier(ref s) if s == "git"));
        assert!(matches!(tokens[1], Token::Identifier(ref s) if s == "diff"));
        assert_eq!(tokens[2], Token::PipeAsk);
        assert!(matches!(tokens[3], Token::String(_)));
    }
}
