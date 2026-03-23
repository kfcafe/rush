//! Scope and call stack management for the shell runtime.
//!
//! Handles variable scoping for function calls and subshells, the function
//! call stack with recursion depth limits, and loop nesting depth tracking.

use std::collections::HashMap;

use super::Runtime;

impl Runtime {
    // Scope management for function calls
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
        // Note: local_variables field doesn't exist yet - commented out for now
        // if let Some(scope) = self.scopes.pop() {
        //     // Clear local variables that were in this scope
        //     for key in scope.keys() {
        //         self.local_variables.remove(key);
        //     }
        // }
    }

    // Call stack management
    pub fn push_call(&mut self, name: String) -> Result<(), String> {
        if self.call_stack.len() >= self.max_call_depth {
            return Err(format!(
                "Maximum recursion depth exceeded ({})",
                self.max_call_depth
            ));
        }
        self.call_stack.push(name);
        Ok(())
    }

    pub fn pop_call(&mut self) {
        self.call_stack.pop();
    }

    /// Get the current function call stack (for error reporting)
    pub fn get_call_stack(&self) -> Vec<String> {
        self.call_stack.clone()
    }

    // Function context tracking for return builtin
    pub fn enter_function_context(&mut self) {
        self.function_depth += 1;
    }

    pub fn exit_function_context(&mut self) {
        if self.function_depth > 0 {
            self.function_depth -= 1;
        }
    }

    pub fn in_function_context(&self) -> bool {
        self.function_depth > 0
    }

    /// Alias for in_function_context (for backward compatibility with local builtin)
    pub fn in_function(&self) -> bool {
        self.in_function_context()
    }

    // Loop context tracking (for break/continue builtins)
    pub fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    pub fn exit_loop(&mut self) {
        if self.loop_depth > 0 {
            self.loop_depth -= 1;
        }
    }

    pub fn get_loop_depth(&self) -> usize {
        self.loop_depth
    }
}
