//! Shell hook management for precmd and preexec callbacks.
//!
//! Hooks are user-defined callbacks that run at specific points in the
//! command execution lifecycle:
//!
//! - `precmd`: runs after each command completes, before the next prompt is shown.
//!   Hook functions receive the last exit code as `$1` and elapsed milliseconds as `$2`.
//! - `preexec`: runs just before a command is executed (after parsing, before exec).
//!   Hook functions receive the command string as `$1`.
//!
//! # Registration
//!
//! Hooks are auto-discovered by well-known names:
//! - Define a function named `rush_precmd` — it will run before every prompt.
//! - Define a function named `rush_preexec` — it will run before every command.
//!
//! Additional functions can be registered explicitly via [`HookManager::add_hook`].
//!
//! # Example (in .rushrc)
//!
//! ```sh
//! function rush_precmd() {
//!     echo "Last exit: $1, took ${2}ms"
//! }
//!
//! function rush_preexec() {
//!     echo "About to run: $1"
//! }
//! ```

/// The type of shell hook event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShellHook {
    /// Fires after each command completes, before showing the next prompt.
    ///
    /// Arguments passed to hook functions:
    /// - `$1`: last exit code (integer)
    /// - `$2`: elapsed time in milliseconds (integer)
    Precmd,

    /// Fires just before a command is executed.
    ///
    /// Arguments passed to hook functions:
    /// - `$1`: the full command string as entered by the user
    Preexec,
}

/// Manages registered shell hook functions.
///
/// Functions are looked up by name at call time, so they can be defined or
/// removed at any point in the session. The well-known names `rush_precmd`
/// and `rush_preexec` are always checked first; additional names may be
/// registered with [`add_hook`].
#[derive(Debug, Clone, Default)]
pub struct HookManager {
    /// Functions registered for the precmd event (beyond the auto-discovered name).
    precmd_hooks: Vec<String>,
    /// Functions registered for the preexec event (beyond the auto-discovered name).
    preexec_hooks: Vec<String>,
}

impl HookManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a function name as a hook for the given event.
    ///
    /// Duplicate registrations are silently ignored.
    pub fn add_hook(&mut self, hook: ShellHook, function_name: String) {
        match hook {
            ShellHook::Precmd => {
                if !self.precmd_hooks.contains(&function_name) {
                    self.precmd_hooks.push(function_name);
                }
            }
            ShellHook::Preexec => {
                if !self.preexec_hooks.contains(&function_name) {
                    self.preexec_hooks.push(function_name);
                }
            }
        }
    }

    /// Unregister a hook function.
    pub fn remove_hook(&mut self, hook: &ShellHook, function_name: &str) {
        match hook {
            ShellHook::Precmd => self.precmd_hooks.retain(|f| f != function_name),
            ShellHook::Preexec => self.preexec_hooks.retain(|f| f != function_name),
        }
    }

    /// Returns all function names that should be called for the precmd event.
    ///
    /// The well-known name `rush_precmd` is always included first.
    pub fn precmd_functions(&self) -> Vec<String> {
        let mut fns = vec!["rush_precmd".to_string()];
        for f in &self.precmd_hooks {
            if f != "rush_precmd" {
                fns.push(f.clone());
            }
        }
        fns
    }

    /// Returns all function names that should be called for the preexec event.
    ///
    /// The well-known name `rush_preexec` is always included first.
    pub fn preexec_functions(&self) -> Vec<String> {
        let mut fns = vec!["rush_preexec".to_string()];
        for f in &self.preexec_hooks {
            if f != "rush_preexec" {
                fns.push(f.clone());
            }
        }
        fns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precmd_always_includes_rush_precmd() {
        let mgr = HookManager::new();
        let fns = mgr.precmd_functions();
        assert!(fns.contains(&"rush_precmd".to_string()));
    }

    #[test]
    fn test_preexec_always_includes_rush_preexec() {
        let mgr = HookManager::new();
        let fns = mgr.preexec_functions();
        assert!(fns.contains(&"rush_preexec".to_string()));
    }

    #[test]
    fn test_add_hook_registers_function() {
        let mut mgr = HookManager::new();
        mgr.add_hook(ShellHook::Precmd, "my_precmd".to_string());
        assert!(mgr.precmd_functions().contains(&"my_precmd".to_string()));
    }

    #[test]
    fn test_add_hook_no_duplicate() {
        let mut mgr = HookManager::new();
        mgr.add_hook(ShellHook::Precmd, "my_hook".to_string());
        mgr.add_hook(ShellHook::Precmd, "my_hook".to_string());
        let count = mgr.precmd_functions().iter().filter(|f| f.as_str() == "my_hook").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_remove_hook() {
        let mut mgr = HookManager::new();
        mgr.add_hook(ShellHook::Preexec, "my_preexec".to_string());
        mgr.remove_hook(&ShellHook::Preexec, "my_preexec");
        assert!(!mgr.preexec_functions().contains(&"my_preexec".to_string()));
    }

    #[test]
    fn test_remove_well_known_name_not_in_extra_list() {
        let mut mgr = HookManager::new();
        // rush_precmd is always included from the fixed list, not the extras list,
        // so removing it from extras has no effect on the output.
        mgr.remove_hook(&ShellHook::Precmd, "rush_precmd");
        assert!(mgr.precmd_functions().contains(&"rush_precmd".to_string()));
    }
}
