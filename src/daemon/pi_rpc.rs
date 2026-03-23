//! Pi RPC subprocess manager for fast `|?` execution
//!
//! Manages Pi as a long-lived subprocess in RPC mode (`pi --rpc`).
//! This reuses Pi's battle-tested RPC protocol over stdin/stdout
//! for lower latency than Unix socket IPC.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Global counter for generating unique request IDs
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique request ID
fn generate_request_id() -> String {
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("rush-rpc-{}-{}", timestamp, count)
}

/// Errors that can occur when using Pi RPC
#[derive(Debug, Error)]
pub enum PiRpcError {
    /// Failed to spawn Pi subprocess
    #[error("Failed to spawn pi subprocess: {0}")]
    SpawnFailed(#[source] std::io::Error),

    /// Pi subprocess not running
    #[error("Pi subprocess is not running")]
    NotRunning,

    /// Failed to write to Pi stdin
    #[error("Failed to write to Pi stdin: {0}")]
    WriteError(#[source] std::io::Error),

    /// Failed to read from Pi stdout
    #[error("Failed to read from Pi stdout: {0}")]
    ReadError(#[source] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Pi returned an error
    #[error("Pi error: {0}")]
    PiError(String),

    /// Pi subprocess exited unexpectedly
    #[error("Pi subprocess exited unexpectedly")]
    ProcessExited,
}

/// Pi RPC command (subset we need for Rush integration)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PiCommand {
    /// Send a prompt to Pi
    #[serde(rename = "prompt")]
    Prompt {
        /// Unique request ID
        id: String,
        /// The prompt/message to send
        message: String,
    },
}

/// Pi RPC events we care about from the response stream
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum PiEvent {
    /// Streaming content delta (partial response)
    #[serde(rename = "content_delta")]
    ContentDelta {
        /// Content fragment
        content: String,
    },

    /// Agent has finished processing
    #[serde(rename = "agent_end")]
    AgentEnd {},

    /// Error occurred during processing
    #[serde(rename = "error")]
    Error {
        /// Error message
        message: String,
    },

    /// Ready indicator (Pi is ready to accept commands)
    #[serde(rename = "ready")]
    Ready {},

    /// Unknown event type - allows forward compatibility
    #[serde(other)]
    Unknown,
}

/// Manages Pi as a long-lived subprocess in RPC mode
///
/// Spawns `pi --rpc` and communicates via stdin/stdout using JSONL protocol.
/// The subprocess is kept alive between requests for fast execution.
pub struct PiRpcManager {
    process: Option<Child>,
    stdin: Option<std::process::ChildStdin>,
    stdout_reader: Option<BufReader<std::process::ChildStdout>>,
}

impl PiRpcManager {
    /// Create a new Pi RPC manager (subprocess not started yet)
    pub fn new() -> Self {
        Self {
            process: None,
            stdin: None,
            stdout_reader: None,
        }
    }

    /// Check if the Pi subprocess is currently running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            // try_wait returns Ok(Some(_)) if exited, Ok(None) if still running
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited, clean up
                    self.cleanup();
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.cleanup();
                    false
                }
            }
        } else {
            false
        }
    }

    /// Clean up subprocess resources
    fn cleanup(&mut self) {
        self.stdin = None;
        self.stdout_reader = None;
        self.process = None;
    }

    /// Ensure Pi subprocess is running, spawning it if necessary
    pub fn ensure_running(&mut self) -> Result<(), PiRpcError> {
        if self.is_running() {
            return Ok(());
        }

        // Clean up any stale state
        self.cleanup();

        // Check for RUSH_PI_PATH override (for testing), otherwise use "pi"
        let pi_path = std::env::var("RUSH_PI_PATH").unwrap_or_else(|_| "pi".to_string());

        // Spawn pi --rpc
        let mut child = Command::new(&pi_path)
            .arg("--rpc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(PiRpcError::SpawnFailed)?;

        // Take ownership of stdin/stdout
        self.stdin = child.stdin.take();
        self.stdout_reader = child.stdout.take().map(BufReader::new);
        self.process = Some(child);

        // Wait for ready signal (optional - Pi may send a ready event)
        // For now, we assume Pi is ready immediately after spawn

        Ok(())
    }

    /// Send a prompt to Pi and return an iterator over response events
    ///
    /// The iterator yields `PiEvent` items until `AgentEnd` or `Error` is received.
    pub fn prompt(&mut self, message: &str) -> Result<PiEventIterator<'_>, PiRpcError> {
        self.ensure_running()?;

        let cmd = PiCommand::Prompt {
            id: generate_request_id(),
            message: message.to_string(),
        };

        // Serialize command to JSON
        let json =
            serde_json::to_string(&cmd).map_err(|e| PiRpcError::ProtocolError(e.to_string()))?;

        // Send command (JSONL format - one line per message)
        let stdin = self.stdin.as_mut().ok_or(PiRpcError::NotRunning)?;
        writeln!(stdin, "{}", json).map_err(PiRpcError::WriteError)?;
        stdin.flush().map_err(PiRpcError::WriteError)?;

        // Return iterator over responses
        Ok(PiEventIterator {
            manager: self,
            done: false,
        })
    }

    /// Send a prompt and collect the full response as a string
    ///
    /// This is a convenience method that collects all content deltas
    /// into a single string.
    pub fn prompt_blocking(&mut self, message: &str) -> Result<String, PiRpcError> {
        let mut result = String::new();

        for event in self.prompt(message)? {
            match event? {
                PiEvent::ContentDelta { content } => {
                    result.push_str(&content);
                }
                PiEvent::Error { message } => {
                    return Err(PiRpcError::PiError(message));
                }
                PiEvent::AgentEnd {} => break,
                _ => {}
            }
        }

        Ok(result)
    }

    /// Gracefully stop the Pi subprocess
    pub fn stop(&mut self) -> Result<(), PiRpcError> {
        if let Some(ref mut child) = self.process {
            // Drop stdin to signal EOF
            self.stdin = None;

            // Try graceful shutdown first with SIGTERM
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM);
            }

            // Wait a bit for graceful exit
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait();
        }

        self.cleanup();
        Ok(())
    }

    /// Read a single event from Pi's stdout
    fn read_event(&mut self) -> Result<Option<PiEvent>, PiRpcError> {
        let reader = self.stdout_reader.as_mut().ok_or(PiRpcError::NotRunning)?;

        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).map_err(PiRpcError::ReadError)?;

        if bytes_read == 0 {
            return Ok(None); // EOF - process likely exited
        }

        // Parse JSON event
        let event: PiEvent = serde_json::from_str(line.trim()).map_err(|e| {
            PiRpcError::ProtocolError(format!("Invalid JSON: {} (line: {})", e, line.trim()))
        })?;

        Ok(Some(event))
    }
}

