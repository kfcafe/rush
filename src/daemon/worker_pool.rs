//! Worker pool implementation for persistent session workers
//!
//! Replaces fork-per-request with a pool of long-lived workers to eliminate
//! process creation overhead (~2-3ms per request).
//!
//! ## Architecture
//! - Pre-spawn N workers at daemon startup
//! - Workers listen on Unix socket pairs for requests
//! - Dispatch requests to available workers (O(1) selection)
//! - Workers reset state between requests
//! - Monitor worker health and auto-respawn

use crate::daemon::protocol::{read_message, write_message, ExecutionResult, Message, SessionInit};
use anyhow::{anyhow, Result};
use nix::unistd::{fork, ForkResult, Pid};
use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Default number of workers in the pool
pub const DEFAULT_POOL_SIZE: usize = 4;

/// Maximum queued requests before backpressure
pub const MAX_QUEUED_REQUESTS: usize = 100;

/// Worker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    /// Worker is available and ready to handle requests
    Available,
    /// Worker is currently processing a request
    Busy,
    /// Worker has crashed or failed health check
    Dead,
}

/// A persistent worker process
pub struct Worker {
    /// Worker process ID
    pub pid: Pid,
    /// Unix socket stream to communicate with worker
    pub channel: UnixStream,
    /// Current state of the worker
    pub state: WorkerState,
    /// When the worker was created
    pub created_at: Instant,
    /// Number of requests processed by this worker
    pub requests_processed: u64,
}

