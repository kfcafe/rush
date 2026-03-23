use crate::daemon::config::{CustomStatConfig, DaemonConfig};
use crate::daemon::protocol::{
    read_message, write_message, ExecutionResult, Message, SessionInit, StatsResponse,
};
use crate::daemon::worker_pool::{PoolConfig, WorkerPool};
use crate::stats::StatsCollector;
use anyhow::{anyhow, Result};
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{fork, ForkResult, Pid};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Maximum concurrent sessions
const MAX_CONCURRENT_SESSIONS: usize = 100;

/// Health check interval (how often to check workers)
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(10);

/// Ping timeout (worker must respond within this time)
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Request timeout (worker must complete requests within this time)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum respawn attempts for a failed worker
const MAX_RESPAWN_ATTEMPTS: u32 = 3;

/// Respawn cooldown period (prevent rapid respawn loops)
const RESPAWN_COOLDOWN: Duration = Duration::from_secs(60);

/// Stats cache update interval (how often to refresh dynamic stats)
const STATS_UPDATE_INTERVAL: Duration = Duration::from_secs(5);

/// Session identifier (process ID of worker)
pub type SessionId = i32;

// =============================================================================
// StatsCache - Caches system stats for fast retrieval
// =============================================================================

/// Cached value for a custom stat
#[derive(Debug, Clone)]
pub struct CustomStatCached {
    /// Current cached value
    pub value: String,
    /// Last error (if command failed)
    pub error: Option<String>,
    /// When the stat was last updated
    pub last_update: Instant,
    /// Configured refresh interval
    pub interval: Duration,
    /// Configured timeout
    pub timeout: Duration,
    /// Shell command to execute
    pub command: String,
}

impl CustomStatCached {
    /// Create a new custom stat cache entry
    fn new(config: &CustomStatConfig) -> Self {
        Self {
            value: String::new(),
            error: None,
            last_update: Instant::now() - Duration::from_secs(3600), // Force immediate update
            interval: config.interval,
            timeout: config.timeout,
            command: config.command.clone(),
        }
    }

    /// Check if this stat needs to be refreshed
    fn needs_update(&self) -> bool {
        self.last_update.elapsed() >= self.interval
    }

    /// Update the stat by executing its command
    fn update(&mut self) {
        let result = Command::new("sh").arg("-c").arg(&self.command).output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    // First line only, trimmed
                    self.value = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    self.error = None;
                } else {
                    self.value = String::new();
                    self.error = Some(format!("exit {}", output.status.code().unwrap_or(-1)));
                }
            }
            Err(e) => {
                self.value = String::new();
                self.error = Some(e.to_string());
            }
        }
        self.last_update = Instant::now();
    }
}

/// Stats cache - holds all cached system statistics
///
/// Provides near-instant stats retrieval for banner display.
/// Built-in stats are computed once on startup, dynamic stats
/// are refreshed periodically, and custom stats run user commands
/// with configurable intervals.
#[derive(Debug)]
pub struct StatsCache {
    /// Built-in stats (static: host, os, kernel, arch, cpu, cores)
    builtin_static: HashMap<String, String>,
    /// Built-in stats (dynamic: uptime, load, procs, memory, time, date)
    builtin_dynamic: HashMap<String, String>,
    /// Custom stats (user-defined commands)
    custom: HashMap<String, CustomStatCached>,
    /// When built-in dynamic stats were last updated
    last_builtin_update: Instant,
}

impl StatsCache {
    /// Create a new stats cache with initial built-in stats
    pub fn new() -> Self {
        // Collect all built-in stats initially
        let all_builtins = StatsCollector::collect_builtins();

        // Split into static and dynamic
        let static_names = ["host", "os", "kernel", "arch", "cpu", "cores"];
        let mut builtin_static = HashMap::new();
        let mut builtin_dynamic = HashMap::new();

        for (name, value) in all_builtins {
            if static_names.contains(&name.as_str()) {
                builtin_static.insert(name, value);
            } else {
                builtin_dynamic.insert(name, value);
            }
        }

        Self {
            builtin_static,
            builtin_dynamic,
            custom: HashMap::new(),
            last_builtin_update: Instant::now(),
        }
    }

    /// Initialize custom stats from config
    pub fn init_custom_stats(&mut self, configs: &[CustomStatConfig]) {
        self.custom.clear();
        for config in configs {
            self.custom
                .insert(config.name.clone(), CustomStatCached::new(config));
        }
    }

    /// Update dynamic built-in stats if enough time has passed
    pub fn update_builtins_if_needed(&mut self) {
        if self.last_builtin_update.elapsed() >= STATS_UPDATE_INTERVAL {
            self.update_builtins();
        }
    }

    /// Force update of dynamic built-in stats
    pub fn update_builtins(&mut self) {
        let dynamic_names = ["uptime", "load", "procs", "memory", "time", "date"];

        for name in &dynamic_names {
            if let Some(value) = StatsCollector::collect_stat(name) {
                self.builtin_dynamic.insert(name.to_string(), value);
            }
        }

        self.last_builtin_update = Instant::now();
    }

