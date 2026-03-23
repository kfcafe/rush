//! Trap and redirect management for the shell runtime.
//!
//! Handles signal trap handlers and permanent file descriptor redirections
//! set by the exec builtin.

use crate::builtins::trap::TrapSignal;
use std::collections::HashMap;

use super::Runtime;

impl Runtime {
    // Permanent file descriptor redirection management (for exec builtin)

    /// Set permanent stdout redirection file descriptor
    pub fn set_permanent_stdout(&mut self, fd: Option<i32>) {
        self.permanent_stdout = fd;
    }

    /// Get permanent stdout redirection file descriptor
    pub fn get_permanent_stdout(&self) -> Option<i32> {
        self.permanent_stdout
    }

    /// Set permanent stderr redirection file descriptor
    pub fn set_permanent_stderr(&mut self, fd: Option<i32>) {
        self.permanent_stderr = fd;
    }

    /// Get permanent stderr redirection file descriptor
    pub fn get_permanent_stderr(&self) -> Option<i32> {
        self.permanent_stderr
    }

    /// Set permanent stdin redirection file descriptor
    pub fn set_permanent_stdin(&mut self, fd: Option<i32>) {
        self.permanent_stdin = fd;
    }

    /// Get permanent stdin redirection file descriptor
    pub fn get_permanent_stdin(&self) -> Option<i32> {
        self.permanent_stdin
    }

    // Trap handler management

    /// Set a trap handler for a signal
    pub fn set_trap(&mut self, signal: TrapSignal, command: String) {
        self.trap_handlers.set(signal, command);
    }

    /// Remove a trap handler for a signal
    pub fn remove_trap(&mut self, signal: TrapSignal) {
        self.trap_handlers.remove(signal);
    }

    /// Get the trap handler for a signal
    pub fn get_trap(&self, signal: TrapSignal) -> Option<&String> {
        self.trap_handlers.get(signal)
    }

    /// Get all trap handlers
    pub fn get_all_traps(&self) -> &HashMap<TrapSignal, String> {
        self.trap_handlers.all()
    }

    /// Check if a signal has a trap handler
    pub fn has_trap(&self, signal: TrapSignal) -> bool {
        self.trap_handlers.has_handler(signal)
    }
}
