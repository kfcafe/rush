//! Visual error formatter for Rush shell
//!
//! This module provides rich, contextual error messages with:
//! - Source code highlighting with line/column markers
//! - Command execution context display
//! - Visual formatting with ANSI colors
//! - Stack traces for nested command/function calls
//! - Common typo suggestions

use crate::error::{help_db, CommandContext, RushError, SourceLocation};

/// ANSI color codes for terminal output
pub mod ansi {
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const GREEN: &str = "\x1b[32m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RESET: &str = "\x1b[0m";
}

/// Error formatter that produces visually rich error messages
pub struct ErrorFormatter;

impl ErrorFormatter {
    /// Format an error with all available context
    pub fn format_error(error: &RushError) -> String {
        let mut output = String::new();

        // Error header with type
        output.push_str(&Self::format_error_header(error));

        // Source location with line/column markers
        if let Some(location) = &error.location {
            output.push('\n');
            output.push_str(&Self::format_source_location(location));
        }

        // Command context
        if let Some(cmd_ctx) = &error.command_context {
            output.push('\n');
            output.push_str(&Self::format_command_context(cmd_ctx));
        }

        // Additional context
        if let Some(context) = &error.context {
            output.push('\n');
            output.push_str(&Self::format_additional_context(context));
        }

        // Help text from database
        if let Some(help) = error.get_help() {
            output.push('\n');
            output.push_str(&Self::format_help(help));
        }

        output
    }

    /// Format the error header with type and message
    fn format_error_header(error: &RushError) -> String {
        let error_type = Self::classify_error(&error.error_code);
        let color = match error_type {
            ErrorType::Syntax => ansi::RED,
            ErrorType::Runtime => ansi::RED,
            ErrorType::Logic => ansi::YELLOW,
            ErrorType::Info => ansi::BLUE,
        };

        format!(
            "{}{}error{} [{}]: {}",
            color,
            ansi::BOLD,
            ansi::RESET,
            error.error_code,
            error.message
        )
    }

    /// Format source location with line/column visual markers
    fn format_source_location(location: &SourceLocation) -> String {
        let mut output = String::new();

        // Location header
        let filename = location.filename.as_deref().unwrap_or("<stdin>");
        output.push_str(&format!(
            "{}{}:{}:{}{}",
            ansi::DIM,
            filename,
            location.line,
            location.column,
            ansi::RESET
        ));

        // If we have line content, display it with markers
        if let Some(content) = &location.line_content {
            output.push('\n');
            output.push_str(&format!("  {}|{} {}\n", ansi::DIM, ansi::RESET, content));

            // Column marker with arrow
            let marker_pos = location.column.saturating_sub(1);
            output.push_str(&format!(
                "  {}|{} {}{}^{}{}{}",
                ansi::DIM,
                ansi::RESET,
                " ".repeat(marker_pos),
                ansi::RED,
                ansi::BOLD,
                " Error here",
                ansi::RESET
            ));
        }

        output
    }

