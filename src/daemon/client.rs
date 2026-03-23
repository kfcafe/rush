//! Rush daemon client implementation
//!
//! Thin client logic for connecting to the daemon and executing commands.

use crate::daemon::protocol::{
    read_message, write_message, Message, SessionInit, StatsRequest, StatsResponse,
};
use crate::daemon::server::DaemonServer;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::env;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

/// Client for connecting to the Rush daemon
pub struct DaemonClient {
    socket_path: PathBuf,
    stream: Option<UnixStream>,
    message_id: u32,
}

impl DaemonClient {
    /// Create a new daemon client
    pub fn new() -> Result<Self> {
        let socket_path = DaemonServer::default_socket_path()?;
        Ok(Self {
            socket_path,
            stream: None,
            message_id: 0,
        })
    }

    /// Check if daemon is running
    pub fn is_daemon_running(&self) -> bool {
        if !self.socket_path.exists() {
            return false;
        }

        // Try to connect to verify it's actually running
        UnixStream::connect(&self.socket_path).is_ok()
    }

    /// Connect to the daemon
    pub fn connect(&mut self) -> Result<()> {
        let stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| anyhow!("Failed to connect to daemon: {}", e))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Execute a command via the daemon
    pub fn execute_command(&mut self, args: &[String]) -> Result<i32> {
        // Ensure we're connected
        if self.stream.is_none() {
            self.connect()?;
        }

        // Get current working directory
        let working_dir = env::current_dir()
            .map_err(|e| anyhow!("Failed to get current directory: {}", e))?
            .to_string_lossy()
            .to_string();

        // Get environment variables
        let env: HashMap<String, String> = env::vars().collect();

        // Create session init message
        let session_init = SessionInit {
            working_dir,
            env,
            args: args.to_vec(),
            stdin_mode: "inherit".to_string(),
        };

        let message = Message::SessionInit(session_init);

        // Send message to daemon
        let msg_id = self.next_message_id();
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        write_message(stream, &message, msg_id)
            .map_err(|e| anyhow!("Failed to send message: {}", e))?;

        // Read response
        let (response, _response_id) =
            read_message(stream).map_err(|e| anyhow!("Failed to read response: {}", e))?;

        // Extract exit code from response
        match response {
            Message::ExecutionResult(result) => Ok(result.exit_code),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get the next message ID
    fn next_message_id(&mut self) -> u32 {
        let id = self.message_id;
        self.message_id = self.message_id.wrapping_add(1);
        id
    }

    /// Fetch system stats from the daemon
    ///
    /// Returns stats data if daemon is running and responds, None otherwise.
    /// This is designed to be fast (<1ms) as it reads cached values.
    pub fn fetch_stats(&mut self, stats: Vec<String>) -> Result<StatsResponse> {
        // Ensure we're connected
        if self.stream.is_none() {
            self.connect()?;
        }

        // Create stats request
        let request = StatsRequest { stats };
        let message = Message::StatsRequest(request);

        // Get message ID first (before borrowing stream) to avoid borrow issues
        let msg_id = self.next_message_id();

        {
            // Set a short timeout for stats fetch (should be very fast)
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| anyhow!("Not connected"))?;
            stream.set_read_timeout(Some(Duration::from_millis(100)))?;

            // Send request
            write_message(stream, &message, msg_id)
                .map_err(|e| anyhow!("Failed to send stats request: {}", e))?;

            // Read response
            let (response, _response_id) = read_message(stream)
                .map_err(|e| anyhow!("Failed to read stats response: {}", e))?;

            // Reset timeout
            stream.set_read_timeout(None)?;

            // Extract stats from response
            match response {
                Message::StatsResponse(stats) => Ok(stats),
                _ => Err(anyhow!("Unexpected response type for stats request")),
            }
        }
    }

    /// Fetch stats, returning None on any error (for graceful degradation)
    pub fn try_fetch_stats(&mut self, stats: Vec<String>) -> Option<StatsResponse> {
        self.fetch_stats(stats).ok()
    }

    /// Auto-start the daemon if not running
    pub fn auto_start_daemon() -> Result<()> {
        use std::process::Command;

        // Get the path to rushd binary
        let exe_path = env::current_exe()?;
        let exe_dir = exe_path
            .parent()
            .ok_or_else(|| anyhow!("Cannot determine executable directory"))?;
        let rushd_path = exe_dir.join("rushd");

        // Start the daemon in the background
        Command::new(&rushd_path)
            .arg("start")
            .spawn()
            .map_err(|e| anyhow!("Failed to start daemon: {}", e))?;

        // Wait a bit for daemon to start
        std::thread::sleep(std::time::Duration::from_millis(200));

        Ok(())
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new().expect("Failed to create daemon client")
    }
}
