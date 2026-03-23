//! Variable management for the shell runtime.
//!
//! Handles shell variables, readonly tracking, IFS splitting, environment
//! variable access, and special variables like $? and PIPESTATUS.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::env;

use super::Runtime;

impl Runtime {
    /// Get the IFS (Internal Field Separator) variable value
    /// Defaults to space, tab, and newline if not set
    pub fn get_ifs(&self) -> String {
        self.get_variable("IFS").unwrap_or_else(|| " \t\n".to_string())
    }
    
    /// Split a string by IFS characters
    /// Returns a vector of fields after splitting
    /// 
    /// If IFS is empty, no splitting occurs.
    /// Leading/trailing IFS whitespace characters are removed.
    /// Consecutive IFS whitespace characters are treated as a single separator.
    pub fn split_by_ifs<'a>(&self, s: &'a str) -> Vec<&'a str> {
        let ifs = self.get_ifs();
        
        if ifs.is_empty() {
            // Empty IFS means no splitting
            return vec![s];
        }
        
        // Split by any character in IFS
        let mut fields = Vec::new();
        let mut current_field_start = 0;
        let mut in_field = false;
        
        for (i, ch) in s.char_indices() {
            if ifs.contains(ch) {
                if in_field {
                    fields.push(&s[current_field_start..i]);
                    in_field = false;
                }
            } else if !in_field {
                current_field_start = i;
                in_field = true;
            }
        }
        
        // Add the last field if we ended in one
        if in_field {
            fields.push(&s[current_field_start..]);
        }
        
        fields
    }

    pub fn set_variable(&mut self, name: String, value: String) {
        // If we're in a function scope, set the variable in the current scope
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        } else {
            // Otherwise set in global scope
            self.variables.insert(name, value);
        }
    }

    /// Set a variable with readonly check
    /// Returns an error if the variable is readonly
    pub fn set_variable_checked(&mut self, name: String, value: String) -> Result<()> {
        if self.is_readonly(&name) {
            return Err(anyhow!("{}: readonly variable", name));
        }
        self.set_variable(name, value);
        Ok(())
    }

    pub fn get_variable(&self, name: &str) -> Option<String> {
        // $RANDOM generates a new random integer 0–32767 on every access (POSIX convention).
        // It is treated as a dynamic special variable — not stored in the variable map.
        if name == "RANDOM" {
            use std::time::{SystemTime, UNIX_EPOCH};
            // Seed with nanoseconds from the current time, mixed with the address of
            // the runtime struct to add per-process jitter.
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(42);
            let addr = (self as *const Self) as usize as u32;
            let seed = nanos ^ addr;
            // LCG parameters from Numerical Recipes
            let v = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            return Some((v % 32768).to_string());
        }

        // Check scopes from most recent to oldest
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value.clone());
            }
        }
        // Fall back to global variables
        self.variables.get(name).cloned()
    }

    /// Remove a variable from the current scope or global scope
    /// Returns true if the variable was found and removed
    pub fn remove_variable(&mut self, name: &str) -> bool {
        // If we're in a function scope, try to remove from the current scope first
        if let Some(scope) = self.scopes.last_mut() {
            if scope.remove(name).is_some() {
                return true;
            }
        }
        // Otherwise remove from global scope
        self.variables.remove(name).is_some()
    }

    /// Get variable with nounset option check
    pub fn get_variable_checked(&self, name: &str) -> Result<String> {
        match self.get_variable(name) {
            Some(value) => Ok(value),
            None => {
                if self.options.nounset {
                    Err(anyhow!("{}: unbound variable", name))
                } else {
                    Ok(String::new())
                }
            }
        }
    }

    /// Set the last exit code (stored in $? variable)
    pub fn set_last_exit_code(&mut self, code: i32) {
        self.variables.insert("?".to_string(), code.to_string());
    }

    /// Get the last exit code (from $? variable)
    pub fn get_last_exit_code(&self) -> i32 {
        self.variables
            .get("?")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// Set the PIPESTATUS array (exit codes of each pipeline command)
    pub fn set_pipestatus(&mut self, codes: Vec<i32>) {
        // Store as space-separated string for POSIX compatibility
        let status_str = codes.iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        self.variables.insert("PIPESTATUS".to_string(), status_str);
    }

    /// Get the PIPESTATUS array as a vector of exit codes
    pub fn get_pipestatus(&self) -> Vec<i32> {
        self.variables
            .get("PIPESTATUS")
            .map(|s| {
                s.split_whitespace()
                    .filter_map(|c| c.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_env(&self) -> HashMap<String, String> {
        env::vars().collect()
    }

    pub fn set_env(&self, key: &str, value: &str) {
        env::set_var(key, value);
    }

    /// Set a local variable in the current function scope
    /// Returns an error if not in a function scope
    pub fn set_local_variable(&mut self, name: String, value: String) -> Result<()> {
        if !self.in_function_context() {
            return Err(anyhow!("Cannot set local variable outside of function"));
        }
        // Set in the current scope (which should be a function scope)
        self.set_variable(name, value);
        Ok(())
    }

    // Readonly variable management

    /// Mark a variable as readonly
    pub fn mark_readonly(&mut self, name: String) {
        self.readonly_vars.insert(name);
    }

    /// Check if a variable is readonly
    pub fn is_readonly(&self, name: &str) -> bool {
        self.readonly_vars.contains(name)
    }

    /// Get all readonly variable names
    pub fn get_readonly_vars(&self) -> Vec<String> {
        let mut vars: Vec<String> = self.readonly_vars.iter().cloned().collect();
        vars.sort();
        vars
    }
}
