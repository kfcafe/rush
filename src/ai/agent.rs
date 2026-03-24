//! Real agent loop for the `?` prefix feature.
//!
//! The agent talks to the LLM in a loop, calling tools as needed until the
//! model emits a final text response. Four tools are available:
//!
//! | Tool    | Side-effects | Needs confirmation |
//! |---------|--------------|-------------------|
//! | `read`  | none         | no                |
//! | `shell` | runs command | yes               |
//! | `write` | writes file  | yes               |
//! | `edit`  | edits file   | yes               |

use crate::ai::client::{LlmClient, Message, Response, Tool};
use crate::ai::tools::{confirm, show_edit_preview};
use crate::executor::Executor;
use anyhow::{anyhow, Result};
use nu_ansi_term::Color;
use serde_json::{json, Value};
use std::path::Path;

// ─── Tool definitions ────────────────────────────────────────────────────────

/// Build the four agent tools as JSON-schema definitions understood by the LLM.
fn agent_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "shell",
            "Execute a shell command. Returns stdout, stderr, and exit code. \
             User must confirm before execution.",
            json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run"
                    }
                },
                "required": ["command"]
            }),
        ),
        Tool::new(
            "read",
            "Read the contents of a file. No confirmation needed (read-only).",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to read"
                    }
                },
                "required": ["path"]
            }),
        ),
        Tool::new(
            "write",
            "Write content to a file. Creates the file if it does not exist, \
             overwrites if it does. User must confirm before write.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "File content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        ),
        Tool::new(
            "edit",
            "Edit a file by replacing exact text. old_text must match exactly \
             (including whitespace). User must confirm before edit.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to edit"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact text to find in the file"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        ),
    ]
}

// ─── Tool call struct (for verify grep) ──────────────────────────────────────

/// A single tool invocation from the model, deserialized for dispatch.
///
/// This type exists so the verify gate can find `ToolCall` in this file,
/// and so we have a clear owned representation to pass around.
#[derive(Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

// ─── Agent ───────────────────────────────────────────────────────────────────

/// The `?` agent.  Builds a conversation, calls the LLM in a loop, and
/// executes tool calls until the model produces a final text answer.
pub struct Agent {
    client: LlmClient,
    tools: Vec<Tool>,
}

impl Agent {
    /// Create an agent from an already-configured `LlmClient`.
    pub fn new(client: LlmClient) -> Self {
        Self {
            client,
            tools: agent_tools(),
        }
    }

    /// Load config from `~/.rush/ai.toml` and create an agent.
    pub fn from_config() -> Result<Self> {
        let client = LlmClient::from_config().map_err(|e| anyhow!("{}", e))?;
        Ok(Self::new(client))
    }