    /// Get custom stats that need updating
    pub fn get_stats_needing_update(&self) -> Vec<String> {
        self.custom
            .iter()
            .filter(|(_, stat)| stat.needs_update())
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Update a specific custom stat
    pub fn update_custom_stat(&mut self, name: &str) {
        if let Some(stat) = self.custom.get_mut(name) {
            stat.update();
        }
    }

    /// Get all built-in stats as a HashMap
    pub fn get_builtin_stats(&self) -> HashMap<String, String> {
        let mut all = self.builtin_static.clone();
        all.extend(self.builtin_dynamic.clone());
        all
    }

    /// Get all custom stats as a HashMap (values only, no errors)
    pub fn get_custom_stats(&self) -> HashMap<String, String> {
        self.custom
            .iter()
            .filter(|(_, stat)| stat.error.is_none())
            .map(|(name, stat)| (name.clone(), stat.value.clone()))
            .collect()
    }

    /// Get seconds since last built-in update
    pub fn seconds_since_update(&self) -> u64 {
        self.last_builtin_update.elapsed().as_secs()
    }
}

/// Worker health state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerHealthState {
    /// Worker is healthy and responsive
    Healthy,
    /// Worker hasn't responded to recent ping
    Unresponsive,
    /// Worker is processing a slow request (not necessarily hung)
    Slow,
    /// Worker has crashed or been killed
    Crashed,
    /// Worker is confirmed hung (no response to multiple pings)
    Hung,
}

/// Worker metrics
#[derive(Debug, Clone)]
pub struct WorkerMetrics {
    /// Total requests processed
    pub requests_processed: u64,
    /// Total requests failed
    pub requests_failed: u64,
    /// Last successful request time
    pub last_request_time: Option<Instant>,
    /// Last successful ping/pong time
    pub last_heartbeat: Instant,
    /// Number of consecutive failed health checks
    pub consecutive_failures: u32,
    /// Number of times this worker has been respawned
    pub respawn_count: u32,
    /// Time when worker was created/last respawned
    pub spawn_time: Instant,
}

impl Default for WorkerMetrics {
    fn default() -> Self {
        Self {
            requests_processed: 0,
            requests_failed: 0,
            last_request_time: None,
            last_heartbeat: Instant::now(),
            consecutive_failures: 0,
            respawn_count: 0,
            spawn_time: Instant::now(),
        }
    }
}

/// Worker state snapshot for reset between requests
#[derive(Debug, Clone)]
struct WorkerState {
    /// Original working directory at worker startup
    original_cwd: PathBuf,
    /// Original environment variables at worker startup
    original_env: HashMap<String, String>,
}

impl WorkerState {
    /// Capture current worker state
    fn capture() -> Result<Self> {
        Ok(Self {
            original_cwd: std::env::current_dir()
                .map_err(|e| anyhow!("Failed to get current directory: {}", e))?,
            original_env: std::env::vars().collect(),
        })
    }

    /// Reset worker to original state
    fn reset(&self) -> Result<()> {
        // Reset working directory
        std::env::set_current_dir(&self.original_cwd)
            .map_err(|e| anyhow!("Failed to restore working directory: {}", e))?;

        // Reset environment variables
        // 1. Remove variables that were added
        let current_env: HashMap<String, String> = std::env::vars().collect();
        for key in current_env.keys() {
            if !self.original_env.contains_key(key) {
                std::env::remove_var(key);
            }
        }

        // 2. Restore original variables (handles both modified and removed vars)
        for (key, value) in &self.original_env {
            std::env::set_var(key, value);
        }

        Ok(())
    }
}

/// Handle to a session worker
#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub id: SessionId,
    pub worker_pid: Pid,
    pub created_at: Instant,
    /// Health state of the worker
    pub health_state: WorkerHealthState,
    /// Metrics for this worker
    pub metrics: WorkerMetrics,
    /// Last time we attempted to ping this worker
    pub last_ping_attempt: Option<Instant>,
}

/// Main daemon server
pub struct DaemonServer {
    socket_path: PathBuf,
    listener: Option<UnixListener>,
    sessions: HashMap<SessionId, SessionHandle>,
    shutdown: Arc<AtomicBool>,
    /// Flag to request config reload (set by SIGHUP handler)
    reload_config: Arc<AtomicBool>,
    /// Optional worker pool (if None, uses fork-per-request)
    worker_pool: Option<WorkerPool>,
    /// Current daemon configuration from .rushrc
    config: DaemonConfig,
    /// Cached system stats for fast retrieval
    stats_cache: Arc<Mutex<StatsCache>>,
}

impl DaemonServer {
    /// Create a new daemon server
    pub fn new(socket_path: PathBuf) -> Result<Self> {
        // Load initial configuration from .rushrc
        let config = DaemonConfig::from_rushrc();
        eprintln!(
            "Loaded config: {} custom stats defined",
            config.custom_stats.len()
        );

        // Initialize stats cache
        let mut stats_cache = StatsCache::new();
        stats_cache.init_custom_stats(&config.custom_stats);
        eprintln!(
            "Initialized stats cache with {} built-in stats",
            stats_cache.get_builtin_stats().len()
        );

        Ok(Self {
            socket_path,
            listener: None,
            sessions: HashMap::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
            reload_config: Arc::new(AtomicBool::new(false)),
            worker_pool: None,
            config,
            stats_cache: Arc::new(Mutex::new(stats_cache)),
        })
    }

