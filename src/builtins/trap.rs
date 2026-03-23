use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// Signal types that can be trapped
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrapSignal {
    /// SIGINT (Ctrl-C)
    Int,
    /// SIGTERM (termination)
    Term,
    /// SIGHUP (hangup)
    Hup,
    /// EXIT - special trap that runs on shell exit
    Exit,
    /// ERR - special trap that runs when a command fails
    Err,
}

impl TrapSignal {
    /// Parse a signal name or number into a TrapSignal
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "INT" | "SIGINT" | "2" => Ok(TrapSignal::Int),
            "TERM" | "SIGTERM" | "15" => Ok(TrapSignal::Term),
            "HUP" | "SIGHUP" | "1" => Ok(TrapSignal::Hup),
            "EXIT" | "0" => Ok(TrapSignal::Exit),
            "ERR" => Ok(TrapSignal::Err),
            _ => Err(anyhow!("trap: invalid signal specification: {}", s)),
        }
    }

    /// Convert signal to string representation
    pub fn to_string(&self) -> &'static str {
        match self {
            TrapSignal::Int => "INT",
            TrapSignal::Term => "TERM",
            TrapSignal::Hup => "HUP",
            TrapSignal::Exit => "EXIT",
            TrapSignal::Err => "ERR",
        }
    }

    /// Convert signal to signal number (for real signals)
    pub fn to_signal_number(&self) -> Option<i32> {
        match self {
            TrapSignal::Int => Some(2),
            TrapSignal::Term => Some(15),
            TrapSignal::Hup => Some(1),
            TrapSignal::Exit => None,
            TrapSignal::Err => None,
        }
    }
}

/// Trap handler storage
#[derive(Clone, Default)]
pub struct TrapHandlers {
    handlers: HashMap<TrapSignal, String>,
}

impl TrapHandlers {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Set a trap handler for a signal
    pub fn set(&mut self, signal: TrapSignal, command: String) {
        self.handlers.insert(signal, command);
    }

    /// Remove a trap handler for a signal
    pub fn remove(&mut self, signal: TrapSignal) {
        self.handlers.remove(&signal);
    }

    /// Get the trap handler for a signal
    pub fn get(&self, signal: TrapSignal) -> Option<&String> {
        self.handlers.get(&signal)
    }

    /// Get all trap handlers
    pub fn all(&self) -> &HashMap<TrapSignal, String> {
        &self.handlers
    }

    /// Check if a signal has a trap handler
    pub fn has_handler(&self, signal: TrapSignal) -> bool {
        self.handlers.contains_key(&signal)
    }
}

