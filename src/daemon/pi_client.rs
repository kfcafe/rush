//! Pi agent client for Unix socket IPC
//!
//! Provides a client to communicate with the Pi agent daemon over Unix sockets
//! using the JSONL protocol defined in `protocol.rs`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

use crate::daemon::protocol::{decode_jsonl, encode_jsonl, PiToRush, RushToPi, ShellContext};

/// Global counter for generating unique request IDs
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique request ID
fn generate_request_id() -> String {
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("rush-{}-{}", timestamp, count)
}

/// Errors that can occur when communicating with the Pi daemon
#[derive(Debug, Error)]
pub enum PiClientError {
    /// Pi daemon is not running (no socket found)
    #[error("Pi daemon not running: no socket found at any of the expected paths")]
    NotRunning,

    /// Socket exists but connection failed
    #[error("Failed to connect to Pi daemon at {path}: {source}")]
    ConnectionFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Protocol error (invalid message format)
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// I/O error during communication
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Client for communicating with the Pi agent daemon
pub struct PiClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl PiClient {
    /// Connect to Pi daemon
    ///
    /// Tries socket paths in order:
    /// 1. `$RUSH_PI_SOCKET` environment variable
    /// 2. `~/.pi/rush.sock`
    /// 3. `/tmp/pi-rush-$USER.sock`
    ///
    /// # Errors
    ///
    /// Returns `PiClientError::NotRunning` if no socket is found.
    /// Returns `PiClientError::ConnectionFailed` if a socket exists but connection fails.
    pub fn connect() -> Result<Self, PiClientError> {
        let socket_path = Self::find_socket()?;
        let stream =
            UnixStream::connect(&socket_path).map_err(|e| PiClientError::ConnectionFailed {
                path: socket_path,
                source: e,
            })?;

        // Clone the stream for the reader (UnixStream implements Clone via dup())
        let reader_stream = stream.try_clone()?;
        let reader = BufReader::new(reader_stream);

        Ok(Self { stream, reader })
    }

    /// Connect to a specific socket path (for testing or custom configurations)
    ///
    /// # Errors
    ///
    /// Returns `PiClientError::ConnectionFailed` if connection fails.
    pub fn connect_to(socket_path: PathBuf) -> Result<Self, PiClientError> {
        let stream =
            UnixStream::connect(&socket_path).map_err(|e| PiClientError::ConnectionFailed {
                path: socket_path,
                source: e,
            })?;

        let reader_stream = stream.try_clone()?;
        let reader = BufReader::new(reader_stream);

        Ok(Self { stream, reader })
    }

    /// Find the Pi daemon socket path
    ///
    /// Search order:
    /// 1. `$RUSH_PI_SOCKET` environment variable
    /// 2. `~/.pi/rush.sock`
    /// 3. `/tmp/pi-rush-$USER.sock`
    fn find_socket() -> Result<PathBuf, PiClientError> {
        // 1. Check environment variable
        if let Ok(path) = std::env::var("RUSH_PI_SOCKET") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(path);
            }
        }

