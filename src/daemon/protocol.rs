//! Rush protocol implementation
//!
//! This module contains two protocols:
//!
//! ## 1. Daemon Protocol (Binary/Bincode)
//!
//! Used for Rush client ↔ Daemon communication. Length-prefixed binary messages.
//!
//! ```text
//! ┌────────────┬──────────────┬──────────────────────┐
//! │   Length   │  Message ID  │  Payload (bincode)   │
//! │  (4 bytes) │  (4 bytes)   │  (variable length)   │
//! └────────────┴──────────────┴──────────────────────┘
//! ```
//!
//! ## 2. Rush ↔ Pi IPC Protocol (JSONL)
//!
//! Used for Rush shell ↔ Pi agent communication. Newline-delimited JSON.
//!
//! - Each message is one line
//! - UTF-8 encoded
//! - Tagged unions with `"type"` field
//!
//! Message types:
//! - [`RushToPi`]: Messages from Rush shell to Pi agent
//! - [`PiToRush`]: Messages from Pi agent to Rush shell

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};

/// Maximum message size (10MB to prevent memory exhaustion)
const MAX_MESSAGE_SIZE: u32 = 10 * 1024 * 1024;

/// Message ID counter type (unique per message for request/response correlation)
pub type MessageId = u32;

/// Message envelope containing all possible message types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Message {
    /// Client initiates a new session
    SessionInit(SessionInit),
    /// Daemon acknowledges session creation
    SessionInitAck(SessionInitAck),
    /// Client requests command execution
    Execute(Execute),
    /// Daemon returns execution result
    ExecutionResult(ExecutionResult),
    /// Client sends signal to running command
    Signal(Signal),
    /// Client requests daemon shutdown
    Shutdown(Shutdown),
    /// Client requests system stats
    StatsRequest(StatsRequest),
    /// Daemon returns cached stats
    StatsResponse(StatsResponse),
}

/// Session initialization request (Client → Daemon)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInit {
    /// Working directory for the session
    pub working_dir: String,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Command-line arguments
    pub args: Vec<String>,
    /// How to handle stdin: "inherit", "pipe", or "null"
    pub stdin_mode: String,
}

/// Session initialization acknowledgment (Daemon → Client)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInitAck {
    /// Unique session ID
    pub session_id: u64,
    /// Worker process PID
    pub worker_pid: i32,
}

/// Command execution request (Client → Daemon)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Execute {
    /// Session ID to execute in
    pub session_id: u64,
    /// Command string to execute
    pub command: String,
}

/// Execution result response (Daemon → Client)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Exit code of the command
    pub exit_code: i32,
    /// Number of bytes written to stdout
    pub stdout_len: u64,
    /// Number of bytes written to stderr
    pub stderr_len: u64,
    /// Standard output content
    pub stdout: String,
    /// Standard error content
    pub stderr: String,
}

/// Signal delivery request (Client → Daemon)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// Session ID to signal
    pub session_id: u64,
    /// Signal number (e.g., SIGINT=2, SIGTERM=15)
    pub signal: i32,
}

/// Daemon shutdown request (Client → Daemon)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shutdown {
    /// Whether to force shutdown (kill active sessions)
    pub force: bool,
}

/// Request system stats from daemon (Client → Daemon)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatsRequest {
    /// Which stats to fetch (empty = all cached stats)
    pub stats: Vec<String>,
}

/// System stats response (Daemon → Client)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatsResponse {
    /// Built-in stats (host, os, uptime, memory, etc.)
    pub builtin: HashMap<String, String>,
    /// Custom stats defined by user
    pub custom: HashMap<String, String>,
    /// Unix timestamp (seconds) when stats were last updated
    pub updated_at: u64,
}

/// Encode a message into the wire format
///
/// Format: [4-byte length][4-byte message ID][bincode payload]
pub fn encode_message(message: &Message, message_id: MessageId) -> io::Result<Vec<u8>> {
    // Serialize the message to bincode
    let payload =
        bincode::serialize(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Check size limit
    let payload_len = payload.len() as u32;
    if payload_len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", payload_len),
        ));
    }

    // Build the frame: length (4) + message_id (4) + payload
    let total_len = 8 + payload_len;
    let mut buffer = Vec::with_capacity(total_len as usize);

    // Write length prefix (includes message_id + payload)
    buffer.extend_from_slice(&(payload_len + 4).to_le_bytes());
    // Write message ID
    buffer.extend_from_slice(&message_id.to_le_bytes());
    // Write payload
    buffer.extend_from_slice(&payload);

    Ok(buffer)
}