impl Default for PiRpcManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PiRpcManager {
    fn drop(&mut self) {
        // Graceful shutdown on drop
        let _ = self.stop();
    }
}

/// Iterator over Pi RPC events
///
/// Yields events until `AgentEnd` or `Error` is received, or the connection closes.
pub struct PiEventIterator<'a> {
    manager: &'a mut PiRpcManager,
    done: bool,
}

impl<'a> Iterator for PiEventIterator<'a> {
    type Item = Result<PiEvent, PiRpcError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.manager.read_event() {
            Ok(Some(event)) => {
                // Check for terminal events
                match &event {
                    PiEvent::AgentEnd {} | PiEvent::Error { .. } => {
                        self.done = true;
                    }
                    _ => {}
                }
                Some(Ok(event))
            }
            Ok(None) => {
                // EOF - connection closed
                self.done = true;
                Some(Err(PiRpcError::ProcessExited))
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

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();

        // IDs should be unique
        assert_ne!(id1, id2);

        // IDs should start with "rush-rpc-"
        assert!(id1.starts_with("rush-rpc-"));
        assert!(id2.starts_with("rush-rpc-"));
    }

    #[test]
    fn test_pi_rpc_manager_new() {
        let manager = PiRpcManager::new();
        assert!(manager.process.is_none());
        assert!(manager.stdin.is_none());
        assert!(manager.stdout_reader.is_none());
    }

    #[test]
    fn test_pi_rpc_manager_default() {
        let manager = PiRpcManager::default();
        assert!(manager.process.is_none());
    }

    #[test]
    fn test_pi_command_serialization() {
        let cmd = PiCommand::Prompt {
            id: "test-123".to_string(),
            message: "Hello, Pi!".to_string(),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""type":"prompt""#));
        assert!(json.contains(r#""id":"test-123""#));
        assert!(json.contains(r#""message":"Hello, Pi!""#));
    }

    #[test]
    fn test_pi_event_deserialization_content_delta() {
        let json = r#"{"type":"content_delta","content":"Hello"}"#;
        let event: PiEvent = serde_json::from_str(json).unwrap();

        match event {
            PiEvent::ContentDelta { content } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected ContentDelta"),
        }
    }

    #[test]
    fn test_pi_event_deserialization_agent_end() {
        let json = r#"{"type":"agent_end"}"#;
        let event: PiEvent = serde_json::from_str(json).unwrap();

        match event {
            PiEvent::AgentEnd {} => {}
            _ => panic!("Expected AgentEnd"),
        }
    }

    #[test]
    fn test_pi_event_deserialization_error() {
        let json = r#"{"type":"error","message":"Something went wrong"}"#;
        let event: PiEvent = serde_json::from_str(json).unwrap();

        match event {
            PiEvent::Error { message } => {
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_pi_event_deserialization_unknown() {
        // Unknown event types should deserialize to Unknown for forward compatibility
        let json = r#"{"type":"future_event","data":"something"}"#;
        let event: PiEvent = serde_json::from_str(json).unwrap();

        match event {
            PiEvent::Unknown => {}
            _ => panic!("Expected Unknown"),
        }
    }

    #[test]
    fn test_pi_rpc_error_display() {
        let err = PiRpcError::NotRunning;
        assert!(err.to_string().contains("not running"));

        let err = PiRpcError::PiError("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = PiRpcError::ProtocolError("invalid json".to_string());
        assert!(err.to_string().contains("Protocol error"));
    }

    #[test]
    fn test_is_running_when_not_started() {
        let mut manager = PiRpcManager::new();
        assert!(!manager.is_running());
    }
}