        // 2. Check ~/.pi/rush.sock
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".pi").join("rush.sock");
            if path.exists() {
                return Ok(path);
            }
        }

        // 3. Check /tmp/pi-rush-$USER.sock
        let username = whoami::username();
        let path = PathBuf::from(format!("/tmp/pi-rush-{}.sock", username));
        if path.exists() {
            return Ok(path);
        }

        Err(PiClientError::NotRunning)
    }

    /// Send a message to the Pi daemon
    fn send(&mut self, message: &RushToPi) -> Result<(), PiClientError> {
        let line =
            encode_jsonl(message).map_err(|e| PiClientError::ProtocolError(e.to_string()))?;
        self.stream.write_all(line.as_bytes())?;
        self.stream.flush()?;
        Ok(())
    }

    /// Read a single response from the Pi daemon
    fn read_response(&mut self) -> Result<Option<PiToRush>, PiClientError> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line)?;

        if bytes_read == 0 {
            return Ok(None); // EOF
        }

        let message: PiToRush =
            decode_jsonl(&line).map_err(|e| PiClientError::ProtocolError(e.to_string()))?;
        Ok(Some(message))
    }

    /// Send a query and return a streaming response iterator
    ///
    /// The iterator yields `PiToRush` messages until a `Done` or `Error` message
    /// is received. Tool calls should be handled by the caller.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's query/prompt
    /// * `stdin` - Optional stdin content piped to the query
    /// * `context` - Current shell context (cwd, history, etc.)
    ///
    /// # Returns
    ///
    /// An iterator over `PiToRush` messages. The iterator continues until
    /// a terminal message (`Done` or `Error`) is received.
    pub fn query(
        &mut self,
        prompt: &str,
        stdin: Option<&str>,
        context: ShellContext,
    ) -> Result<ResponseIterator<'_>, PiClientError> {
        let id = generate_request_id();
        let msg = RushToPi::Query {
            id: id.clone(),
            prompt: prompt.to_string(),
            stdin: stdin.map(String::from),
            context,
        };
        self.send(&msg)?;
        Ok(ResponseIterator {
            client: self,
            request_id: id,
            done: false,
        })
    }

    /// Send an intent query to convert natural language to a shell command
    ///
    /// Used by the `? <intent>` prefix. Pi returns a suggested command
    /// that the user can accept, edit, or cancel.
    ///
    /// # Arguments
    ///
    /// * `intent` - Natural language intent (e.g., "find all rust files modified today")
    /// * `context` - Current shell context (cwd, history, etc.)
    /// * `project_type` - Detected project type (e.g., "rust", "node", "python")
    ///
    /// # Returns
    ///
    /// An iterator over `PiToRush` messages. Typically returns a single
    /// `SuggestedCommand` message followed by `Done`.
    pub fn intent(
        &mut self,
        intent: &str,
        context: ShellContext,
        project_type: Option<&str>,
    ) -> Result<ResponseIterator<'_>, PiClientError> {
        let id = generate_request_id();
        let msg = RushToPi::Intent {
            id: id.clone(),
            intent: intent.to_string(),
            context,
            project_type: project_type.map(String::from),
        };
        self.send(&msg)?;
        Ok(ResponseIterator {
            client: self,
            request_id: id,
            done: false,
        })
    }

    /// Handle a tool call from Pi by sending the result back
    ///
    /// This is called after executing a tool that Pi requested.
    ///
    /// # Arguments
    ///
    /// * `tool_call_id` - The ID from the `ToolCall` message
    /// * `output` - The tool's output (stdout/result)
    /// * `exit_code` - The tool's exit code (0 for success)
    pub fn send_tool_result(
        &mut self,
        tool_call_id: &str,
        output: String,
        exit_code: i32,
    ) -> Result<(), PiClientError> {
        let msg = RushToPi::ToolResult {
            id: tool_call_id.to_string(),
            output,
            exit_code,
        };
        self.send(&msg)
    }

    /// Execute a tool call and send the result back to Pi
    ///
    /// Handles the "bash" tool by executing commands. Other tools
    /// return an error result.
    ///
    /// # Arguments
    ///
    /// * `tool_call_id` - The ID from the `ToolCall` message
    /// * `tool` - The tool name (e.g., "bash")
    /// * `args` - The tool arguments as JSON
    pub fn handle_tool_call(
        &mut self,
        tool_call_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> Result<(), PiClientError> {
        let (output, exit_code) = match tool {
            "bash" => {
                let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");

                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                {
                    Ok(result) => {
                        let stdout = String::from_utf8_lossy(&result.stdout);
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        let output = if stderr.is_empty() {
                            stdout.to_string()
                        } else {
                            format!("{}\n{}", stdout, stderr)
                        };
                        let exit_code = result.status.code().unwrap_or(-1);
                        (output, exit_code)
                    }
                    Err(e) => (format!("Failed to execute command: {}", e), 1),
                }
            }
            _ => (format!("Unknown tool: {}", tool), 1),
        };

        self.send_tool_result(tool_call_id, output, exit_code)
    }

    /// Check if the Pi daemon is available (socket exists)
    pub fn is_available() -> bool {
        Self::find_socket().is_ok()
    }

    /// Get the socket path that would be used for connection
    pub fn socket_path() -> Option<PathBuf> {
        Self::find_socket().ok()
    }
}

/// Iterator over streaming responses from Pi
pub struct ResponseIterator<'a> {
    client: &'a mut PiClient,
    request_id: String,
    done: bool,
}

impl<'a> ResponseIterator<'a> {
    /// Get the request ID for this query
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl<'a> Iterator for ResponseIterator<'a> {
    type Item = Result<PiToRush, PiClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.client.read_response() {
            Ok(Some(msg)) => {
                // Check for terminal messages
                match &msg {
                    PiToRush::Done { .. } | PiToRush::Error { .. } => {
                        self.done = true;
                    }
                    _ => {}
                }
                Some(Ok(msg))
            }
            Ok(None) => {
                self.done = true;
                None // EOF
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();

        // IDs should be unique
        assert_ne!(id1, id2);

        // IDs should start with "rush-"
        assert!(id1.starts_with("rush-"));
        assert!(id2.starts_with("rush-"));
    }

    #[test]
    fn test_pi_client_error_display() {
        let err = PiClientError::NotRunning;
        assert!(err.to_string().contains("not running"));

        let err = PiClientError::ProtocolError("invalid json".to_string());
        assert!(err.to_string().contains("Protocol error"));
    }

    #[test]
    fn test_shell_context_creation() {
        let context = ShellContext {
            cwd: "/home/user".to_string(),
            last_command: Some("ls -la".to_string()),
            last_exit_code: Some(0),
            history: vec!["cd /home".to_string(), "ls".to_string()],
            env: HashMap::new(),
        };

        assert_eq!(context.cwd, "/home/user");
        assert_eq!(context.last_command, Some("ls -la".to_string()));
    }

    #[test]
    fn test_is_available_returns_false_when_no_socket() {
        // In test environment, there's likely no Pi daemon running
        // This test just ensures the function doesn't panic
        let _ = PiClient::is_available();
    }

    #[test]
    fn test_socket_path_discovery() {
        // Test that socket_path() doesn't panic
        let _ = PiClient::socket_path();
    }
}
