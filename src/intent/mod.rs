//! Intent-to-command module for the `?` prefix feature
//!
//! Converts natural language intent into shell commands using Pi.
//!
//! ## Usage
//! ```bash
//! ? find all rust files modified today
//! # Pi generates: find . -name "*.rs" -mtime 0
//! # Shows preview, user confirms with Enter or edits
//! ```

use crate::daemon::{PiClient, PiClientError, PiToRush, ShellContext};
use nu_ansi_term::Color;
use std::collections::HashMap;
use std::io::{self, Write};

/// Result of processing an intent query
#[derive(Debug, Clone)]
pub enum IntentResult {
    /// User accepted the suggested command
    Accept(String),
    /// User wants to edit the command (pre-filled in line editor)
    Edit(String),
    /// User cancelled (Ctrl-C)
    Cancel,
    /// Error occurred
    Error(String),
}

/// Suggested command from Pi
#[derive(Debug, Clone)]
pub struct SuggestedCommand {
    /// The suggested shell command
    pub command: String,
    /// Brief explanation of what the command does
    pub explanation: String,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

/// Detect the project type based on files in the current directory
///
/// Returns a project type identifier like "rust", "node", "python", etc.
pub fn detect_project_type() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;

    // Check for various project markers in order of specificity
    let checks: &[(&str, &str)] = &[
        ("Cargo.toml", "rust"),
        ("package.json", "node"),
        ("pyproject.toml", "python"),
        ("requirements.txt", "python"),
        ("setup.py", "python"),
        ("go.mod", "go"),
        ("Gemfile", "ruby"),
        ("pom.xml", "java"),
        ("build.gradle", "java"),
        ("build.gradle.kts", "kotlin"),
        ("CMakeLists.txt", "cmake"),
        ("Makefile", "make"),
        ("docker-compose.yml", "docker"),
        ("docker-compose.yaml", "docker"),
        ("Dockerfile", "docker"),
        (".git", "git"), // At least detect if it's a git repo
    ];

    for (file, project_type) in checks {
        if cwd.join(file).exists() {
            return Some(project_type.to_string());
        }
    }

    None
}

/// Build shell context for the intent query
pub fn build_shell_context(
    last_command: Option<&str>,
    last_exit_code: Option<i32>,
    history: Vec<String>,
) -> ShellContext {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    // Collect relevant environment variables
    let mut env = HashMap::new();
    for key in &["PATH", "HOME", "USER", "SHELL", "EDITOR", "TERM"] {
        if let Ok(val) = std::env::var(key) {
            env.insert(key.to_string(), val);
        }
    }

    ShellContext {
        cwd,
        last_command: last_command.map(String::from),
        last_exit_code,
        history,
        env,
    }
}

/// Send intent to Pi and get suggested command
///
/// Returns the suggested command or an error
pub fn query_intent(
    client: &mut PiClient,
    intent: &str,
    context: ShellContext,
    project_type: Option<&str>,
) -> Result<SuggestedCommand, PiClientError> {
    let mut responses = client.intent(intent, context, project_type)?;

    // Collect responses - we expect a SuggestedCommand followed by Done
    for response in responses.by_ref() {
        match response? {
            PiToRush::SuggestedCommand {
                command,
                explanation,
                confidence,
                ..
            } => {
                return Ok(SuggestedCommand {
                    command,
                    explanation,
                    confidence,
                });
            }
            PiToRush::Error { message, .. } => {
                return Err(PiClientError::ProtocolError(message));
            }
            PiToRush::Done { .. } => {
                // Done without a command means no suggestion
                return Err(PiClientError::ProtocolError(
                    "No command suggestion received".to_string(),
                ));
            }
            PiToRush::Chunk { content, .. } => {
                // Streaming chunks - Pi might stream the explanation
                // For now, we ignore these as we expect SuggestedCommand
                eprintln!("{}", content);
            }
            PiToRush::ToolCall { .. } => {
                // Tool calls shouldn't happen for intent queries
                // but handle gracefully
            }
        }
    }

    Err(PiClientError::ProtocolError(
        "Connection closed without response".to_string(),
    ))
}

/// Display the suggested command with syntax highlighting and explanation
pub fn display_suggestion(suggestion: &SuggestedCommand) {
    // Display the suggested command with styling
    let command_style = Color::Cyan.bold();
    let explanation_style = Color::DarkGray;
    let prompt_style = Color::Yellow;

    println!();
    println!(
        "{}",
        explanation_style.paint(format!("# {}", suggestion.explanation))
    );
    println!("{}", command_style.paint(&suggestion.command));
    println!();

    // Show confidence indicator if low
    if suggestion.confidence < 0.7 {
        println!(
            "{}",
            Color::Yellow.paint(format!(
                "⚠ Low confidence ({:.0}%) - review carefully",
                suggestion.confidence * 100.0
            ))
        );
        println!();
    }

    // Show action hints
    println!(
        "{}",
        prompt_style.paint("[Enter] Execute  [Tab] Edit  [Ctrl-C] Cancel")
    );
}

