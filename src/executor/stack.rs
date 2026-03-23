//! Function call stack tracking for error reporting
//!
//! This module provides utilities for tracking function call stacks during
//! script execution, enabling detailed error messages with full call context.

use crate::error::{CommandContext, SourceLocation};

/// Tracks the function call stack during execution
#[derive(Debug, Clone)]
pub struct CallStack {
    /// Stack of function names in call order
    entries: Vec<CallStackEntry>,
}

/// A single entry in the call stack
#[derive(Debug, Clone)]
pub struct CallStackEntry {
    /// Name of the function
    pub function_name: String,
    /// Optional source location where the call was made
    pub location: Option<SourceLocation>,
}

impl CallStack {
    /// Create a new empty call stack
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a function call onto the stack
    pub fn push(&mut self, function_name: String) {
        self.entries.push(CallStackEntry {
            function_name,
            location: None,
        });
    }

    /// Push a function call with source location
    pub fn push_with_location(&mut self, function_name: String, location: SourceLocation) {
        self.entries.push(CallStackEntry {
            function_name,
            location: Some(location),
        });
    }

    /// Pop a function call from the stack
    pub fn pop(&mut self) -> Option<CallStackEntry> {
        self.entries.pop()
    }

    /// Get the current call stack as a vector of function names
    pub fn as_vec(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| e.function_name.clone())
            .collect()
    }

    /// Check if the stack is empty (not in any function)
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the depth of the current call stack
    pub fn depth(&self) -> usize {
        self.entries.len()
    }

    /// Get the current function name (top of stack)
    pub fn current_function(&self) -> Option<&str> {
        self.entries.last().map(|e| e.function_name.as_str())
    }

    /// Get the full call stack entries (for detailed error reporting)
    pub fn entries(&self) -> &[CallStackEntry] {
        &self.entries
    }

    /// Create a CommandContext with the current call stack
    pub fn create_context(&self, command_name: impl Into<String>) -> CommandContext {
        let context = CommandContext::new(command_name);
        if !self.is_empty() {
            context.with_function_stack(self.as_vec())
        } else {
            context
        }
    }

    /// Create a CommandContext with the current call stack and arguments
    pub fn create_context_with_args(
        &self,
        command_name: impl Into<String>,
        args: Vec<String>,
    ) -> CommandContext {
        let context = CommandContext::new(command_name).with_args(args);
        if !self.is_empty() {
            context.with_function_stack(self.as_vec())
        } else {
            context
        }
    }
}

impl Default for CallStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_stack_push_pop() {
        let mut stack = CallStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.depth(), 0);

        stack.push("func1".to_string());
        assert!(!stack.is_empty());
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.current_function(), Some("func1"));

        stack.push("func2".to_string());
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.current_function(), Some("func2"));

        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().function_name, "func2");
        assert_eq!(stack.depth(), 1);

        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(stack.depth(), 0);
    }

    #[test]
    fn test_call_stack_as_vec() {
        let mut stack = CallStack::new();
        stack.push("outer".to_string());
        stack.push("middle".to_string());
        stack.push("inner".to_string());

        let vec = stack.as_vec();
        assert_eq!(vec, vec!["outer", "middle", "inner"]);
    }

    #[test]
    fn test_call_stack_context() {
        let mut stack = CallStack::new();
        stack.push("func1".to_string());
        stack.push("func2".to_string());

        let ctx = stack.create_context("test_cmd");
        assert_eq!(ctx.command_name, "test_cmd");
        assert_eq!(
            ctx.function_stack,
            Some(vec!["func1".to_string(), "func2".to_string()])
        );
    }

    #[test]
    fn test_call_stack_context_empty() {
        let stack = CallStack::new();
        let ctx = stack.create_context("test_cmd");
        assert_eq!(ctx.command_name, "test_cmd");
        assert_eq!(ctx.function_stack, None);
    }

    #[test]
    fn test_call_stack_with_location() {
        let mut stack = CallStack::new();
        let loc = SourceLocation::new(42, 10).with_filename("test.sh".to_string());
        stack.push_with_location("myfunc".to_string(), loc.clone());

        assert_eq!(stack.current_function(), Some("myfunc"));
        let entry = &stack.entries()[0];
        assert_eq!(entry.function_name, "myfunc");
        assert!(entry.location.is_some());
    }
}