    /// Format command execution context
    fn format_command_context(ctx: &CommandContext) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "{}Context:{} {}{}{}",
            ansi::BOLD,
            ansi::RESET,
            ansi::DIM,
            ctx.command_name,
            ansi::RESET
        ));

        // Display arguments if available
        if let Some(args) = &ctx.args {
            if !args.is_empty() {
                output.push_str(&format!(
                    " {}[{}{}{}]",
                    ansi::DIM,
                    ansi::RESET,
                    args.join(", "),
                    ansi::DIM
                ));
            }
        }

        // Display function call stack if available
        if let Some(stack) = &ctx.function_stack {
            if !stack.is_empty() {
                output.push_str(&format!(
                    "\n  {}Stack:{} {}",
                    ansi::BOLD,
                    ansi::RESET,
                    ansi::DIM
                ));
                for (i, func) in stack.iter().enumerate() {
                    if i > 0 {
                        output.push_str(" → ");
                    }
                    output.push_str(func);
                }
                output.push_str(ansi::RESET);
            }
        }

        output
    }

    /// Format additional context information
    fn format_additional_context(context: &serde_json::Value) -> String {
        let mut output = String::new();
        output.push_str(&format!("{}Additional Info:{}", ansi::BOLD, ansi::RESET));

        match context {
            serde_json::Value::Object(map) => {
                for (key, value) in map.iter() {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        _ => value.to_string(),
                    };
                    output.push_str(&format!(
                        "\n  {}{}{}: {}",
                        ansi::DIM,
                        key,
                        ansi::RESET,
                        value_str
                    ));
                }
            }
            _ => {
                output.push_str(&format!("\n  {}{}", ansi::DIM, context));
            }
        }

        output.push_str(ansi::RESET);
        output
    }

    /// Format help text for an error
    fn format_help(help_entry: &help_db::HelpEntry) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "{}{}Help: {}{}",
            ansi::BOLD,
            ansi::BLUE,
            help_entry.title,
            ansi::RESET
        ));
        output.push_str(&format!(
            "\n{}{}What to do:{}",
            ansi::BOLD,
            ansi::RESET,
            ansi::DIM
        ));
        output.push('\n');
        output.push_str(help_entry.fix);
        output.push_str(ansi::RESET);
        output
    }

    /// Classify error type from error code
    fn classify_error(error_code: &str) -> ErrorType {
        match error_code {
            code if code.contains("SYNTAX") || code.contains("PARSE") => ErrorType::Syntax,
            code if code.contains("EXECUTION") || code.contains("RUNTIME") => ErrorType::Runtime,
            code if code.contains("LOGIC") || code.contains("ASSERTION") => ErrorType::Logic,
            _ => ErrorType::Info,
        }
    }

    /// Format error as simple text (for non-TTY output)
    pub fn format_plain(error: &RushError) -> String {
        let mut output = String::new();
        output.push_str(&format!("error [{}]: {}", error.error_code, error.message));

        if let Some(location) = &error.location {
            let filename = location.filename.as_deref().unwrap_or("<stdin>");
            output.push_str(&format!(
                "\n  at {}:{}:{}",
                filename, location.line, location.column
            ));
            if let Some(content) = &location.line_content {
                output.push_str(&format!("\n    {}", content));
            }
        }

        if let Some(cmd_ctx) = &error.command_context {
            output.push_str(&format!("\n  while executing: {}", cmd_ctx.command_name));
            if let Some(args) = &cmd_ctx.args {
                if !args.is_empty() {
                    output.push_str(&format!(" [{}]", args.join(", ")));
                }
            }
            if let Some(stack) = &cmd_ctx.function_stack {
                if !stack.is_empty() {
                    output.push_str(&format!("\n  in functions: {}", stack.join(" → ")));
                }
            }
        }

        output
    }
}