/// Handle user input for accepting, editing, or cancelling
///
/// This is a simple implementation that uses raw terminal input.
/// In the full reedline integration, this would be more sophisticated.
pub fn prompt_user_action() -> IntentResult {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    // Enable raw mode for key-by-key input
    if enable_raw_mode().is_err() {
        return IntentResult::Error("Failed to enable raw mode".to_string());
    }

    let result = loop {
        match event::read() {
            Ok(Event::Key(key_event)) => {
                match (key_event.code, key_event.modifiers) {
                    // Enter - accept
                    (KeyCode::Enter, _) => {
                        break IntentResult::Accept(String::new());
                    }
                    // Tab - edit
                    (KeyCode::Tab, _) => {
                        break IntentResult::Edit(String::new());
                    }
                    // Ctrl-C - cancel
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        break IntentResult::Cancel;
                    }
                    // Escape - cancel
                    (KeyCode::Esc, _) => {
                        break IntentResult::Cancel;
                    }
                    _ => {
                        // Ignore other keys
                    }
                }
            }
            Err(e) => {
                break IntentResult::Error(format!("Input error: {}", e));
            }
            _ => {
                // Ignore other events
            }
        }
    };

    // Restore terminal
    let _ = disable_raw_mode();
    println!(); // Move to next line

    result
}

/// Process an intent query (the `? <intent>` prefix)
///
/// This is the main entry point for the intent feature:
/// 1. Detects project type
/// 2. Sends intent to Pi
/// 3. Displays suggestion
/// 4. Handles user input (accept/edit/cancel)
/// 5. Returns the result
pub fn process_intent(
    intent: &str,
    last_command: Option<&str>,
    last_exit_code: Option<i32>,
    history: Vec<String>,
) -> IntentResult {
    // Try to connect to Pi
    let mut client = match PiClient::connect() {
        Ok(c) => c,
        Err(PiClientError::NotRunning) => {
            eprintln!(
                "{}",
                Color::Yellow.paint("Pi daemon not running. Start with: pi daemon start")
            );
            return IntentResult::Error("Pi daemon not running".to_string());
        }
        Err(e) => {
            eprintln!(
                "{}",
                Color::Red.paint(format!("Failed to connect to Pi: {}", e))
            );
            return IntentResult::Error(e.to_string());
        }
    };

    // Build context
    let context = build_shell_context(last_command, last_exit_code, history);
    let project_type = detect_project_type();

    // Query Pi for suggested command
    print!("{}", Color::DarkGray.paint("Thinking..."));
    io::stdout().flush().ok();

    let suggestion = match query_intent(&mut client, intent, context, project_type.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            // Clear "Thinking..." line
            print!("\r                \r");
            io::stdout().flush().ok();
            eprintln!("{}", Color::Red.paint(format!("Error: {}", e)));
            return IntentResult::Error(e.to_string());
        }
    };

    // Clear "Thinking..." line
    print!("\r                \r");
    io::stdout().flush().ok();

    // Display the suggestion
    display_suggestion(&suggestion);

    // Get user action
    match prompt_user_action() {
        IntentResult::Accept(_) => IntentResult::Accept(suggestion.command),
        IntentResult::Edit(_) => IntentResult::Edit(suggestion.command),
        IntentResult::Cancel => IntentResult::Cancel,
        IntentResult::Error(e) => IntentResult::Error(e),
    }
}

/// Check if a line is an intent query (starts with `?`)
pub fn is_intent_query(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("? ") || trimmed == "?"
}

/// Extract the intent from a query line
///
/// Removes the leading `?` and any whitespace
pub fn extract_intent(line: &str) -> &str {
    let trimmed = line.trim();
    if trimmed.starts_with("? ") {
        &trimmed[2..]
    } else if trimmed == "?" {
        ""
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_intent_query() {
        assert!(is_intent_query("? find all rust files"));
        assert!(is_intent_query("?"));
        assert!(is_intent_query("  ? deploy to staging"));
        assert!(!is_intent_query("echo hello"));
        assert!(!is_intent_query("?command")); // No space after ?
        assert!(!is_intent_query("echo ?"));
    }

    #[test]
    fn test_extract_intent() {
        assert_eq!(
            extract_intent("? find all rust files"),
            "find all rust files"
        );
        assert_eq!(extract_intent("?"), "");
        assert_eq!(extract_intent("  ? deploy  "), "deploy");
    }

    #[test]
    fn test_detect_project_type() {
        // This test depends on the actual cwd, so we just verify it doesn't panic
        let _ = detect_project_type();
    }

    #[test]
    fn test_build_shell_context() {
        let context = build_shell_context(
            Some("ls -la"),
            Some(0),
            vec!["cd /tmp".to_string(), "ls".to_string()],
        );

        assert!(!context.cwd.is_empty());
        assert_eq!(context.last_command, Some("ls -la".to_string()));
        assert_eq!(context.last_exit_code, Some(0));
        assert_eq!(context.history.len(), 2);
    }

    #[test]
    fn test_suggested_command() {
        let suggestion = SuggestedCommand {
            command: "find . -name \"*.rs\" -mtime 0".to_string(),
            explanation: "Find all Rust files modified today".to_string(),
            confidence: 0.95,
        };

        assert!(suggestion.confidence > 0.9);
        assert!(suggestion.command.contains("find"));
    }
}
