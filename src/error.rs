//! Structured error types for Rush shell
//!
//! This module provides typed error representations that can be formatted
//! as either human-readable text or structured JSON.

pub mod help_db;

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Source location information for errors
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceLocation {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Content of the line for error display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_content: Option<String>,
    /// Optional filename
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

impl SourceLocation {
    /// Create a new source location
    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            line_content: None,
            filename: None,
        }
    }

    /// Add line content for display
    pub fn with_line_content(mut self, content: String) -> Self {
        self.line_content = Some(content);
        self
    }

    /// Add filename
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }
}

/// Command context for execution errors
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandContext {
    /// Name of the command or builtin that failed
    pub command_name: String,
    /// Arguments passed to the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// Function call stack (for nested function calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_stack: Option<Vec<String>>,
}

impl CommandContext {
    /// Create a new command context
    pub fn new(command_name: impl Into<String>) -> Self {
        Self {
            command_name: command_name.into(),
            args: None,
            function_stack: None,
        }
    }

    /// Add arguments to context
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = Some(args);
        self
    }

    /// Add function call stack
    pub fn with_function_stack(mut self, stack: Vec<String>) -> Self {
        self.function_stack = Some(stack);
        self
    }
}

/// Structured error type for Rush shell operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RushError {
    /// Error code category
    pub error_code: String,
    /// Human-readable error message
    pub message: String,
    /// Exit code for the shell
    pub exit_code: i32,
    /// Additional context information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    /// Source location where error occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
    /// Command execution context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_context: Option<CommandContext>,
}

impl RushError {
    /// Create a new error with the given code, message, and exit code
    pub fn new(error_code: impl Into<String>, message: impl Into<String>, exit_code: i32) -> Self {
        Self {
            error_code: error_code.into(),
            message: message.into(),
            exit_code,
            context: None,
            location: None,
            command_context: None,
        }
    }

    /// Add context information to the error
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Add source location to the error
    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Add command execution context to the error
    pub fn with_command_context(mut self, command_context: CommandContext) -> Self {
        self.command_context = Some(command_context);
        self
    }

    /// File not found error
    pub fn file_not_found(path: &Path) -> Self {
        Self::new(
            "FILE_NOT_FOUND",
            format!("{}: No such file or directory", path.display()),
            1,
        )
    }

    /// Is a directory error (when a file was expected)
    pub fn is_a_directory(path: &Path) -> Self {
        Self::new(
            "IS_A_DIRECTORY",
            format!("{}: Is a directory", path.display()),
            1,
        )
    }

    /// Format error as JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                r#"{{"error_code":"{}","message":"{}","exit_code":{}}}"#,
                self.error_code, self.message, self.exit_code
            )
        })
    }

    /// Format error as human-readable text
    pub fn to_text(&self) -> String {
        self.message.clone()
    }

    /// Get help text for this error code, if available
    pub fn get_help(&self) -> Option<&'static help_db::HelpEntry> {
        help_db::get_help(&self.error_code)
    }

    /// Format error with help text appended
    pub fn with_help(&self) -> String {
        let mut output = self.to_text();
        if let Some(help) = self.get_help() {
            output.push_str("\n\n");
            output.push_str("Help: ");
            output.push_str(help.title);
            output.push('\n');
            output.push_str(help.fix);
        }
        output
    }
}