/// Classifies the type of error for visual formatting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorType {
    /// Syntax or parse errors
    Syntax,
    /// Runtime execution errors
    Runtime,
    /// Logic errors (failed assertions)
    Logic,
    /// Informational messages
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SourceLocation;

    #[test]
    fn test_format_error_header() {
        let error = RushError::new("SYNTAX_ERROR", "Expected identifier", 1);
        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("SYNTAX_ERROR"));
        assert!(formatted.contains("Expected identifier"));
    }

    #[test]
    fn test_format_with_source_location() {
        let location = SourceLocation::new(5, 10)
            .with_line_content("let x = invalid".to_string())
            .with_filename("test.rush".to_string());
        let error = RushError::new("PARSE_ERROR", "Invalid syntax", 1).with_location(location);

        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("test.rush:5:10"));
        assert!(formatted.contains("let x = invalid"));
        assert!(formatted.contains("^"));
    }

    #[test]
    fn test_format_with_command_context() {
        let ctx =
            CommandContext::new("echo").with_args(vec!["hello".to_string(), "world".to_string()]);
        let error = RushError::new("EXECUTION_ERROR", "Failed", 1).with_command_context(ctx);

        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("Context:"));
        assert!(formatted.contains("echo"));
        assert!(formatted.contains("hello"));
    }

    #[test]
    fn test_format_with_function_stack() {
        let ctx = CommandContext::new("cmd")
            .with_function_stack(vec!["main".to_string(), "helper".to_string()]);
        let error = RushError::new("RUNTIME_ERROR", "Error", 1).with_command_context(ctx);

        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("Stack:"));
        assert!(formatted.contains("main"));
        assert!(formatted.contains("helper"));
    }

    #[test]
    fn test_format_with_additional_context() {
        let error = RushError::new("ERROR", "Test", 1).with_context(serde_json::json!({
            "file": "/tmp/test.txt",
            "reason": "Permission denied"
        }));

        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("Additional Info:"));
        assert!(formatted.contains("file"));
        assert!(formatted.contains("/tmp/test.txt"));
    }

    #[test]
    fn test_format_plain_text() {
        let location = SourceLocation::new(3, 5)
            .with_line_content("invalid code".to_string())
            .with_filename("script.rush".to_string());
        let error = RushError::new("SYNTAX_ERROR", "Bad syntax", 1).with_location(location);

        let formatted = ErrorFormatter::format_plain(&error);
        assert!(formatted.contains("error [SYNTAX_ERROR]"));
        assert!(formatted.contains("script.rush:3:5"));
        assert!(!formatted.contains("\x1b[")); // No ANSI codes
    }

    #[test]
    fn test_error_type_classification() {
        assert_eq!(
            ErrorFormatter::classify_error("SYNTAX_ERROR"),
            ErrorType::Syntax
        );
        assert_eq!(
            ErrorFormatter::classify_error("PARSE_ERROR"),
            ErrorType::Syntax
        );
        assert_eq!(
            ErrorFormatter::classify_error("EXECUTION_ERROR"),
            ErrorType::Runtime
        );
        assert_eq!(
            ErrorFormatter::classify_error("RUNTIME_ERROR"),
            ErrorType::Runtime
        );
        assert_eq!(
            ErrorFormatter::classify_error("LOGIC_ERROR"),
            ErrorType::Logic
        );
        assert_eq!(
            ErrorFormatter::classify_error("FILE_NOT_FOUND"),
            ErrorType::Info
        );
    }

    #[test]
    fn test_ansi_codes_present_in_formatted() {
        let error = RushError::new("ERROR", "Test error", 1);
        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("\x1b[")); // Contains ANSI escape codes
    }

    #[test]
    fn test_plain_format_no_ansi_codes() {
        let error = RushError::new("ERROR", "Test error", 1);
        let formatted = ErrorFormatter::format_plain(&error);
        assert!(!formatted.contains("\x1b[")); // No ANSI escape codes
    }

    #[test]
    fn test_complete_error_with_all_context() {
        let location = SourceLocation::new(10, 15)
            .with_line_content("undefined_var=$unknown".to_string())
            .with_filename("main.rush".to_string());

        let ctx = CommandContext::new("assignment")
            .with_args(vec!["undefined_var".to_string()])
            .with_function_stack(vec!["setup".to_string(), "init".to_string()]);

        let error = RushError::new("RUNTIME_ERROR", "Variable not found", 1)
            .with_location(location)
            .with_command_context(ctx)
            .with_context(serde_json::json!({
                "variable": "unknown",
                "suggestion": "Did you mean '$PATH'?"
            }));

        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("main.rush:10:15"));
        assert!(formatted.contains("undefined_var=$unknown"));
        assert!(formatted.contains("assignment"));
        assert!(formatted.contains("setup"));
        assert!(formatted.contains("variable"));
        assert!(formatted.contains("suggestion"));
    }

    #[test]
    fn test_format_error_with_help_text() {
        let error = RushError::new(
            "FILE_NOT_FOUND",
            "missing.txt: No such file or directory",
            1,
        );
        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("FILE_NOT_FOUND"));
        assert!(formatted.contains("missing.txt: No such file or directory"));
        assert!(formatted.contains("Help:"));
        assert!(formatted.contains("File or directory not found"));
    }

    #[test]
    fn test_format_error_without_help_text() {
        let error = RushError::new("CUSTOM_ERROR", "Custom error message", 1);
        let formatted = ErrorFormatter::format_error(&error);
        assert!(formatted.contains("CUSTOM_ERROR"));
        assert!(formatted.contains("Custom error message"));
        // Should not contain help since CUSTOM_ERROR is not in the database
        assert!(!formatted.contains("Help:"));
    }
}