/// Implement the `trap` builtin command
///
/// Usage:
/// - `trap` - List all traps
/// - `trap 'command' SIGNAL` - Set trap handler
/// - `trap - SIGNAL` - Remove trap handler
/// - `trap '' SIGNAL` - Ignore signal
///
/// Examples:
/// ```bash
/// trap 'echo "Cleaning up..."; rm -f /tmp/$$-*' EXIT
/// trap 'echo "Interrupted!"; exit 1' INT
/// trap - INT  # Remove INT trap
/// trap  # List all traps
/// ```
pub fn builtin_trap(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // No args: list all traps
    if args.is_empty() {
        return list_traps(runtime);
    }

    // Special flags that work with single argument
    if args[0] == "-l" {
        return list_signals();
    }

    if args[0] == "-p" {
        return list_traps(runtime);
    }

    // Single arg (that isn't a flag): invalid syntax
    if args.len() == 1 {
        return Err(anyhow!("trap: usage: trap [-lp] [arg signal_spec ...]"));
    }

    // Parse: trap 'command' SIGNAL [SIGNAL ...]
    let command = &args[0];
    let signals = &args[1..];

    // Check if we're removing traps
    let removing = command == "-";

    for signal_spec in signals {
        let signal = TrapSignal::from_str(signal_spec)?;

        if removing {
            // Remove trap
            runtime.remove_trap(signal);
        } else if command.is_empty() {
            // Empty command means ignore signal
            runtime.set_trap(signal, String::new());
        } else {
            // Set trap handler
            runtime.set_trap(signal, command.clone());
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

/// List all currently set traps
fn list_traps(runtime: &Runtime) -> Result<ExecutionResult> {
    let traps = runtime.get_all_traps();

    if traps.is_empty() {
        return Ok(ExecutionResult::success(String::new()));
    }

    let mut output = String::new();

    // Sort by signal name for consistent output
    let mut trap_list: Vec<_> = traps.iter().collect();
    trap_list.sort_by_key(|(sig, _)| sig.to_string());

    for (signal, command) in trap_list {
        if command.is_empty() {
            output.push_str(&format!("trap -- '' {}\n", signal.to_string()));
        } else {
            output.push_str(&format!("trap -- '{}' {}\n", command, signal.to_string()));
        }
    }

    Ok(ExecutionResult::success(output))
}

/// List all available signals
fn list_signals() -> Result<ExecutionResult> {
    let signals = vec!["INT (2)", "TERM (15)", "HUP (1)", "EXIT (0)", "ERR"];

    let mut output = String::new();
    for signal in signals {
        output.push_str(signal);
        output.push('\n');
    }

    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_parsing() {
        assert_eq!(TrapSignal::from_str("INT").unwrap(), TrapSignal::Int);
        assert_eq!(TrapSignal::from_str("SIGINT").unwrap(), TrapSignal::Int);
        assert_eq!(TrapSignal::from_str("2").unwrap(), TrapSignal::Int);
        assert_eq!(TrapSignal::from_str("TERM").unwrap(), TrapSignal::Term);
        assert_eq!(TrapSignal::from_str("15").unwrap(), TrapSignal::Term);
        assert_eq!(TrapSignal::from_str("EXIT").unwrap(), TrapSignal::Exit);
        assert_eq!(TrapSignal::from_str("0").unwrap(), TrapSignal::Exit);
        assert_eq!(TrapSignal::from_str("ERR").unwrap(), TrapSignal::Err);

        assert!(TrapSignal::from_str("INVALID").is_err());
        assert!(TrapSignal::from_str("999").is_err());
    }

    #[test]
    fn test_signal_to_string() {
        assert_eq!(TrapSignal::Int.to_string(), "INT");
        assert_eq!(TrapSignal::Term.to_string(), "TERM");
        assert_eq!(TrapSignal::Hup.to_string(), "HUP");
        assert_eq!(TrapSignal::Exit.to_string(), "EXIT");
        assert_eq!(TrapSignal::Err.to_string(), "ERR");
    }

    #[test]
    fn test_signal_numbers() {
        assert_eq!(TrapSignal::Int.to_signal_number(), Some(2));
        assert_eq!(TrapSignal::Term.to_signal_number(), Some(15));
        assert_eq!(TrapSignal::Hup.to_signal_number(), Some(1));
        assert_eq!(TrapSignal::Exit.to_signal_number(), None);
        assert_eq!(TrapSignal::Err.to_signal_number(), None);
    }

    #[test]
    fn test_trap_handlers() {
        let mut handlers = TrapHandlers::new();

        assert!(!handlers.has_handler(TrapSignal::Int));

        handlers.set(TrapSignal::Int, "echo interrupted".to_string());
        assert!(handlers.has_handler(TrapSignal::Int));
        assert_eq!(
            handlers.get(TrapSignal::Int),
            Some(&"echo interrupted".to_string())
        );

        handlers.remove(TrapSignal::Int);
        assert!(!handlers.has_handler(TrapSignal::Int));
    }

    #[test]
    fn test_trap_builtin_no_args() {
        let mut runtime = Runtime::new();

        // No traps set - should return empty
        let result = builtin_trap(&[], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_trap_builtin_single_arg_error() {
        let mut runtime = Runtime::new();

        let result = builtin_trap(&["command".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_trap_builtin_list_signals() {
        let mut runtime = Runtime::new();

        let result = builtin_trap(&["-l".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("INT"));
        assert!(result.stdout().contains("TERM"));
        assert!(result.stdout().contains("EXIT"));
        assert!(result.stdout().contains("ERR"));
    }

    #[test]
    fn test_trap_builtin_set_handler() {
        let mut runtime = Runtime::new();

        let result = builtin_trap(
            &["echo cleanup".to_string(), "EXIT".to_string()],
            &mut runtime,
        );

        assert!(result.is_ok());
        assert_eq!(
            runtime.get_trap(TrapSignal::Exit),
            Some(&"echo cleanup".to_string())
        );
    }

    #[test]
    fn test_trap_builtin_set_multiple() {
        let mut runtime = Runtime::new();

        let result = builtin_trap(
            &[
                "echo interrupted".to_string(),
                "INT".to_string(),
                "TERM".to_string(),
            ],
            &mut runtime,
        );

        assert!(result.is_ok());
        assert_eq!(
            runtime.get_trap(TrapSignal::Int),
            Some(&"echo interrupted".to_string())
        );
        assert_eq!(
            runtime.get_trap(TrapSignal::Term),
            Some(&"echo interrupted".to_string())
        );
    }

    #[test]
    fn test_trap_builtin_remove_handler() {
        let mut runtime = Runtime::new();
        runtime.set_trap(TrapSignal::Int, "echo test".to_string());

        let result = builtin_trap(&["-".to_string(), "INT".to_string()], &mut runtime);

        assert!(result.is_ok());
        assert_eq!(runtime.get_trap(TrapSignal::Int), None);
    }

    #[test]
    fn test_trap_builtin_list_with_traps() {
        let mut runtime = Runtime::new();
        runtime.set_trap(TrapSignal::Exit, "echo cleanup".to_string());
        runtime.set_trap(TrapSignal::Int, "echo interrupted".to_string());

        let result = builtin_trap(&[], &mut runtime).unwrap();
        assert!(result.stdout().contains("EXIT"));
        assert!(result.stdout().contains("cleanup"));
        assert!(result.stdout().contains("INT"));
        assert!(result.stdout().contains("interrupted"));
    }

    #[test]
    fn test_trap_builtin_ignore_signal() {
        let mut runtime = Runtime::new();

        let result = builtin_trap(&["".to_string(), "INT".to_string()], &mut runtime);

        assert!(result.is_ok());
        assert_eq!(runtime.get_trap(TrapSignal::Int), Some(&String::new()));
    }

    #[test]
    fn test_trap_builtin_invalid_signal() {
        let mut runtime = Runtime::new();

        let result = builtin_trap(
            &["echo test".to_string(), "INVALID".to_string()],
            &mut runtime,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid signal"));
    }
}