/// Check if errors should be output in JSON format
///
/// Currently checks the RUSH_ERROR_FORMAT environment variable.
/// Returns true if it's set to "json", false otherwise.
pub fn should_output_json_errors() -> bool {
    std::env::var("RUSH_ERROR_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_not_found() {
        let err = RushError::file_not_found(Path::new("/tmp/nonexistent"));
        assert_eq!(err.error_code, "FILE_NOT_FOUND");
        assert_eq!(err.exit_code, 1);
        assert!(err.message.contains("No such file or directory"));
    }

    #[test]
    fn test_is_a_directory() {
        let err = RushError::is_a_directory(Path::new("/tmp"));
        assert_eq!(err.error_code, "IS_A_DIRECTORY");
        assert_eq!(err.exit_code, 1);
        assert!(err.message.contains("Is a directory"));
    }

    #[test]
    fn test_to_json() {
        let err = RushError::new("TEST_ERROR", "Test message", 42);
        let json = err.to_json();
        assert!(json.contains("TEST_ERROR"));
        assert!(json.contains("Test message"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_to_text() {
        let err = RushError::new("TEST_ERROR", "Test message", 42);
        assert_eq!(err.to_text(), "Test message");
    }

    #[test]
    fn test_with_context() {
        let err = RushError::new("TEST_ERROR", "Test message", 1)
            .with_context(serde_json::json!({"file": "/tmp/test.txt"}));
        assert!(err.context.is_some());
    }

    #[test]
    fn test_should_output_json_errors() {
        // Default should be false
        std::env::remove_var("RUSH_ERROR_FORMAT");
        assert!(!should_output_json_errors());

        // Set to json
        std::env::set_var("RUSH_ERROR_FORMAT", "json");
        assert!(should_output_json_errors());

        // Set to JSON (uppercase)
        std::env::set_var("RUSH_ERROR_FORMAT", "JSON");
        assert!(should_output_json_errors());

        // Set to something else
        std::env::set_var("RUSH_ERROR_FORMAT", "text");
        assert!(!should_output_json_errors());

        // Clean up
        std::env::remove_var("RUSH_ERROR_FORMAT");
    }

    #[test]
    fn test_source_location() {
        let loc = SourceLocation::new(5, 10);
        assert_eq!(loc.line, 5);
        assert_eq!(loc.column, 10);
        assert_eq!(loc.line_content, None);
        assert_eq!(loc.filename, None);
    }

    #[test]
    fn test_source_location_with_content() {
        let loc = SourceLocation::new(5, 10)
            .with_line_content("let x = invalid".to_string())
            .with_filename("script.rush".to_string());
        assert_eq!(loc.line, 5);
        assert_eq!(loc.column, 10);
        assert_eq!(loc.line_content, Some("let x = invalid".to_string()));
        assert_eq!(loc.filename, Some("script.rush".to_string()));
    }

    #[test]
    fn test_command_context() {
        let ctx = CommandContext::new("echo");
        assert_eq!(ctx.command_name, "echo");
        assert_eq!(ctx.args, None);
        assert_eq!(ctx.function_stack, None);
    }

    #[test]
    fn test_command_context_with_args() {
        let ctx = CommandContext::new("ls").with_args(vec!["-la".to_string(), "/home".to_string()]);
        assert_eq!(ctx.command_name, "ls");
        assert_eq!(ctx.args, Some(vec!["-la".to_string(), "/home".to_string()]));
    }

    #[test]
    fn test_command_context_with_stack() {
        let stack = vec!["main".to_string(), "helper".to_string()];
        let ctx = CommandContext::new("builtin").with_function_stack(stack.clone());
        assert_eq!(ctx.function_stack, Some(stack));
    }

    #[test]
    fn test_error_with_location() {
        let loc = SourceLocation::new(5, 10).with_line_content("let x = invalid".to_string());
        let err = RushError::new("PARSE_ERROR", "Invalid syntax", 1).with_location(loc);
        assert!(err.location.is_some());
        assert_eq!(err.location.as_ref().unwrap().line, 5);
    }

    #[test]
    fn test_error_with_command_context() {
        let ctx = CommandContext::new("echo").with_args(vec!["hello".to_string()]);
        let err =
            RushError::new("EXECUTION_ERROR", "Failed to execute", 1).with_command_context(ctx);
        assert!(err.command_context.is_some());
        assert_eq!(err.command_context.as_ref().unwrap().command_name, "echo");
    }

    #[test]
    fn test_error_with_both_contexts() {
        let loc = SourceLocation::new(5, 10);
        let cmd_ctx = CommandContext::new("test");
        let err = RushError::new("ERROR", "Test error", 1)
            .with_location(loc)
            .with_command_context(cmd_ctx);
        assert!(err.location.is_some());
        assert!(err.command_context.is_some());
    }

    #[test]
    fn test_get_help_for_error() {
        let err = RushError::new("FILE_NOT_FOUND", "missing.txt: No such file", 1);
        let help = err.get_help();
        assert!(help.is_some());
        let entry = help.unwrap();
        assert_eq!(entry.title, "File or directory not found");
    }

    #[test]
    fn test_get_help_for_nonexistent_error() {
        let err = RushError::new("CUSTOM_ERROR", "Custom message", 1);
        let help = err.get_help();
        assert!(help.is_none());
    }

    #[test]
    fn test_with_help_formatting() {
        let err = RushError::new("FILE_NOT_FOUND", "missing.txt: No such file", 1);
        let formatted = err.with_help();
        assert!(formatted.contains("missing.txt: No such file"));
        assert!(formatted.contains("Help:"));
        assert!(formatted.contains("File or directory not found"));
    }

    #[test]
    fn test_with_help_no_help_available() {
        let err = RushError::new("UNKNOWN_ERROR", "Something went wrong", 1);
        let formatted = err.with_help();
        // Should still contain the error message, just without help
        assert!(formatted.contains("Something went wrong"));
    }
}
