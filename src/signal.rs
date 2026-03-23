use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use signal_hook::consts::{SIGCHLD, SIGHUP, SIGINT, SIGTERM, SIGTSTP};
use signal_hook::iterator::Signals;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::thread;

/// Global flag indicating if a signal was received
static SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);

/// The signal that was received (0 if none)
pub static SIGNAL_NUMBER: AtomicI32 = AtomicI32::new(0);

/// Flag indicating if a terminal stop signal was received
pub static TERMINAL_STOP: AtomicBool = AtomicBool::new(false);

/// Global flag for SIGINT received (separate from shutdown to allow interrupt without exit)
static SIGINT_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Counter for consecutive SIGINT presses
static SIGINT_COUNT: AtomicI32 = AtomicI32::new(0);

/// Process group ID of the foreground job (0 if none)
static FOREGROUND_PGID: AtomicI32 = AtomicI32::new(0);

/// Signal handler state shared between main thread and signal handler thread
#[derive(Clone)]
pub struct SignalHandler {
    shutdown_flag: Arc<AtomicBool>,
    sigchld_flag: Arc<AtomicBool>,
    interactive_mode: Arc<AtomicBool>,
}

impl SignalHandler {
    /// Create a new signal handler
    pub fn new() -> Self {
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            sigchld_flag: Arc::new(AtomicBool::new(false)),
            interactive_mode: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new signal handler for interactive mode
    pub fn new_interactive() -> Self {
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            sigchld_flag: Arc::new(AtomicBool::new(false)),
            interactive_mode: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Set interactive mode
    pub fn set_interactive(&self, interactive: bool) {
        self.interactive_mode.store(interactive, Ordering::SeqCst);
    }

    /// Check if in interactive mode
    pub fn is_interactive(&self) -> bool {
        self.interactive_mode.load(Ordering::SeqCst)
    }

    /// Setup signal handlers for SIGINT, SIGTERM, SIGHUP, and SIGCHLD
    /// Note: SIGTTIN/SIGTTOU are ignored by the shell (set in main.rs) so we don't handle them here
    pub fn setup(&self) -> Result<()> {
        let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP, SIGCHLD, SIGTSTP])?;
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let sigchld_flag = Arc::clone(&self.sigchld_flag);
        let interactive_mode = Arc::clone(&self.interactive_mode);

        thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGINT => {
                        SIGNAL_RECEIVED.store(true, Ordering::SeqCst);
                        SIGNAL_NUMBER.store(SIGINT, Ordering::SeqCst);
                        SIGINT_RECEIVED.store(true, Ordering::SeqCst);
                        SIGINT_COUNT.fetch_add(1, Ordering::SeqCst);

                        // Forward signal to foreground process group if any
                        let fg_pgid = FOREGROUND_PGID.load(Ordering::SeqCst);
                        if fg_pgid > 0 {
                            // Send SIGINT to the process group (negative PID)
                            let _ = kill(Pid::from_raw(-fg_pgid), Signal::SIGINT);
                        }

                        // In interactive mode, don't set shutdown flag on SIGINT
                        // (shell should stay alive, just interrupt current command)
                        if !interactive_mode.load(Ordering::SeqCst) {
                            shutdown_flag.store(true, Ordering::SeqCst);
                        }
                    }
                    SIGTERM => {
                        SIGNAL_RECEIVED.store(true, Ordering::SeqCst);
                        SIGNAL_NUMBER.store(SIGTERM, Ordering::SeqCst);
                        shutdown_flag.store(true, Ordering::SeqCst);
                    }
                    SIGHUP => {
                        SIGNAL_RECEIVED.store(true, Ordering::SeqCst);
                        SIGNAL_NUMBER.store(SIGHUP, Ordering::SeqCst);
                        shutdown_flag.store(true, Ordering::SeqCst);
                    }
                    SIGTSTP => {
                        // Terminal stop signal (Ctrl+Z)
                        SIGNAL_RECEIVED.store(true, Ordering::SeqCst);
                        SIGNAL_NUMBER.store(SIGTSTP, Ordering::SeqCst);
                        TERMINAL_STOP.store(true, Ordering::SeqCst);
                    }
                    // Note: SIGTTIN/SIGTTOU are ignored by the shell (SIG_IGN in main.rs)
                    SIGCHLD => {
                        // Set flag to indicate a child process has changed state
                        sigchld_flag.store(true, Ordering::SeqCst);
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// Check if a signal was received
    pub fn signal_received(&self) -> bool {
        SIGNAL_RECEIVED.load(Ordering::SeqCst)
    }

    /// Get the signal number that was received
    pub fn signal_number(&self) -> i32 {
        SIGNAL_NUMBER.load(Ordering::SeqCst)
    }

    /// Check if shutdown was requested
    pub fn should_shutdown(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    /// Check if SIGCHLD was received
    pub fn sigchld_received(&self) -> bool {
        self.sigchld_flag.load(Ordering::SeqCst)
    }

    /// Clear the SIGCHLD flag
    pub fn clear_sigchld(&self) {
        self.sigchld_flag.store(false, Ordering::SeqCst);
    }

    /// Check if SIGINT was received
    pub fn sigint_received(&self) -> bool {
        SIGINT_RECEIVED.load(Ordering::SeqCst)
    }

    /// Get the SIGINT count (for detecting multiple Ctrl-C presses)
    pub fn sigint_count(&self) -> i32 {
        SIGINT_COUNT.load(Ordering::SeqCst)
    }

    /// Reset the SIGINT count
    pub fn reset_sigint_count(&self) {
        SIGINT_COUNT.store(0, Ordering::SeqCst);
    }

    /// Set the foreground process group ID
    pub fn set_foreground_pgid(&self, pgid: i32) {
        FOREGROUND_PGID.store(pgid, Ordering::SeqCst);
    }

    /// Clear the foreground process group ID
    pub fn clear_foreground_pgid(&self) {
        FOREGROUND_PGID.store(0, Ordering::SeqCst);
    }

    /// Get the foreground process group ID
    pub fn foreground_pgid(&self) -> i32 {
        FOREGROUND_PGID.load(Ordering::SeqCst)
    }

    /// Clear the SIGINT flag
    pub fn clear_sigint(&self) {
        SIGINT_RECEIVED.store(false, Ordering::SeqCst);
        SIGNAL_RECEIVED.store(false, Ordering::SeqCst);
        SIGNAL_NUMBER.store(0, Ordering::SeqCst);
    }

    /// Reset the signal state
    pub fn reset(&self) {
        SIGNAL_RECEIVED.store(false, Ordering::SeqCst);
        SIGNAL_NUMBER.store(0, Ordering::SeqCst);
        TERMINAL_STOP.store(false, Ordering::SeqCst);
        SIGINT_RECEIVED.store(false, Ordering::SeqCst);
        self.shutdown_flag.store(false, Ordering::SeqCst);
    }

    /// Check if a terminal stop signal was received
    pub fn terminal_stop(&self) -> bool {
        TERMINAL_STOP.load(Ordering::SeqCst)
    }

    /// Get the exit code for the received signal
    pub fn exit_code(&self) -> i32 {
        match self.signal_number() {
            SIGINT => 130,  // Standard exit code for SIGINT (128 + 2)
            SIGTERM => 143, // Standard exit code for SIGTERM (128 + 15)
            SIGHUP => 129,  // Standard exit code for SIGHUP (128 + 1)
            SIGTSTP => 148, // Standard exit code for SIGTSTP (128 + 20)
            // Note: SIGTTIN/SIGTTOU are ignored by the shell, so they never reach here
            _ => 1,
        }
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Global signal checking function
pub fn signal_received() -> bool {
    SIGNAL_RECEIVED.load(Ordering::SeqCst)
}

/// Get the current signal number
pub fn signal_number() -> i32 {
    SIGNAL_NUMBER.load(Ordering::SeqCst)
}

/// Check if SIGINT was received
pub fn sigint_received() -> bool {
    SIGINT_RECEIVED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_handler_creation() {
        let handler = SignalHandler::new();
        assert!(!handler.should_shutdown());
        assert!(!handler.signal_received());
        assert_eq!(handler.signal_number(), 0);
        assert!(!handler.is_interactive());
    }

    #[test]
    fn test_signal_handler_interactive() {
        let handler = SignalHandler::new_interactive();
        assert!(handler.is_interactive());

        handler.set_interactive(false);
        assert!(!handler.is_interactive());
    }

    #[test]
    fn test_signal_handler_setup() {
        let handler = SignalHandler::new();
        assert!(handler.setup().is_ok());
    }

    #[test]
    fn test_signal_handler_reset() {
        let handler = SignalHandler::new();
        handler.reset();
        assert!(!handler.signal_received());
        assert_eq!(handler.signal_number(), 0);
        assert!(!handler.sigint_received());
    }

    #[test]
    fn test_exit_codes() {
        let handler = SignalHandler::new();

        // Set SIGINT and check exit code
        SIGNAL_NUMBER.store(SIGINT, Ordering::SeqCst);
        assert_eq!(handler.exit_code(), 130);

        // Set SIGTERM and check exit code
        SIGNAL_NUMBER.store(SIGTERM, Ordering::SeqCst);
        assert_eq!(handler.exit_code(), 143);

        // Set SIGHUP and check exit code
        SIGNAL_NUMBER.store(SIGHUP, Ordering::SeqCst);
        assert_eq!(handler.exit_code(), 129);

        // Reset
        handler.reset();
    }

    #[test]
    fn test_sigint_handling() {
        // Verify the handler can be created and setup
        // Full integration testing is done in tests/signal_handling_tests.rs
        let handler = SignalHandler::new();
        assert!(handler.setup().is_ok());
    }

    #[test]
    fn test_foreground_pgid() {
        let handler = SignalHandler::new();
        assert_eq!(handler.foreground_pgid(), 0);

        handler.set_foreground_pgid(1234);
        assert_eq!(handler.foreground_pgid(), 1234);

        handler.clear_foreground_pgid();
        assert_eq!(handler.foreground_pgid(), 0);
    }

    #[test]
    fn test_sigint_count() {
        let handler = SignalHandler::new();
        handler.reset();

        assert_eq!(handler.sigint_count(), 0);

        // Manual increment for testing
        SIGINT_COUNT.fetch_add(1, Ordering::SeqCst);
        assert_eq!(handler.sigint_count(), 1);

        handler.reset_sigint_count();
        assert_eq!(handler.sigint_count(), 0);
    }
}
