//! Shell event system — inspired by fish's `--on-event`, `--on-variable`,
//! and `--on-job-exit` function attributes.
//!
//! An `EventSystem` holds a registry of named listeners (shell function names)
//! that are invoked when a matching event is emitted. The executor is responsible
//! for calling `emit_event` at the right moments; `EventSystem` only tracks
//! registrations and returns which functions need to run.
//!
//! ## Supported event kinds
//!
//! | Kind               | Fired when…                                      |
//! |--------------------|--------------------------------------------------|
//! | `Named(name)`      | `emit event <name>` is called                    |
//! | `VariableSet(var)` | a shell variable named `var` is set              |
//! | `JobExit(pid)`     | a background job with the given PID exits        |
//!
//! ## Example (shell-level, future syntax)
//!
//! ```fish
//! function on_path_change --on-variable PATH
//!     echo "PATH changed to $PATH"
//! end
//!
//! function greet --on-event shell_init
//!     echo "Shell ready"
//! end
//! ```

use std::collections::HashMap;

/// The kind of event a listener subscribes to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A named event emitted via `emit event <name>` or similar.
    Named(String),
    /// Fires whenever a specific shell variable is assigned a new value.
    VariableSet(String),
    /// Fires when a background job with the given PID exits.
    JobExit(u32),
}

/// A registration binding an event kind to a shell function name.
#[derive(Clone, Debug)]
pub struct EventListener {
    /// The event this listener watches for.
    pub event: EventKind,
    /// Name of the shell function to invoke when the event fires.
    pub function: String,
}

/// Registry and dispatcher for shell events.
///
/// `EventSystem` is cloneable so it can be stored inside `Runtime`
/// without introducing lifetime complications.
#[derive(Clone, Default, Debug)]
pub struct EventSystem {
    /// `event_kind → [function_name, …]`
    listeners: HashMap<EventKind, Vec<String>>,
}

impl EventSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a shell function to be called when `event` fires.
    pub fn register(&mut self, event: EventKind, function: impl Into<String>) {
        self.listeners
            .entry(event)
            .or_default()
            .push(function.into());
    }

    /// Unregister all listeners associated with a shell function name.
    /// Called when a function is deleted (e.g. via `functions --erase`).
    pub fn unregister_function(&mut self, function: &str) {
        for handlers in self.listeners.values_mut() {
            handlers.retain(|f| f != function);
        }
        self.listeners.retain(|_, v| !v.is_empty());
    }

    /// Emit an event and return the list of shell function names that should
    /// be invoked (in registration order). The caller is responsible for
    /// actually executing those functions.
    pub fn emit_event(&self, event: &EventKind) -> Vec<String> {
        self.listeners.get(event).cloned().unwrap_or_default()
    }

    /// Convenience: emit a named event by string.
    pub fn emit_named(&self, name: &str) -> Vec<String> {
        self.emit_event(&EventKind::Named(name.to_string()))
    }

    /// Convenience: emit a variable-set event.
    pub fn emit_variable_set(&self, var_name: &str) -> Vec<String> {
        self.emit_event(&EventKind::VariableSet(var_name.to_string()))
    }

    /// Convenience: emit a job-exit event for a PID.
    pub fn emit_job_exit(&self, pid: u32) -> Vec<String> {
        self.emit_event(&EventKind::JobExit(pid))
    }

    /// Return all registered listeners as owned values.
    pub fn listeners_owned(&self) -> Vec<EventListener> {
        self.listeners
            .iter()
            .flat_map(|(event, funcs)| {
                funcs.iter().map(|f| EventListener {
                    event: event.clone(),
                    function: f.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_event_no_listeners() {
        let es = EventSystem::new();
        assert!(es.emit_named("nothing").is_empty());
    }

    #[test]
    fn test_register_and_emit_named() {
        let mut es = EventSystem::new();
        es.register(EventKind::Named("shell_init".into()), "greet");
        let triggered = es.emit_named("shell_init");
        assert_eq!(triggered, vec!["greet"]);
    }

    #[test]
    fn test_emit_variable_set() {
        let mut es = EventSystem::new();
        es.register(EventKind::VariableSet("PATH".into()), "on_path_change");
        let triggered = es.emit_variable_set("PATH");
        assert_eq!(triggered, vec!["on_path_change"]);
        assert!(es.emit_variable_set("HOME").is_empty());
    }

    #[test]
    fn test_emit_job_exit() {
        let mut es = EventSystem::new();
        es.register(EventKind::JobExit(1234), "handle_job");
        assert_eq!(es.emit_job_exit(1234), vec!["handle_job"]);
        assert!(es.emit_job_exit(9999).is_empty());
    }

    #[test]
    fn test_multiple_listeners_same_event() {
        let mut es = EventSystem::new();
        es.register(EventKind::Named("tick".into()), "handler_a");
        es.register(EventKind::Named("tick".into()), "handler_b");
        let triggered = es.emit_named("tick");
        assert!(triggered.contains(&"handler_a".to_string()));
        assert!(triggered.contains(&"handler_b".to_string()));
    }

    #[test]
    fn test_unregister_function() {
        let mut es = EventSystem::new();
        es.register(EventKind::Named("ev".into()), "fn_a");
        es.register(EventKind::Named("ev".into()), "fn_b");
        es.unregister_function("fn_a");
        let triggered = es.emit_named("ev");
        assert!(!triggered.contains(&"fn_a".to_string()));
        assert!(triggered.contains(&"fn_b".to_string()));
    }

    #[test]
    fn test_listeners_owned() {
        let mut es = EventSystem::new();
        es.register(EventKind::Named("x".into()), "f");
        let all = es.listeners_owned();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].function, "f");
    }
}
