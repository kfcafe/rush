//! Syntax highlighting for the Rush interactive prompt.
//!
//! Implements `reedline::Highlighter` to color command line input live:
//! - Commands: green if found (builtin or on PATH), red if not found
//! - Strings: yellow
//! - Variables: cyan
//! - Keywords: bold blue
//! - Operators/pipes: light cyan
//! - Flags: dark gray
//! - Numbers: magenta
//! - Comments: dark gray

use std::sync::Arc;

use logos::Logos;
use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

use crate::builtins::Builtins;
use crate::lexer::Token;

pub struct RushHighlighter {
    builtins: Arc<Builtins>,
}

impl RushHighlighter {
    pub fn new(builtins: Arc<Builtins>) -> Self {
        Self { builtins }
    }

    /// Check whether `name` is an executable reachable on PATH.
    fn exists_in_path(name: &str) -> bool {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                let full = format!("{}/{}", dir, name);
                if std::path::Path::new(&full).exists() {
                    return true;
                }
            }
        }
        false
    }

    /// Emit any whitespace gap between `pos` and `span.start` as unstyled text,
    /// but detect inline comments (`# ...`) and render them as dark gray.
    fn push_gap(styled: &mut StyledText, line: &str, pos: usize, end: usize) {
        if pos >= end {
            return;
        }
        let gap = &line[pos..end];
        // If the gap contains a `#` (after leading whitespace), everything from
        // the `#` onwards is a comment.
        if let Some(hash_offset) = gap.find('#') {
            if hash_offset > 0 {
                styled.push((Style::new(), gap[..hash_offset].to_string()));
            }
            styled.push((
                Style::new().fg(Color::DarkGray),
                gap[hash_offset..].to_string(),
            ));
        } else {
            styled.push((Style::new(), gap.to_string()));
        }
    }
}

impl Highlighter for RushHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();

        if line.is_empty() {
            return styled;
        }

        let mut lexer = Token::lexer(line);
        // Byte offset of the end of the last token we processed.
        let mut pos: usize = 0;
        // Whether the next identifier-like token is in command position.
        let mut command_position = true;

        while let Some(result) = lexer.next() {
            let span = lexer.span();

            // Fill any whitespace gap (or inline comment) before this token.
            Self::push_gap(&mut styled, line, pos, span.start);
            pos = span.end;

            let token_str = line[span.clone()].to_string();

            let style = match result {
                Ok(ref token) => match token {
                    // ── Strings ────────────────────────────────────────────
                    Token::String(_) | Token::SingleQuotedString(_) | Token::AnsiCString(_) => {
                        command_position = false;
                        Style::new().fg(Color::Yellow)
                    }

                    // ── Variables ──────────────────────────────────────────
                    Token::Variable(_) | Token::SpecialVariable(_) | Token::BracedVariable(_) => {
                        command_position = false;
                        Style::new().fg(Color::Cyan)
                    }

                    // ── Command / arithmetic substitution ──────────────────
                    Token::CommandSubstitution(_) | Token::BacktickSubstitution(_) => {
                        command_position = false;
                        Style::new().fg(Color::Cyan)
                    }

                    // ── Pipes ──────────────────────────────────────────────
                    Token::Pipe | Token::ParallelPipe | Token::PipeAsk => {
                        command_position = true;
                        Style::new().fg(Color::LightCyan)
                    }

                    // ── Logic operators ────────────────────────────────────
                    Token::And | Token::Or => {
                        command_position = true;
                        Style::new().fg(Color::LightCyan)
                    }

                    // ── Statement separators ───────────────────────────────
                    Token::Semicolon | Token::DoubleSemicolon | Token::Newline | Token::CrLf => {
                        command_position = true;
                        Style::new()
                    }

                    // ── Redirects ──────────────────────────────────────────
                    Token::GreaterThan
                    | Token::StdoutAppend
                    | Token::StderrRedirect
                    | Token::StdinRedirect
                    | Token::StderrToStdout
                    | Token::BothRedirect
                    | Token::HereDoc
                    | Token::HereDocStrip => {
                        // After a redirect the next word is a file/fd, not a command.
                        command_position = false;
                        Style::new().fg(Color::LightCyan)
                    }

                    // ── Flags ──────────────────────────────────────────────
                    Token::ShortFlag(_)
                    | Token::LongFlag(_)
                    | Token::PlusFlag(_)
                    | Token::Dash
                    | Token::DoubleDash => {
                        command_position = false;
                        Style::new().fg(Color::DarkGray)
                    }

                    // ── Numbers ────────────────────────────────────────────
                    Token::Integer(_) | Token::Float(_) => {
                        command_position = false;
                        Style::new().fg(Color::Magenta)
                    }

                    // ── Keywords ───────────────────────────────────────────
                    Token::If
                    | Token::Elif
                    | Token::Else
                    | Token::Then
                    | Token::Fi
                    | Token::For
                    | Token::In
                    | Token::While
                    | Token::Until
                    | Token::Do
                    | Token::Done
                    | Token::Case
                    | Token::Esac
                    | Token::Let
                    | Token::Fn
                    | Token::Match
                    | Token::Function => {
                        // Keywords that introduce a new command body reset command_position.
                        match token {
                            Token::Then | Token::Do | Token::Else => {
                                command_position = true;
                            }
                            Token::Fi | Token::Done | Token::Esac | Token::In => {
                                command_position = false;
                            }
                            _ => {
                                // if/while/for/case/let/fn/match/function —
                                // the next word is a condition or name, not a command.
                                command_position = false;
                            }
                        }
                        Style::new().fg(Color::Blue).bold()
                    }

                    // ── Identifiers (potentially command names) ────────────
                    Token::Identifier(name) => {
                        let style = if command_position {
                            if self.builtins.is_builtin(name) || Self::exists_in_path(name) {
                                Style::new().fg(Color::Green)
                            } else {
                                Style::new().fg(Color::Red)
                            }
                        } else {
                            Style::new()
                        };
                        command_position = false;
                        style
                    }

                    // ── Paths and globs ────────────────────────────────────
                    Token::Path(_) | Token::GlobPattern(_) | Token::Tilde => {
                        command_position = false;
                        Style::new()
                    }

                    // ── Grouping: { } ( ) ──────────────────────────────────
                    Token::LeftBrace | Token::LeftParen | Token::LeftBracket => {
                        command_position = true;
                        Style::new()
                    }
                    Token::RightBrace | Token::RightParen | Token::RightBracket => {
                        command_position = false;
                        Style::new()
                    }
                    // ── Negation ───────────────────────────────────────────
                    Token::Bang => {
                        command_position = true;
                        Style::new().fg(Color::LightCyan)
                    }

                    // ── Ampersand (background job) ─────────────────────────
                    Token::Ampersand => {
                        command_position = true;
                        Style::new().fg(Color::LightCyan)
                    }

                    // ── Here-doc body (synthetic token) ───────────────────
                    Token::HereDocBody(_) => {
                        command_position = false;
                        Style::new().fg(Color::Yellow)
                    }

                    // ── Everything else: operators, punctuation ────────────
                    _ => {
                        command_position = false;
                        Style::new()
                    }
                },

                // Unrecognised input — show in red so the user knows it's invalid.
                Err(_) => {
                    command_position = false;
                    Style::new().fg(Color::Red)
                }
            };

            styled.push((style, token_str));
        }

        // Render any trailing text (unterminated strings, trailing comments, etc.)
        if pos < line.len() {
            Self::push_gap(&mut styled, line, pos, line.len());
        }

        styled
    }
}