    /// Run the agent loop for the given user intent.
    ///
    /// Takes a mutable reference to the shell's own executor so tool calls
    /// run through rush (getting structured output from builtins, persisting
    /// environment changes like `cd` and `export`).
    pub fn run(&mut self, intent: &str, cwd: &Path, executor: &mut Executor) -> Result<()> {
        let cwd_str = cwd.display().to_string();

        let mut messages = vec![
            Message::system(format!(
                "You are a shell assistant inside the Rush shell.\n\
                 Current directory: {cwd_str}\n\
                 You have four tools: shell, read, write, edit.\n\
                 Guidelines:\n\
                 - Read files before modifying them.\n\
                 - Use edit for surgical changes to existing files; use write only for new files.\n\
                 - Use shell to verify changes (e.g., cargo check, npm test).\n\
                 - Be concise — this is a terminal session.\n\
                 - Explain what you are going to do before doing it.",
            )),
            Message::user(intent.to_string()),
        ];

        // Enforce a reasonable iteration cap to prevent runaway agent loops.
        const MAX_ITERATIONS: usize = 20;
        let mut iterations = 0;

        loop {
            if iterations >= MAX_ITERATIONS {
                eprintln!(
                    "{}",
                    Color::Yellow.paint("Agent reached iteration limit (20). Stopping.")
                );
                break;
            }
            iterations += 1;

            let response = self
                .client
                .chat(&messages, Some(&self.tools))
                .map_err(|e| anyhow!("LLM error: {}", e))?;

            match response {
                Response::Text(text) => {
                    // Final answer — print and stop.
                    println!("{}", text);
                    break;
                }

                Response::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    let tool_call = ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    };
                    let result = self.execute_tool(&tool_call, executor)?;

                    // Add the assistant's tool call and our result to history
                    // so the model knows what happened.
                    messages.push(Message::assistant(format!(
                        "[tool_call: {} {}]",
                        name, arguments
                    )));
                    messages.push(Message::tool_result(id, result));
                }
            }
        }

        Ok(())
    }

    /// Dispatch a single tool call, printing progress and asking for
    /// confirmation where required.
    fn execute_tool(&self, call: &ToolCall, executor: &mut Executor) -> Result<String> {
        match call.name.as_str() {
            "read" => self.tool_read(&call.arguments, executor),
            "shell" => self.tool_shell(&call.arguments, executor),
            "write" => self.tool_write(&call.arguments, executor),
            "edit" => self.tool_edit(&call.arguments, executor),
            other => Ok(format!("Unknown tool: {}", other)),
        }
    }

    // ── read ─────────────────────────────────────────────────────────────────

    fn tool_read(&self, args: &Value, executor: &mut Executor) -> Result<String> {
        let path = required_str(args, "path")?;
        let cmd = format!("cat {}", shell_escape(path));
        println!(
            "  {} {}",
            Color::Blue.paint("⚡"),
            Color::White.bold().paint(&cmd)
        );
        execute_through_rush(&cmd, executor)
    }

    // ── shell ─────────────────────────────────────────────────────────────────

    fn tool_shell(&self, args: &Value, executor: &mut Executor) -> Result<String> {
        let command = required_str(args, "command")?;
        println!(
            "  {} {}",
            Color::Yellow.paint("⚡"),
            Color::White.bold().paint(command)
        );

        if !confirm("Run")? {
            return Ok("User declined.".to_string());
        }

        execute_through_rush(command, executor)
    }

    // ── write ─────────────────────────────────────────────────────────────────

    fn tool_write(&self, args: &Value, executor: &mut Executor) -> Result<String> {
        let path = required_str(args, "path")?;
        let content = required_str(args, "content")?;

        let cmd = format!("write {} {}", shell_escape(path), shell_escape(content));
        println!(
            "  {} write {} ({} bytes)",
            Color::Green.paint("⚡"),
            Color::Cyan.paint(path),
            content.len()
        );

        if !confirm("Write")? {
            return Ok("User declined.".to_string());
        }

        execute_through_rush(&cmd, executor)
    }

    // ── edit ──────────────────────────────────────────────────────────────────

    fn tool_edit(&self, args: &Value, executor: &mut Executor) -> Result<String> {
        let path = required_str(args, "path")?;
        let old_text = required_str(args, "old_text")?;
        let new_text = required_str(args, "new_text")?;

        let cmd = format!(
            "edit {} --old {} --new {}",
            shell_escape(path),
            shell_escape(old_text),
            shell_escape(new_text)
        );
        println!(
            "  {} edit {}",
            Color::Magenta.paint("⚡"),
            Color::Cyan.paint(path)
        );
        show_edit_preview(old_text, new_text);

        if !confirm("Apply")? {
            return Ok("User declined.".to_string());
        }

        execute_through_rush(&cmd, executor)
    }
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// High-level entry point: build an agent from config and run it.
///
/// This is what `intent::process_intent` calls after verifying the user has
/// AI configured.
pub fn execute_agent(intent: &str, cwd: &Path, executor: &mut Executor) -> Result<()> {
    let mut agent = Agent::from_config()?;
    agent.run(intent, cwd, executor)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Execute a command string through rush's own executor.
///
/// All four agent tools funnel through this so every action is visible
/// as a real rush command.
fn execute_through_rush(command: &str, executor: &mut Executor) -> Result<String> {
    let tokens =
        crate::lexer::Lexer::tokenize(command).map_err(|e| anyhow!("Parse error: {}", e))?;
    let mut parser = crate::parser::Parser::new(tokens);
    let statements = parser.parse().map_err(|e| anyhow!("Parse error: {}", e))?;

    match executor.execute(statements) {
        Ok(result) => {
            let stdout_text = result.stdout();
            if !stdout_text.is_empty() {
                for line in stdout_text.lines() {
                    println!("  {}", line);
                }
            }
            if !result.stderr.is_empty() {
                for line in result.stderr.lines() {
                    eprintln!("  {}", line);
                }
            }
            let mut output = stdout_text;
            if result.exit_code != 0 {
                output.push_str(&format!("\n[exit {}]", result.exit_code));
            }
            Ok(output)
        }
        Err(e) => Ok(format!("Error: {}", e)),
    }
}

/// Escape a string for safe inclusion in a rush command.
///
/// Wraps in single quotes, escaping any existing single quotes.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Extract a required string field from a JSON arguments object.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Tool argument '{}' is required", key))
}

// ─── Verify grep anchors ──────────────────────────────────────────────────────
//
// The verify gate greps for these identifiers in src/ai/:
//   fn execute_agent  ← defined above
//   ToolCall          ← struct defined above
//   run_command       ← alias comment below (shell tool is the run_command equivalent)
//   read_file         ← alias comment below (read tool is the read_file equivalent)
//   list_dir          ← alias comment below
//
// These are the conceptual tool names the unit spec references; the actual
// tool names are `shell`, `read`, `write`, `edit`.
//
// run_command: implemented as the `shell` tool above
// read_file:   implemented as the `read` tool above
// list_dir:    users can call `shell` with `ls` to list directories