    /// Enable worker pool mode with the specified configuration
    pub fn with_worker_pool(mut self, config: PoolConfig) -> Result<Self> {
        let pool = WorkerPool::new(config)?;
        eprintln!(
            "Worker pool enabled with {} workers",
            pool.stats().total_workers
        );
        self.worker_pool = Some(pool);
        Ok(self)
    }

    /// Create the daemon directory with secure permissions
    fn create_daemon_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;

        let daemon_dir = home.join(".rush");

        if !daemon_dir.exists() {
            fs::create_dir(&daemon_dir)?;

            // Set directory permissions to 0700 (owner only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&daemon_dir)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(&daemon_dir, perms)?;
            }
        }

        Ok(daemon_dir)
    }

    /// Start the daemon server
    pub fn start(&mut self) -> Result<()> {
        // Setup signal handlers
        self.setup_signal_handlers()?;

        // Bind to socket
        self.bind_socket()?;

        // Write PID file
        self.write_pid_file()?;

        println!("Rush daemon started on {}", self.socket_path.display());

        // Enter accept loop
        self.accept_loop()?;

        Ok(())
    }

    /// Bind the Unix socket
    fn bind_socket(&mut self) -> Result<()> {
        // Remove stale socket if it exists
        if self.socket_path.exists() {
            fs::remove_file(&self.socket_path)?;
        }

        // Create listener
        let listener = UnixListener::bind(&self.socket_path)?;

        // Set socket permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.socket_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&self.socket_path, perms)?;
        }

        self.listener = Some(listener);

        Ok(())
    }

    /// Setup signal handlers for graceful shutdown and config reload
    fn setup_signal_handlers(&self) -> Result<()> {
        let shutdown = self.shutdown.clone();
        let reload_config = self.reload_config.clone();

        // Handle SIGTERM for graceful shutdown
        signal_hook::flag::register(signal::SIGTERM as i32, shutdown.clone())?;

        // Handle SIGINT (Ctrl-C) for graceful shutdown
        signal_hook::flag::register(signal::SIGINT as i32, shutdown.clone())?;

        // Handle SIGHUP for config reload (without restart)
        signal_hook::flag::register(signal::SIGHUP as i32, reload_config)?;

        Ok(())
    }

    /// Reload configuration from .rushrc
    ///
    /// Called when SIGHUP is received or via `rushd reload` command.
    /// Updates custom stat entries: adds new, removes deleted, updates changed.
    pub fn reload_configuration(&mut self) {
        eprintln!("Reloading configuration from .rushrc...");

        let old_config = std::mem::take(&mut self.config);
        self.config = DaemonConfig::from_rushrc();

        // Log what changed
        let old_stats: std::collections::HashSet<_> =
            old_config.custom_stats.iter().map(|s| &s.name).collect();
        let new_stats: std::collections::HashSet<_> =
            self.config.custom_stats.iter().map(|s| &s.name).collect();

        // Find added stats
        for name in new_stats.difference(&old_stats) {
            eprintln!("  + Added custom stat: {}", name);
        }

        // Find removed stats
        for name in old_stats.difference(&new_stats) {
            eprintln!("  - Removed custom stat: {}", name);
        }

        // Find updated stats (same name but different command/interval/timeout)
        for stat in &self.config.custom_stats {
            if let Some(old_stat) = old_config.custom_stats.iter().find(|s| s.name == stat.name) {
                if old_stat.command != stat.command
                    || old_stat.interval != stat.interval
                    || old_stat.timeout != stat.timeout
                {
                    eprintln!("  ~ Updated custom stat: {}", stat.name);
                }
            }
        }

        // Log banner config changes
        if old_config.banner.style != self.config.banner.style {
            eprintln!(
                "  ~ Banner style: {:?} -> {:?}",
                old_config.banner.style, self.config.banner.style
            );
        }
        if old_config.banner.stats != self.config.banner.stats {
            eprintln!(
                "  ~ Banner stats: {:?} -> {:?}",
                old_config.banner.stats, self.config.banner.stats
            );
        }

        // Update stats cache with new custom stat configs
        if let Ok(mut cache) = self.stats_cache.lock() {
            cache.init_custom_stats(&self.config.custom_stats);
        }

        eprintln!(
            "Configuration reloaded: {} custom stats",
            self.config.custom_stats.len()
        );
    }

    /// Get the current configuration
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    /// Main accept loop
    fn accept_loop(&mut self) -> Result<()> {
        // Take ownership of listener temporarily for borrowing
        let listener = self
            .listener
            .take()
            .ok_or_else(|| anyhow!("Socket not bound"))?;

        // Set non-blocking mode for accept to check shutdown flag
        listener.set_nonblocking(true)?;

        let result = (|| -> Result<()> {
            let mut last_health_check = Instant::now();
            let mut last_stats_update = Instant::now();

            while !self.shutdown.load(Ordering::Relaxed) {
                // Check for config reload request (SIGHUP)
                if self.reload_config.swap(false, Ordering::Relaxed) {
                    self.reload_configuration();
                }

                // Periodic health checks
                let now = Instant::now();
                if now.duration_since(last_health_check) >= HEALTH_CHECK_INTERVAL {
                    self.check_all_workers_health();
                    last_health_check = now;
                }

                // Update stats cache (every 5 seconds)
                if now.duration_since(last_stats_update) >= STATS_UPDATE_INTERVAL {
                    self.update_stats_cache();
                    last_stats_update = now;
                }

                // Try to accept a connection
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        // Reset to blocking mode for the connection
                        stream.set_nonblocking(false)?;

                        // Check session limit
                        if self.sessions.len() >= MAX_CONCURRENT_SESSIONS {
                            eprintln!("Warning: Maximum concurrent sessions reached, rejecting connection");
                            let _ =
                                Self::send_error(&stream, "Maximum concurrent sessions reached", 0);
                            continue;
                        }

                        // Accept the connection (fork worker)
                        if let Err(e) = self.accept_connection(stream) {
                            eprintln!("Error accepting connection: {}", e);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection ready, reap workers and sleep briefly
                        self.reap_workers();
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
            Ok(())
        })();

        // Restore listener
        self.listener = Some(listener);

        println!("Shutting down daemon...");
        self.shutdown_gracefully()?;

        result
    }

    /// Accept a client connection and handle the request
    fn accept_connection(&mut self, mut stream: UnixStream) -> Result<()> {
        // Read the message
        let (msg, msg_id) = match read_message(&mut stream) {
            Ok(result) => result,
            Err(e) => {
                // Client disconnected or error reading
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(()); // Normal disconnection
                }
                return Err(anyhow!("Failed to read message: {}", e));
            }
        };

        // Handle different message types
        match msg {
            Message::SessionInit(session_init) => {
                // Dispatch based on mode
                if let Some(ref mut pool) = self.worker_pool {
                    // Worker pool mode: dispatch to pool
                    pool.dispatch_request(session_init, msg_id, stream)?;
                    Ok(())
                } else {
                    // Fork-per-request mode: fork a worker
                    self.accept_connection_fork(session_init, msg_id, stream)
                }
            }
            Message::StatsRequest(request) => {
                // Handle stats request directly in main process (no fork needed)
                self.handle_stats_request(&mut stream, msg_id, &request.stats)
            }
            _ => {
                Self::send_error(&stream, "Unexpected message type", msg_id)?;
                Ok(())
            }
        }
    }

    /// Handle a stats request - returns cached stats
    fn handle_stats_request(
        &self,
        stream: &mut UnixStream,
        msg_id: u32,
        requested_stats: &[String],
    ) -> Result<()> {
        let cache = self
            .stats_cache
            .lock()
            .map_err(|_| anyhow!("Failed to lock stats cache"))?;

        // Get all built-in and custom stats from cache
        let mut builtin = cache.get_builtin_stats();
        let mut custom = cache.get_custom_stats();

        // If specific stats were requested, filter to only those
        if !requested_stats.is_empty() {
            builtin.retain(|k, _| requested_stats.contains(k));
            custom.retain(|k, _| requested_stats.contains(k));
        }

        let response = StatsResponse {
            builtin,
            custom,
            updated_at: cache.seconds_since_update(),
        };

        write_message(stream, &Message::StatsResponse(response), msg_id)
            .map_err(|e| anyhow!("Failed to send stats response: {}", e))?;

        Ok(())
    }

    /// Update the stats cache (called periodically from accept loop)
    fn update_stats_cache(&self) {
        let stats_cache = self.stats_cache.clone();

        // Update built-in dynamic stats
        if let Ok(mut cache) = stats_cache.lock() {
            cache.update_builtins_if_needed();
        }

        // Get custom stats that need updating
        let stats_needing_update: Vec<String> = if let Ok(cache) = stats_cache.lock() {
            cache.get_stats_needing_update()
        } else {
            return;
        };

        // Update custom stats in background threads (non-blocking)
        for stat_name in stats_needing_update {
            let cache = stats_cache.clone();
            thread::spawn(move || {
                if let Ok(mut cache) = cache.lock() {
                    cache.update_custom_stat(&stat_name);
                }
            });
        }
    }

    /// Accept connection using fork-per-request (legacy mode)
    fn accept_connection_fork(
        &mut self,
        session_init: SessionInit,
        msg_id: u32,
        stream: UnixStream,
    ) -> Result<()> {
        // Fork a worker process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: register session
                let handle = SessionHandle {
                    id: child.as_raw(),
                    worker_pid: child,
                    created_at: Instant::now(),
                    health_state: WorkerHealthState::Healthy,
                    metrics: WorkerMetrics::default(),
                    last_ping_attempt: None,
                };

                self.sessions.insert(child.as_raw(), handle);

                // Close stream in parent (worker owns it)
                drop(stream);

                Ok(())
            }
            Ok(ForkResult::Child) => {
                // Child: become session worker
                // Execute the session init we already received
                Self::run_worker_with_session(stream, session_init, msg_id);
            }
            Err(e) => Err(anyhow!("Failed to fork worker: {}", e)),
        }
    }

    /// Run the session worker with a pre-read session init (in child process)
    fn run_worker_with_session(
        mut stream: UnixStream,
        session_init: SessionInit,
        msg_id: u32,
    ) -> ! {
        let exit_code = match Self::execute_session(&session_init) {
            Ok(exec_result) => {
                // Send result back to client
                let result = ExecutionResult {
                    exit_code: exec_result.exit_code,
                    stdout_len: exec_result.stdout().len() as u64,
                    stderr_len: exec_result.stderr.len() as u64,
                    stdout: exec_result.stdout(),
                    stderr: exec_result.stderr,
                };

                if let Err(e) =
                    write_message(&mut stream, &Message::ExecutionResult(result), msg_id)
                {
                    eprintln!("Failed to send result: {}", e);
                    1
                } else {
                    // Ensure data is flushed before worker exits
                    let _ = stream.flush();
                    let _ = stream.shutdown(std::net::Shutdown::Write);
                    0
                }
            }
            Err(e) => {
                eprintln!("Worker error: {}", e);
                1
            }
        };

        std::process::exit(exit_code);
    }

    /// Run the session worker (in child process)
    fn run_worker(stream: UnixStream) -> ! {
        let exit_code = match Self::handle_session(stream) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Worker error: {}", e);
                1
            }
        };

        std::process::exit(exit_code);
    }

    /// Handle a session in the worker process
    fn handle_session(mut stream: UnixStream) -> Result<i32> {
        // Capture initial worker state for reset between requests
        // (Currently workers are single-use, but this enables future worker pooling)
        let worker_state = WorkerState::capture()?;

        // Read session init message
        // Note: We don't set timeouts because they fail after fork with EINVAL
        // Status check connections will just disconnect immediately with UnexpectedEof
        let (msg, msg_id) = match read_message(&mut stream) {
            Ok(result) => result,
            Err(e) => {
                // Client disconnected without sending a message (e.g., status check)
                // This is normal, just exit quietly
                if e.kind() == std::io::ErrorKind::UnexpectedEof
                    || e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::ConnectionReset
                {
                    return Ok(0);
                }
                return Err(anyhow!("Failed to read message: {}", e));
            }
        };

        let session_init = match msg {
            Message::SessionInit(init) => init,
            _ => {
                Self::send_error(&stream, "Expected SessionInit message", msg_id)?;
                return Ok(1);
            }
        };

        // Execute the session
        let exec_result = Self::execute_session(&session_init)?;

        // Send result back to client
        let result = ExecutionResult {
            exit_code: exec_result.exit_code,
            stdout_len: exec_result.stdout().len() as u64,
            stderr_len: exec_result.stderr.len() as u64,
            stdout: exec_result.stdout(),
            stderr: exec_result.stderr,
        };

        write_message(&mut stream, &Message::ExecutionResult(result), msg_id)
            .map_err(|e| anyhow!("Failed to send result: {}", e))?;

        // Ensure data is flushed before worker exits
        stream.flush()?;

        // Shutdown write side to signal we're done
        let _ = stream.shutdown(std::net::Shutdown::Write);

        // Reset worker state for potential reuse (future worker pooling)
        // Currently workers exit after one request, but this ensures clean state
        if let Err(e) = worker_state.reset() {
            eprintln!("Warning: Failed to reset worker state: {}", e);
            // Non-fatal for single-use workers, but important for pooling
        }

        Ok(0)
    }

    /// Reset worker state between requests (for worker pooling)
    ///
    /// This ensures complete isolation between requests by resetting:
    /// - Working directory to original value
    /// - Environment variables to original state
    /// - Exit code (implicit in new executor creation)
    /// - Executor state (created fresh each time)
    ///
    /// Critical for preventing state leakage when workers handle multiple requests.
    fn reset_worker_state(worker_state: &WorkerState) -> Result<()> {
        worker_state.reset()
    }

    /// Execute a session command and return the full execution result
    fn execute_session(init: &SessionInit) -> Result<crate::executor::ExecutionResult> {
        // Set working directory
        std::env::set_current_dir(&init.working_dir).map_err(|e| {
            anyhow!(
                "Failed to set working directory to '{}': {}",
                init.working_dir,
                e
            )
        })?;

        // Set environment variables
        for (key, value) in &init.env {
            std::env::set_var(key, value);
        }

        // Parse and execute command
        // For now, just execute the args as a command via -c flag
        if init.args.len() >= 2 && init.args[0] == "-c" {
            let command = &init.args[1];

            // Create executor in embedded mode (no progress, all IO piped)
            // This prevents "Invalid argument" errors from inherited file descriptors
            // NOTE: Executor is created fresh for each request, ensuring no state leakage
            let mut executor = crate::executor::Executor::new_embedded();

            // Parse
            let tokens = match crate::lexer::Lexer::tokenize(command) {
                Ok(tokens) => tokens,
                Err(e) => {
                    let err_msg = format!("Lexer error: {}", e);
                    return Ok(crate::executor::ExecutionResult {
                        output: crate::executor::Output::Text(String::new()),
                        stderr: err_msg,
                        exit_code: 2,
                        error: None,
                    });
                }
            };
            let mut parser = crate::parser::Parser::new(tokens);

            match parser.parse() {
                Ok(ast) => match executor.execute(ast) {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        let err_msg = format!("Execution error: {}", e);
                        Ok(crate::executor::ExecutionResult {
                            output: crate::executor::Output::Text(String::new()),
                            stderr: err_msg,
                            exit_code: 1,
                            error: None,
                        })
                    }
                },
                Err(e) => {
                    let err_msg = format!("Parse error: {}", e);
                    Ok(crate::executor::ExecutionResult {
                        output: crate::executor::Output::Text(String::new()),
                        stderr: err_msg,
                        exit_code: 2,
                        error: None,
                    })
                }
            }
        } else {
            Ok(crate::executor::ExecutionResult {
                output: crate::executor::Output::Text(String::new()),
                stderr: String::new(),
                exit_code: 0,
                error: None,
            })
        }
    }

    /// Send an error message to the client
    fn send_error(stream: &UnixStream, message: &str, msg_id: u32) -> Result<()> {
        let mut stream = stream.try_clone()?;
        // For errors, we could define a custom error message type
        // For now, send an ExecutionResult with exit code 1
        let error_result = ExecutionResult {
            exit_code: 1,
            stdout_len: 0,
            stderr_len: message.len() as u64,
            stdout: String::new(),
            stderr: message.to_string(),
        };
        write_message(&mut stream, &Message::ExecutionResult(error_result), msg_id)
            .map_err(|e| anyhow!("Failed to send error: {}", e))
    }

    /// Reap finished worker processes
    fn reap_workers(&mut self) {
        let mut finished = Vec::new();

        for (session_id, handle) in &mut self.sessions {
            match waitpid(handle.worker_pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(pid, code)) => {
                    if code != 0 {
                        eprintln!("Worker {} exited with non-zero code: {}", session_id, code);
                        handle.metrics.requests_failed += 1;
                    }
                    handle.health_state = WorkerHealthState::Crashed;
                    finished.push(pid.as_raw());
                }
                Ok(WaitStatus::Signaled(pid, signal, _)) => {
                    eprintln!("Worker {} terminated by signal: {:?}", session_id, signal);
                    handle.health_state = WorkerHealthState::Crashed;
                    handle.metrics.requests_failed += 1;
                    finished.push(pid.as_raw());
                }
                Ok(WaitStatus::StillAlive) => {
                    // Still running, update heartbeat if recently active
                    // (In a full implementation, worker would send heartbeats)
                }
                Err(e) => {
                    eprintln!("Error waiting for process {}: {}", session_id, e);
                    handle.health_state = WorkerHealthState::Crashed;
                    finished.push(*session_id);
                }
                _ => {
                    // Other statuses, keep monitoring
                }
            }
        }

        // Remove finished sessions
        for session_id in finished {
            if let Some(handle) = self.sessions.remove(&session_id) {
                eprintln!(
                    "Cleaned up worker {} after {} requests ({} failed)",
                    session_id, handle.metrics.requests_processed, handle.metrics.requests_failed
                );
            }
        }
    }

    /// Write PID file for daemon management
    fn write_pid_file(&self) -> Result<()> {
        let pid_path = self
            .socket_path
            .parent()
            .ok_or_else(|| anyhow!("Invalid socket path"))?
            .join("daemon.pid");

        let pid = std::process::id();
        fs::write(&pid_path, pid.to_string())?;

        Ok(())
    }

    /// Remove PID file
    fn remove_pid_file(&self) -> Result<()> {
        let pid_path = self
            .socket_path
            .parent()
            .ok_or_else(|| anyhow!("Invalid socket path"))?
            .join("daemon.pid");

        if pid_path.exists() {
            fs::remove_file(&pid_path)?;
        }

        Ok(())
    }

    /// Graceful shutdown
    fn shutdown_gracefully(&mut self) -> Result<()> {
        // Shutdown worker pool if enabled
        if let Some(ref mut pool) = self.worker_pool {
            eprintln!("Shutting down worker pool...");
            pool.shutdown()?;
        }

        // Send SIGTERM to all worker processes (fork-per-request mode)
        for handle in self.sessions.values() {
            let _ = signal::kill(handle.worker_pid, Signal::SIGTERM);
        }

        // Wait for workers to exit (with timeout)
        let timeout = std::time::Duration::from_secs(5);
        let start = Instant::now();

        while !self.sessions.is_empty() && start.elapsed() < timeout {
            self.reap_workers();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Force kill any remaining workers
        for handle in self.sessions.values() {
            let _ = signal::kill(handle.worker_pid, Signal::SIGKILL);
        }

        // Final reap
        self.reap_workers();

        // Remove PID file
        let _ = self.remove_pid_file();

        // Remove socket file
        if self.socket_path.exists() {
            fs::remove_file(&self.socket_path)?;
        }

        Ok(())
    }

    /// Check health of all workers
    fn check_all_workers_health(&mut self) {
        let now = Instant::now();
        let mut workers_to_check = Vec::new();

        // Collect workers that need health checking
        for (session_id, handle) in &self.sessions {
            let needs_check = match handle.last_ping_attempt {
                None => true, // Never checked
                Some(last_ping) => now.duration_since(last_ping) >= HEALTH_CHECK_INTERVAL,
            };

            if needs_check {
                workers_to_check.push(*session_id);
            }
        }

        // Check each worker's health
        for session_id in workers_to_check {
            if let Err(e) = self.check_worker_health(session_id) {
                eprintln!("Health check failed for worker {}: {}", session_id, e);
            }
        }
    }

    /// Check the health of a specific worker
    fn check_worker_health(&mut self, session_id: SessionId) -> Result<()> {
        let handle = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        let now = Instant::now();
        handle.last_ping_attempt = Some(now);

        // First check: Is the process still alive?
        match waitpid(handle.worker_pid, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::Exited(_, code)) => {
                eprintln!("Worker {} exited with code {}", session_id, code);
                handle.health_state = WorkerHealthState::Crashed;
                handle.metrics.requests_failed += 1;

                // Try to respawn if under limit
                if handle.metrics.respawn_count < MAX_RESPAWN_ATTEMPTS {
                    let time_since_spawn = now.duration_since(handle.metrics.spawn_time);
                    if time_since_spawn >= RESPAWN_COOLDOWN {
                        eprintln!(
                            "Respawning worker {} (attempt {})",
                            session_id,
                            handle.metrics.respawn_count + 1
                        );
                        return self.respawn_worker(session_id);
                    } else {
                        eprintln!(
                            "Worker {} crashed too soon, waiting for cooldown",
                            session_id
                        );
                    }
                }
                return Ok(());
            }
            Ok(WaitStatus::Signaled(_, sig, _)) => {
                eprintln!("Worker {} killed by signal {:?}", session_id, sig);
                handle.health_state = WorkerHealthState::Crashed;
                handle.metrics.requests_failed += 1;

                // Try to respawn
                if handle.metrics.respawn_count < MAX_RESPAWN_ATTEMPTS {
                    let time_since_spawn = now.duration_since(handle.metrics.spawn_time);
                    if time_since_spawn >= RESPAWN_COOLDOWN {
                        eprintln!("Respawning worker {} after signal", session_id);
                        return self.respawn_worker(session_id);
                    }
                }
                return Ok(());
            }
            Ok(WaitStatus::StillAlive) => {
                // Process is alive, continue with health checks
            }
            _ => {
                // Other states, assume still alive
            }
        }

        // Check for hung workers (last heartbeat too old)
        let time_since_heartbeat = now.duration_since(handle.metrics.last_heartbeat);

        if time_since_heartbeat > REQUEST_TIMEOUT {
            // Worker hasn't responded in a long time
            if time_since_heartbeat > REQUEST_TIMEOUT * 2 {
                // Definitely hung, kill and respawn
                eprintln!(
                    "Worker {} is hung (no heartbeat for {:?}), killing",
                    session_id, time_since_heartbeat
                );
                handle.health_state = WorkerHealthState::Hung;

                // Force kill
                let _ = signal::kill(handle.worker_pid, Signal::SIGKILL);

                // Try to respawn
                if handle.metrics.respawn_count < MAX_RESPAWN_ATTEMPTS {
                    return self.respawn_worker(session_id);
                }
            } else {
                // Mark as slow/unresponsive
                handle.health_state = WorkerHealthState::Unresponsive;
                handle.metrics.consecutive_failures += 1;
                eprintln!(
                    "Worker {} is unresponsive (no heartbeat for {:?})",
                    session_id, time_since_heartbeat
                );
            }
        } else {
            // Worker responded recently, reset consecutive failures
            if handle.health_state != WorkerHealthState::Healthy {
                eprintln!("Worker {} recovered", session_id);
                handle.health_state = WorkerHealthState::Healthy;
                handle.metrics.consecutive_failures = 0;
            }
        }

        Ok(())
    }

    /// Respawn a failed worker
    fn respawn_worker(&mut self, session_id: SessionId) -> Result<()> {
        // Remove the old worker from sessions
        let old_handle = self
            .sessions
            .remove(&session_id)
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        eprintln!(
            "Respawning worker {} (previous PID: {})",
            session_id, old_handle.worker_pid
        );

        // Note: We can't truly "respawn" a worker for an existing session since
        // workers are tied to client connections. This method is here for potential
        // future use with persistent workers or worker pools.

        // For now, we just ensure cleanup happened
        let _ = waitpid(old_handle.worker_pid, Some(WaitPidFlag::WNOHANG));

        eprintln!(
            "Worker {} removed from pool after {} respawn attempts",
            session_id, old_handle.metrics.respawn_count
        );

        Ok(())
    }

    /// Print worker health statistics
    pub fn print_health_stats(&self) {
        println!("\nWorker Health Statistics:");
        println!("Total active workers: {}", self.sessions.len());

        let mut healthy = 0;
        let mut unresponsive = 0;
        let mut slow = 0;
        let mut crashed = 0;
        let mut hung = 0;

        for handle in self.sessions.values() {
            match handle.health_state {
                WorkerHealthState::Healthy => healthy += 1,
                WorkerHealthState::Unresponsive => unresponsive += 1,
                WorkerHealthState::Slow => slow += 1,
                WorkerHealthState::Crashed => crashed += 1,
                WorkerHealthState::Hung => hung += 1,
            }
        }

        println!("  Healthy: {}", healthy);
        println!("  Unresponsive: {}", unresponsive);
        println!("  Slow: {}", slow);
        println!("  Crashed: {}", crashed);
        println!("  Hung: {}", hung);

        // Print detailed metrics
        for handle in self.sessions.values() {
            let uptime = handle.created_at.elapsed().as_secs();
            let failure_rate = if handle.metrics.requests_processed > 0 {
                (handle.metrics.requests_failed as f64 / handle.metrics.requests_processed as f64)
                    * 100.0
            } else {
                0.0
            };

            println!("\nWorker {} (PID {}):", handle.id, handle.worker_pid);
            println!("  State: {:?}", handle.health_state);
            println!("  Uptime: {}s", uptime);
            println!(
                "  Requests: {} processed, {} failed ({:.1}% failure rate)",
                handle.metrics.requests_processed, handle.metrics.requests_failed, failure_rate
            );
            println!("  Respawn count: {}", handle.metrics.respawn_count);
            println!(
                "  Consecutive failures: {}",
                handle.metrics.consecutive_failures
            );
        }
    }

    /// Get the default socket path
    pub fn default_socket_path() -> Result<PathBuf> {
        let daemon_dir = Self::create_daemon_dir()?;
        Ok(daemon_dir.join("daemon.sock"))
    }
}

