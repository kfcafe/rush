//! Positional parameter management for the shell runtime.
//!
//! Handles $1, $2, etc., the shift builtin, and parameter scope
//! push/pop for function calls.

use anyhow::{anyhow, Result};

use super::Runtime;

impl Runtime {
    // Positional parameter management

    /// Set all positional parameters ($1, $2, etc.)
    pub fn set_positional_params(&mut self, params: Vec<String>) {
        self.positional_params = params;
        self.update_positional_variables();
    }

    /// Get a specific positional parameter by index (1-based)
    pub fn get_positional_param(&self, index: usize) -> Option<String> {
        if index == 0 {
            // $0 is handled separately
            None
        } else {
            self.positional_params.get(index - 1).cloned()
        }
    }

    /// Get all positional parameters
    pub fn get_positional_params(&self) -> &[String] {
        &self.positional_params
    }

    /// Shift positional parameters by n positions
    pub fn shift_params(&mut self, n: usize) -> Result<()> {
        if n > self.positional_params.len() {
            return Err(anyhow!(
                "shift: shift count ({}) exceeds number of positional parameters ({})",
                n,
                self.positional_params.len()
            ));
        }

        // Remove first n parameters
        self.positional_params.drain(0..n);

        // Update $1, $2, $#, $@, $* variables
        self.update_positional_variables();

        Ok(())
    }

    /// Push positional parameters onto stack (for function calls)
    pub fn push_positional_scope(&mut self, params: Vec<String>) {
        self.positional_stack.push(self.positional_params.clone());
        self.positional_params = params;
        self.update_positional_variables();
    }

    /// Pop positional parameters from stack (after function returns)
    pub fn pop_positional_scope(&mut self) {
        if let Some(params) = self.positional_stack.pop() {
            self.positional_params = params;
            self.update_positional_variables();
        }
    }

    /// Get the count of positional parameters (for $#)
    pub fn param_count(&self) -> usize {
        self.positional_params.len()
    }

    /// Update $1, $2, $#, $@, $* variables based on current positional params
    fn update_positional_variables(&mut self) {
        // Get old count BEFORE updating $#
        let old_count = self
            .variables
            .get("#")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        // Update $# (parameter count)
        self.variables
            .insert("#".to_string(), self.positional_params.len().to_string());

        // Update $@ and $* (all parameters as space-separated string)
        let all_params = self.positional_params.join(" ");
        self.variables.insert("@".to_string(), all_params.clone());
        self.variables.insert("*".to_string(), all_params);

        // Clear old numbered parameters that are no longer in use
        for i in 1..=old_count.max(self.positional_params.len()) {
            self.variables.remove(&i.to_string());
        }

        // Set new numbered parameters
        for (i, param) in self.positional_params.iter().enumerate() {
            self.variables.insert((i + 1).to_string(), param.clone());
        }
    }
}