impl Worker {
    /// Spawn a new worker process
    pub fn spawn() -> Result<Self> {
        // Create a Unix socket pair for parent-worker communication
        let (parent_stream, worker_stream) = UnixStream::pair()?;

        // Fork the worker process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: Return worker handle
                // Close worker's end of the socket
                drop(worker_stream);

                Ok(Worker {
                    pid: child,
                    channel: parent_stream,
                    state: WorkerState::Available,
                    created_at: Instant::now(),
                    requests_processed: 0,
                })
            }
            Ok(ForkResult::Child) => {
                // Child: Become worker process
                // Close parent's end of the socket
                drop(parent_stream);

                // Run the worker loop (this never returns)
                Self::worker_loop(worker_stream);
            }
            Err(e) => Err(anyhow!("Failed to fork worker: {}", e)),
        }
    }

    /// Worker process loop (runs in child process)
    ///
    /// Creates a single Executor instance and reuses it across requests,
    /// calling reset() between commands to clear transient state while
    /// preserving long-lived resources (history, job_manager, builtins, corrector).
    fn worker_loop(mut stream: UnixStream) -> ! {
        // Create executor ONCE -- reuse across all requests
        let mut executor = crate::executor::Executor::new_embedded();

        loop {
            // Wait for a request from the pool
            let (msg, msg_id) = match read_message(&mut stream) {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Worker: Failed to read request: {}", e);
                    std::process::exit(1);
                }
            };

            // Handle the request
            match msg {
                Message::SessionInit(init) => {
                    // Reset executor state before each request to prevent leakage
                    if let Err(e) = executor.reset() {
                        eprintln!("Worker: Failed to reset executor: {}", e);
                    }

                    // Execute the session with the reused executor
                    match Self::execute_session_with_executor(&init, &mut executor) {
                        Ok(exec_result) => {
                            // Send result back to pool
                            let result = ExecutionResult {
                                exit_code: exec_result.exit_code,
                                stdout_len: exec_result.stdout().len() as u64,
                                stderr_len: exec_result.stderr.len() as u64,
                                stdout: exec_result.stdout(),
                                stderr: exec_result.stderr,
                            };

                            if let Err(e) = write_message(
                                &mut stream,
                                &Message::ExecutionResult(result),
                                msg_id,
                            ) {
                                eprintln!("Worker: Failed to send result: {}", e);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Worker: Execution error: {}", e);

                            // Send error result
                            let error_result = ExecutionResult {
                                exit_code: 1,
                                stdout_len: 0,
                                stderr_len: e.to_string().len() as u64,
                                stdout: String::new(),
                                stderr: e.to_string(),
                            };

                            if let Err(e) = write_message(
                                &mut stream,
                                &Message::ExecutionResult(error_result),
                                msg_id,
                            ) {
                                eprintln!("Worker: Failed to send error result: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Message::Shutdown(_) => {
                    // Graceful shutdown
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("Worker: Unexpected message type");
                }
            };
        }
    }

    /// Execute a session command using a reusable executor instance.
    ///
    /// The executor should already be reset before calling this method.
    fn execute_session_with_executor(
        init: &SessionInit,
        executor: &mut crate::executor::Executor,
    ) -> Result<crate::executor::ExecutionResult> {
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
        if init.args.len() >= 2 && init.args[0] == "-c" {
            let command = &init.args[1];

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

    /// Execute a session command (creates a new executor per call).
    /// Kept for backward compatibility; prefer execute_session_with_executor.
    fn execute_session(init: &SessionInit) -> Result<crate::executor::ExecutionResult> {
        let mut executor = crate::executor::Executor::new_embedded();
        Self::execute_session_with_executor(init, &mut executor)
    }

    /// Check if worker is alive and responsive
    pub fn is_alive(&self) -> bool {
        use nix::sys::signal::kill;

        // Send signal 0 (None) to check if process exists
        match kill(self.pid, None) {
            Ok(_) => true,
            Err(nix::errno::Errno::ESRCH) => false, // No such process
            Err(_) => true,                         // Other errors (e.g., permission) assume alive
        }
    }
}

/// Pool of persistent worker processes
pub struct WorkerPool {
    /// All workers in the pool (both available and busy)
    workers: Vec<Worker>,
    /// Queue of available worker indices (for O(1) dispatch)
    available: Arc<Mutex<VecDeque<usize>>>,
    /// Queue of pending requests when all workers are busy
    request_queue: Arc<Mutex<VecDeque<PendingRequest>>>,
    /// Pool configuration
    config: PoolConfig,
}

/// Configuration for the worker pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Number of workers to maintain
    pub pool_size: usize,
    /// Maximum queued requests before rejecting
    pub max_queue_size: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            pool_size: DEFAULT_POOL_SIZE,
            max_queue_size: MAX_QUEUED_REQUESTS,
        }
    }
}

/// A request waiting for an available worker
#[derive(Debug)]
struct PendingRequest {
    session_init: SessionInit,
    msg_id: u32,
    client_stream: UnixStream,
    queued_at: Instant,
}

impl WorkerPool {
    /// Create a new worker pool and spawn workers
    pub fn new(config: PoolConfig) -> Result<Self> {
        let mut workers = Vec::with_capacity(config.pool_size);
        let mut available = VecDeque::with_capacity(config.pool_size);

        // Spawn all workers
        for i in 0..config.pool_size {
            match Worker::spawn() {
                Ok(worker) => {
                    workers.push(worker);
                    available.push_back(i); // All workers start available
                }
                Err(e) => {
                    eprintln!("Warning: Failed to spawn worker {}: {}", i, e);
                    // Continue with fewer workers
                }
            }
        }

        if workers.is_empty() {
            return Err(anyhow!("Failed to spawn any workers"));
        }

        Ok(Self {
            workers,
            available: Arc::new(Mutex::new(available)),
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
        })
    }

    /// Dispatch a request to an available worker (O(1) selection)
    ///
    /// Returns immediately with the result or queues the request if all workers are busy.
    pub fn dispatch_request(
        &mut self,
        session_init: SessionInit,
        msg_id: u32,
        client_stream: UnixStream,
    ) -> Result<()> {
        // Try to get an available worker
        let worker_idx = {
            let mut available = self.available.lock().unwrap_or_else(|e| {
                eprintln!("Warning: available queue mutex poisoned, recovering");
                e.into_inner()
            });
            available.pop_front() // O(1) queue operation
        };

        match worker_idx {
            Some(idx) => {
                // Worker available, dispatch immediately
                self.dispatch_to_worker(idx, session_init, msg_id, client_stream)?;
                Ok(())
            }
            None => {
                // All workers busy, queue the request
                let mut queue = self.request_queue.lock().unwrap_or_else(|e| {
                    eprintln!("Warning: request queue mutex poisoned, recovering");
                    e.into_inner()
                });

                if queue.len() >= self.config.max_queue_size {
                    // Queue full - backpressure
                    return Err(anyhow!(
                        "Worker pool overloaded: {} requests queued (max: {})",
                        queue.len(),
                        self.config.max_queue_size
                    ));
                }

                queue.push_back(PendingRequest {
                    session_init,
                    msg_id,
                    client_stream,
                    queued_at: Instant::now(),
                });

                Ok(())
            }
        }
    }

    /// Dispatch a request to a specific worker
    fn dispatch_to_worker(
        &mut self,
        worker_idx: usize,
        session_init: SessionInit,
        msg_id: u32,
        client_stream: UnixStream,
    ) -> Result<()> {
        let worker = &mut self.workers[worker_idx];

        // Mark worker as busy
        worker.state = WorkerState::Busy;

        // Send request to worker
        let msg = Message::SessionInit(session_init);
        write_message(&mut worker.channel, &msg, msg_id)
            .map_err(|e| anyhow!("Failed to send request to worker: {}", e))?;

        // Read result from worker (blocking)
        let (response, response_msg_id) = read_message(&mut worker.channel)
            .map_err(|e| anyhow!("Failed to read response from worker: {}", e))?;

        // Forward result to client
        let mut client = client_stream;
        write_message(&mut client, &response, response_msg_id)
            .map_err(|e| anyhow!("Failed to forward response to client: {}", e))?;

        // Mark worker as available again
        worker.state = WorkerState::Available;
        worker.requests_processed += 1;

        // Return worker to available queue
        {
            let mut available = self.available.lock().unwrap_or_else(|e| {
                eprintln!("Warning: available queue mutex poisoned, recovering");
                e.into_inner()
            });
            available.push_back(worker_idx); // Load balancing via round-robin
        }

        // Process next queued request if any
        self.process_next_queued_request()?;

        Ok(())
    }

    /// Process the next queued request if workers are available
    fn process_next_queued_request(&mut self) -> Result<()> {
        // Check if there are queued requests
        let pending = {
            let mut queue = self.request_queue.lock().unwrap_or_else(|e| {
                eprintln!("Warning: request queue mutex poisoned, recovering");
                e.into_inner()
            });
            queue.pop_front()
        };

        if let Some(req) = pending {
            // Recursively dispatch (will either execute or re-queue)
            self.dispatch_request(req.session_init, req.msg_id, req.client_stream)?;
        }

        Ok(())
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let available = self.available.lock().unwrap_or_else(|e| {
            eprintln!("Warning: available queue mutex poisoned, recovering");
            e.into_inner()
        });
        let queue = self.request_queue.lock().unwrap_or_else(|e| {
            eprintln!("Warning: request queue mutex poisoned, recovering");
            e.into_inner()
        });

        PoolStats {
            total_workers: self.workers.len(),
            available_workers: available.len(),
            busy_workers: self.workers.len() - available.len(),
            queued_requests: queue.len(),
            total_requests_processed: self.workers.iter().map(|w| w.requests_processed).sum(),
        }
    }

    /// Shutdown all workers gracefully
    pub fn shutdown(&mut self) -> Result<()> {
        use nix::sys::signal::{kill, Signal};

        for worker in &self.workers {
            // Try to send shutdown message
            let _ = write_message(
                &mut worker.channel.try_clone()?,
                &Message::Shutdown(crate::daemon::protocol::Shutdown { force: false }),
                0,
            );

            // Send SIGTERM
            let _ = kill(worker.pid, Signal::SIGTERM);
        }

        // Wait briefly for graceful shutdown
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Force kill any remaining workers
        for worker in &self.workers {
            if worker.is_alive() {
                let _ = kill(worker.pid, Signal::SIGKILL);
            }
        }

        Ok(())
    }
}

/// Worker pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_workers: usize,
    pub available_workers: usize,
    pub busy_workers: usize,
    pub queued_requests: usize,
    pub total_requests_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_spawn() {
        let worker = Worker::spawn().expect("Failed to spawn worker");
        assert!(worker.is_alive());
        assert_eq!(worker.state, WorkerState::Available);
        assert_eq!(worker.requests_processed, 0);
    }

    #[test]
    fn test_pool_creation() {
        let config = PoolConfig {
            pool_size: 2,
            max_queue_size: 10,
        };

        let pool = WorkerPool::new(config).expect("Failed to create pool");
        let stats = pool.stats();

        assert_eq!(stats.total_workers, 2);
        assert_eq!(stats.available_workers, 2);
        assert_eq!(stats.busy_workers, 0);
        assert_eq!(stats.queued_requests, 0);
    }

    #[test]
    fn test_pool_stats() {
        let config = PoolConfig::default();
        let pool = WorkerPool::new(config).expect("Failed to create pool");
        let stats = pool.stats();

        assert!(stats.total_workers > 0);
        assert_eq!(stats.available_workers, stats.total_workers);
        assert_eq!(stats.busy_workers, 0);
    }
}