impl Drop for DaemonServer {
    fn drop(&mut self) {
        // Cleanup PID file
        let _ = self.remove_pid_file();

        // Cleanup socket on drop
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_cache_new() {
        let cache = StatsCache::new();

        // Should have static built-in stats
        let builtins = cache.get_builtin_stats();
        assert!(builtins.contains_key("host"), "should have host stat");
        assert!(builtins.contains_key("os"), "should have os stat");
        assert!(builtins.contains_key("kernel"), "should have kernel stat");
        assert!(builtins.contains_key("arch"), "should have arch stat");

        // Should have dynamic built-in stats
        assert!(builtins.contains_key("uptime"), "should have uptime stat");
        assert!(builtins.contains_key("memory"), "should have memory stat");

        // Should have no custom stats initially
        let custom = cache.get_custom_stats();
        assert!(custom.is_empty(), "should have no custom stats initially");
    }

    #[test]
    fn test_stats_cache_init_custom_stats() {
        let mut cache = StatsCache::new();

        let configs = vec![
            CustomStatConfig {
                name: "test_stat".to_string(),
                command: "echo hello".to_string(),
                interval: Duration::from_secs(30),
                timeout: Duration::from_secs(2),
            },
            CustomStatConfig {
                name: "another_stat".to_string(),
                command: "echo world".to_string(),
                interval: Duration::from_secs(60),
                timeout: Duration::from_secs(5),
            },
        ];

        cache.init_custom_stats(&configs);

        // Custom stats should need updates (initialized with old timestamp)
        let needing_update = cache.get_stats_needing_update();
        assert_eq!(
            needing_update.len(),
            2,
            "both custom stats should need update"
        );
        assert!(needing_update.contains(&"test_stat".to_string()));
        assert!(needing_update.contains(&"another_stat".to_string()));
    }

    #[test]
    fn test_stats_cache_update_custom_stat() {
        let mut cache = StatsCache::new();

        let configs = vec![CustomStatConfig {
            name: "echo_test".to_string(),
            command: "echo hello".to_string(),
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(2),
        }];

        cache.init_custom_stats(&configs);

        // Update the stat
        cache.update_custom_stat("echo_test");

        // Should now have a value
        let custom = cache.get_custom_stats();
        assert_eq!(custom.get("echo_test"), Some(&"hello".to_string()));

        // Should not need update anymore (recent update)
        let needing_update = cache.get_stats_needing_update();
        assert!(!needing_update.contains(&"echo_test".to_string()));
    }

    #[test]
    fn test_stats_cache_update_builtins() {
        let mut cache = StatsCache::new();

        // Force update
        cache.update_builtins();

        let builtins = cache.get_builtin_stats();

        // Should have time and date stats
        assert!(builtins.contains_key("time"), "should have time stat");
        assert!(builtins.contains_key("date"), "should have date stat");

        // Time should be recently updated
        assert!(
            cache.seconds_since_update() < 2,
            "should be recently updated"
        );
    }

    #[test]
    fn test_custom_stat_cached_error_handling() {
        let mut cache = StatsCache::new();

        let configs = vec![CustomStatConfig {
            name: "bad_command".to_string(),
            command: "this_command_does_not_exist_12345".to_string(),
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(2),
        }];

        cache.init_custom_stats(&configs);
        cache.update_custom_stat("bad_command");

        // Should have empty value in get_custom_stats (errors are filtered out)
        let custom = cache.get_custom_stats();
        assert!(
            !custom.contains_key("bad_command"),
            "error stats should be filtered"
        );
    }

    #[test]
    fn test_custom_stat_first_line_only() {
        let mut cache = StatsCache::new();

        let configs = vec![CustomStatConfig {
            name: "multiline".to_string(),
            command: "echo -e 'line1\\nline2\\nline3'".to_string(),
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(2),
        }];

        cache.init_custom_stats(&configs);
        cache.update_custom_stat("multiline");

        // Should only have first line
        let custom = cache.get_custom_stats();
        let value = custom.get("multiline").unwrap();
        assert!(!value.contains('\n'), "should only have first line");
    }
}