/// Decode a message from the wire format
///
/// Returns (message, message_id)
pub fn decode_message<R: Read>(reader: &mut R) -> io::Result<(Message, MessageId)> {
    // Read length prefix (4 bytes)
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let payload_len = u32::from_le_bytes(len_bytes);

    // Validate length
    if payload_len < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Message length too small",
        ));
    }
    if payload_len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", payload_len),
        ));
    }

    // Read message ID (4 bytes)
    let mut id_bytes = [0u8; 4];
    reader.read_exact(&mut id_bytes)?;
    let message_id = u32::from_le_bytes(id_bytes);

    // Read bincode payload
    let data_len = (payload_len - 4) as usize;
    let mut payload = vec![0u8; data_len];
    reader.read_exact(&mut payload)?;

    // Deserialize message
    let message: Message = bincode::deserialize(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok((message, message_id))
}

/// Write a message to a stream
pub fn write_message<W: Write>(
    writer: &mut W,
    message: &Message,
    message_id: MessageId,
) -> io::Result<()> {
    let bytes = encode_message(message, message_id)?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// Read a message from a stream
pub fn read_message<R: Read>(reader: &mut R) -> io::Result<(Message, MessageId)> {
    decode_message(reader)
}

// Unix FD passing helpers using SCM_RIGHTS

/// Send file descriptors over a Unix socket using SCM_RIGHTS
///
/// This allows zero-copy I/O by passing stdin/stdout/stderr FDs directly
pub fn send_fds<S: AsRawFd>(socket: &S, fds: &[RawFd]) -> io::Result<()> {
    use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};

    // We need to send at least 1 byte of data along with the FDs
    let dummy_data = [0u8; 1];
    let iov = [std::io::IoSlice::new(&dummy_data)];

    // Create SCM_RIGHTS control message
    let fds_msg = ControlMessage::ScmRights(fds);

    // Send the message
    sendmsg::<()>(
        socket.as_raw_fd(),
        &iov,
        &[fds_msg],
        MsgFlags::empty(),
        None,
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}

/// Receive file descriptors from a Unix socket using SCM_RIGHTS
///
/// Returns the received file descriptors
pub fn recv_fds<S: AsRawFd>(socket: &S, max_fds: usize) -> io::Result<Vec<RawFd>> {
    use nix::sys::socket::{recvmsg, ControlMessageOwned, MsgFlags};

    // Buffer to receive the dummy data
    let mut dummy_data = [0u8; 1];
    let mut iov = [std::io::IoSliceMut::new(&mut dummy_data)];

    // Buffer for control messages (each FD is 4 bytes)
    let mut cmsg_buffer = nix::cmsg_space!([RawFd; 16]);

    // Receive the message
    let msg = recvmsg::<()>(
        socket.as_raw_fd(),
        &mut iov,
        Some(&mut cmsg_buffer),
        MsgFlags::empty(),
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Extract file descriptors from control messages
    let mut fds = Vec::new();

    // Note: In nix 0.29, cmsgs() returns a Result<CmsgIterator>
    // The iterator yields ControlMessageOwned items
    match msg.cmsgs() {
        Ok(cmsgs_iter) => {
            for cmsg in cmsgs_iter {
                if let ControlMessageOwned::ScmRights(received_fds) = cmsg {
                    fds.extend(received_fds.iter().take(max_fds - fds.len()));
                    if fds.len() >= max_fds {
                        break;
                    }
                }
            }
        }
        Err(e) => {
            return Err(io::Error::new(io::ErrorKind::Other, e));
        }
    }

    Ok(fds)
}

// ============================================================================
// Rush ↔ Pi IPC Protocol (JSONL)
// ============================================================================
//
// Wire format: Newline-delimited JSON (JSONL)
// - Each message is one line
// - UTF-8 encoded
// ============================================================================

/// Shell context passed with every query from Rush to Pi
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellContext {
    /// Current working directory
    pub cwd: String,
    /// Last command executed (if any)
    pub last_command: Option<String>,
    /// Exit code of the last command (if any)
    pub last_exit_code: Option<i32>,
    /// Recent command history (last N commands)
    pub history: Vec<String>,
    /// Selected environment variables
    pub env: HashMap<String, String>,
}

/// Rush → Pi messages
///
/// Messages sent from the Rush shell to the Pi agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RushToPi {
    /// Query the LLM with a prompt and context
    #[serde(rename = "query")]
    Query {
        /// Unique request ID for correlation
        id: String,
        /// User's prompt/question
        prompt: String,
        /// Optional stdin content piped to the query
        stdin: Option<String>,
        /// Current shell context
        context: ShellContext,
    },
    /// Convert natural language intent to shell command(s)
    ///
    /// Used by the `? <intent>` prefix. Pi returns a suggested command
    /// that the user can accept, edit, or cancel.
    #[serde(rename = "intent")]
    Intent {
        /// Unique request ID for correlation
        id: String,
        /// Natural language intent (e.g., "find all rust files modified today")
        intent: String,
        /// Current shell context
        context: ShellContext,
        /// Detected project type (e.g., "rust", "node", "python")
        project_type: Option<String>,
    },
    /// Response to a tool call request from Pi
    #[serde(rename = "tool_result")]
    ToolResult {
        /// ID matching the original tool call
        id: String,
        /// Tool execution output
        output: String,
        /// Tool execution exit code
        exit_code: i32,
    },
}

/// Pi → Rush messages
///
/// Messages sent from the Pi agent to the Rush shell
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PiToRush {
    /// Streaming content chunk (partial response)
    #[serde(rename = "chunk")]
    Chunk {
        /// Request ID this chunk belongs to
        id: String,
        /// Content fragment
        content: String,
    },
    /// Stream complete - no more chunks for this request
    #[serde(rename = "done")]
    Done {
        /// Request ID that completed
        id: String,
    },
    /// Error occurred during processing
    #[serde(rename = "error")]
    Error {
        /// Request ID that failed
        id: String,
        /// Error description
        message: String,
    },
    /// Pi wants to execute a tool/command
    #[serde(rename = "tool_call")]
    ToolCall {
        /// Unique tool call ID for result correlation
        id: String,
        /// Tool name (e.g., "bash", "read", "write")
        tool: String,
        /// Tool arguments as JSON
        args: serde_json::Value,
    },
    /// Suggested command(s) for an intent query
    ///
    /// Response to `RushToPi::Intent`. Contains one or more shell commands
    /// that the user can accept, edit, or cancel.
    #[serde(rename = "suggested_command")]
    SuggestedCommand {
        /// Request ID this response belongs to
        id: String,
        /// The suggested shell command(s)
        command: String,
        /// Brief explanation of what the command does
        explanation: String,
        /// Confidence level (0.0-1.0) - lower confidence may warrant review
        confidence: f64,
    },
}

/// Encode a Rush ↔ Pi message to JSONL format (single line with newline)
pub fn encode_jsonl<T: Serialize>(message: &T) -> io::Result<String> {
    let json = serde_json::to_string(message)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(format!("{}\n", json))
}

/// Decode a Rush ↔ Pi message from a JSONL line
pub fn decode_jsonl<T: for<'de> Deserialize<'de>>(line: &str) -> io::Result<T> {
    serde_json::from_str(line.trim()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_session_init() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/test".to_string());

        let message = Message::SessionInit(SessionInit {
            working_dir: "/tmp".to_string(),
            env,
            args: vec!["-c".to_string(), "echo test".to_string()],
            stdin_mode: "inherit".to_string(),
        });

        let message_id = 42;
        let encoded = encode_message(&message, message_id).unwrap();

        // Decode from bytes
        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_session_init_ack() {
        let message = Message::SessionInitAck(SessionInitAck {
            session_id: 12345,
            worker_pid: 98765,
        });

        let message_id = 43;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_execute() {
        let message = Message::Execute(Execute {
            session_id: 12345,
            command: "ls -la".to_string(),
        });

        let message_id = 44;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_execution_result() {
        let message = Message::ExecutionResult(ExecutionResult {
            exit_code: 0,
            stdout_len: 1024,
            stderr_len: 0,
            stdout: "stdout".to_string(),
            stderr: "stderr".to_string(),
        });

        let message_id = 45;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_signal() {
        let message = Message::Signal(Signal {
            session_id: 12345,
            signal: 2, // SIGINT
        });

        let message_id = 46;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_shutdown() {
        let message = Message::Shutdown(Shutdown { force: true });

        let message_id = 47;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_stats_request() {
        // Test with specific stats
        let message = Message::StatsRequest(StatsRequest {
            stats: vec![
                "memory".to_string(),
                "cpu".to_string(),
                "uptime".to_string(),
            ],
        });

        let message_id = 48;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_stats_request_empty() {
        // Test with empty stats (meaning "all stats")
        let message = Message::StatsRequest(StatsRequest { stats: vec![] });

        let message_id = 49;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_stats_response() {
        let mut builtin = HashMap::new();
        builtin.insert("host".to_string(), "myhost".to_string());
        builtin.insert("os".to_string(), "darwin".to_string());
        builtin.insert("uptime".to_string(), "3600".to_string());
        builtin.insert("memory_total".to_string(), "16384".to_string());
        builtin.insert("memory_used".to_string(), "8192".to_string());

        let mut custom = HashMap::new();
        custom.insert("git_branch".to_string(), "main".to_string());
        custom.insert("node_version".to_string(), "v20.10.0".to_string());

        let message = Message::StatsResponse(StatsResponse {
            builtin,
            custom,
            updated_at: 1706745600, // Example Unix timestamp
        });

        let message_id = 50;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_encode_decode_stats_response_empty() {
        // Test with empty stats maps
        let message = Message::StatsResponse(StatsResponse {
            builtin: HashMap::new(),
            custom: HashMap::new(),
            updated_at: 0,
        });

        let message_id = 51;
        let encoded = encode_message(&message, message_id).unwrap();

        let mut cursor = std::io::Cursor::new(encoded);
        let (decoded, decoded_id) = decode_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_message_too_large() {
        // Create a message with a very large payload
        let large_env: HashMap<String, String> = (0..100000)
            .map(|i| (format!("VAR_{}", i), "x".repeat(100)))
            .collect();

        let message = Message::SessionInit(SessionInit {
            working_dir: "/tmp".to_string(),
            env: large_env,
            args: vec![],
            stdin_mode: "inherit".to_string(),
        });

        let result = encode_message(&message, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_read_message() {
        let message = Message::ExecutionResult(ExecutionResult {
            exit_code: 0,
            stdout_len: 512,
            stderr_len: 0,
            stdout: "stdout".to_string(),
            stderr: "stderr".to_string(),
        });

        let message_id = 100;

        // Write to a buffer
        let mut buffer = Vec::new();
        write_message(&mut buffer, &message, message_id).unwrap();

        // Read from the buffer
        let mut cursor = std::io::Cursor::new(buffer);
        let (decoded, decoded_id) = read_message(&mut cursor).unwrap();

        assert_eq!(message, decoded);
        assert_eq!(message_id, decoded_id);
    }

    #[test]
    fn test_multiple_messages() {
        let messages = vec![
            (
                Message::SessionInit(SessionInit {
                    working_dir: "/tmp".to_string(),
                    env: HashMap::new(),
                    args: vec![],
                    stdin_mode: "inherit".to_string(),
                }),
                1,
            ),
            (
                Message::Execute(Execute {
                    session_id: 1,
                    command: "echo test".to_string(),
                }),
                2,
            ),
            (
                Message::ExecutionResult(ExecutionResult {
                    exit_code: 0,
                    stdout_len: 5,
                    stderr_len: 0,
                    stdout: "stdout".to_string(),
                    stderr: "stderr".to_string(),
                }),
                3,
            ),
        ];

        // Write all messages to a buffer
        let mut buffer = Vec::new();
        for (msg, id) in &messages {
            write_message(&mut buffer, msg, *id).unwrap();
        }

        // Read all messages back
        let mut cursor = std::io::Cursor::new(buffer);
        for (expected_msg, expected_id) in &messages {
            let (msg, id) = read_message(&mut cursor).unwrap();
            assert_eq!(*expected_msg, msg);
            assert_eq!(*expected_id, id);
        }
    }

    // =========================================================================
    // Rush ↔ Pi JSONL Protocol Tests
    // =========================================================================

    #[test]
    fn test_rush_to_pi_query() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());

        let message = RushToPi::Query {
            id: "req-123".to_string(),
            prompt: "list all files".to_string(),
            stdin: Some("file1.txt\nfile2.txt".to_string()),
            context: ShellContext {
                cwd: "/home/user/project".to_string(),
                last_command: Some("ls".to_string()),
                last_exit_code: Some(0),
                history: vec!["cd project".to_string(), "ls".to_string()],
                env,
            },
        };

        // Encode to JSONL
        let encoded = encode_jsonl(&message).unwrap();

        // Should be single line ending with newline
        assert!(encoded.ends_with('\n'));
        assert_eq!(encoded.matches('\n').count(), 1);

        // Should contain type tag
        assert!(encoded.contains(r#""type":"query""#));

        // Decode back
        let decoded: RushToPi = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_rush_to_pi_tool_result() {
        let message = RushToPi::ToolResult {
            id: "tool-456".to_string(),
            output: "total 32\ndrwxr-xr-x 5 user staff 160 Jan 1 12:00 .".to_string(),
            exit_code: 0,
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""type":"tool_result""#));

        let decoded: RushToPi = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_pi_to_rush_chunk() {
        let message = PiToRush::Chunk {
            id: "req-123".to_string(),
            content: "Here are the files in ".to_string(),
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""type":"chunk""#));

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_pi_to_rush_done() {
        let message = PiToRush::Done {
            id: "req-123".to_string(),
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""type":"done""#));

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_pi_to_rush_error() {
        let message = PiToRush::Error {
            id: "req-123".to_string(),
            message: "Rate limit exceeded".to_string(),
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""type":"error""#));

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_pi_to_rush_tool_call() {
        let message = PiToRush::ToolCall {
            id: "tool-789".to_string(),
            tool: "bash".to_string(),
            args: serde_json::json!({
                "command": "ls -la",
                "timeout": 30
            }),
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""type":"tool_call""#));
        assert!(encoded.contains(r#""tool":"bash""#));

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_jsonl_multiline_content() {
        // Content can have newlines, but the JSON itself is single line
        let message = PiToRush::Chunk {
            id: "req-123".to_string(),
            content: "line1\nline2\nline3".to_string(),
        };

        let encoded = encode_jsonl(&message).unwrap();
        // The JSON escapes newlines, so there's only one actual newline at the end
        assert_eq!(encoded.matches('\n').count(), 1);
        assert!(encoded.contains(r#"\n"#)); // escaped newlines in JSON

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_shell_context_serialization() {
        let context = ShellContext {
            cwd: "/home/user".to_string(),
            last_command: None,
            last_exit_code: None,
            history: vec![],
            env: HashMap::new(),
        };

        let json = serde_json::to_string(&context).unwrap();
        let decoded: ShellContext = serde_json::from_str(&json).unwrap();
        assert_eq!(context, decoded);
    }

    #[test]
    fn test_rush_to_pi_intent() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());

        let message = RushToPi::Intent {
            id: "intent-123".to_string(),
            intent: "find all rust files modified today".to_string(),
            context: ShellContext {
                cwd: "/home/user/project".to_string(),
                last_command: Some("cargo build".to_string()),
                last_exit_code: Some(0),
                history: vec!["cargo build".to_string(), "cargo test".to_string()],
                env,
            },
            project_type: Some("rust".to_string()),
        };

        // Encode to JSONL
        let encoded = encode_jsonl(&message).unwrap();

        // Should be single line ending with newline
        assert!(encoded.ends_with('\n'));
        assert_eq!(encoded.matches('\n').count(), 1);

        // Should contain type tag
        assert!(encoded.contains(r#""type":"intent""#));
        assert!(encoded.contains(r#""intent":"find all rust files modified today""#));
        assert!(encoded.contains(r#""project_type":"rust""#));

        // Decode back
        let decoded: RushToPi = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_rush_to_pi_intent_no_project_type() {
        let message = RushToPi::Intent {
            id: "intent-456".to_string(),
            intent: "deploy to staging".to_string(),
            context: ShellContext {
                cwd: "/tmp".to_string(),
                last_command: None,
                last_exit_code: None,
                history: vec![],
                env: HashMap::new(),
            },
            project_type: None,
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""project_type":null"#));

        let decoded: RushToPi = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_pi_to_rush_suggested_command() {
        let message = PiToRush::SuggestedCommand {
            id: "intent-123".to_string(),
            command: r#"find . -name "*.rs" -mtime 0"#.to_string(),
            explanation: "Finds all Rust files (*.rs) modified today (-mtime 0)".to_string(),
            confidence: 0.95,
        };

        let encoded = encode_jsonl(&message).unwrap();
        assert!(encoded.contains(r#""type":"suggested_command""#));
        assert!(encoded.contains(r#"find . -name"#));
        assert!(encoded.contains(r#""confidence":0.95"#));

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }

    #[test]
    fn test_pi_to_rush_suggested_command_multi_line() {
        let message = PiToRush::SuggestedCommand {
            id: "intent-789".to_string(),
            command: "git push origin main && ssh staging \"./deploy.sh\"".to_string(),
            explanation: "Push to main branch, then deploy via SSH".to_string(),
            confidence: 0.85,
        };

        let encoded = encode_jsonl(&message).unwrap();
        // Should still be single line JSON
        assert_eq!(encoded.matches('\n').count(), 1);

        let decoded: PiToRush = decode_jsonl(&encoded).unwrap();
        assert_eq!(message, decoded);
    }
}
