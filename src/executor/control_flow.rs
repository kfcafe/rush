//! Loop and conditional execution for the Rush shell executor.
//!
//! This module handles control flow constructs:
//! - If/elif/else statements
//! - For, while, and until loops (with break/continue support)
//! - Match and case statements
//! - Conditional AND (&&) and OR (||) operators
//! - Block execution

use super::*;

/// Result of handling a break/continue signal inside a loop body.
pub(crate) enum LoopControl {
    /// Break out of the current loop and return the accumulated result.
    Break(ExecutionResult),
    /// Continue to the next iteration of the current loop.
    Continue,
    /// Propagate an error outward (decremented signal or non-loop error).
    Propagate(anyhow::Error),
}

/// Inspect a statement error and decide whether to break, continue, or propagate.
///
/// Handles `BreakSignal` and `ContinueSignal` by absorbing their accumulated
/// output, decrementing levels, and returning the appropriate `LoopControl`.
pub(crate) fn handle_loop_signal(
    e: anyhow::Error,
    accumulated_stdout: &mut String,
    accumulated_stderr: &mut String,
    last_exit_code: i32,
) -> LoopControl {
    if let Some(break_signal) = e.downcast_ref::<crate::builtins::break_builtin::BreakSignal>() {
        accumulated_stdout.push_str(&break_signal.accumulated_stdout);
        accumulated_stderr.push_str(&break_signal.accumulated_stderr);

        if break_signal.levels == 1 {
            return LoopControl::Break(ExecutionResult {
                output: Output::Text(accumulated_stdout.clone()),
                stderr: accumulated_stderr.clone(),
                exit_code: last_exit_code,
                error: None,
            });
        } else {
            return LoopControl::Propagate(anyhow::Error::new(
                crate::builtins::break_builtin::BreakSignal {
                    levels: break_signal.levels - 1,
                    accumulated_stdout: accumulated_stdout.clone(),
                    accumulated_stderr: accumulated_stderr.clone(),
                },
            ));
        }
    }

    if let Some(continue_signal) =
        e.downcast_ref::<crate::builtins::continue_builtin::ContinueSignal>()
    {
        accumulated_stdout.push_str(&continue_signal.accumulated_stdout);
        accumulated_stderr.push_str(&continue_signal.accumulated_stderr);

        if continue_signal.levels == 1 {
            return LoopControl::Continue;
        } else {
            return LoopControl::Propagate(anyhow::Error::new(
                crate::builtins::continue_builtin::ContinueSignal {
                    levels: continue_signal.levels - 1,
                    accumulated_stdout: accumulated_stdout.clone(),
                    accumulated_stderr: accumulated_stderr.clone(),
                },
            ));
        }
    }

    LoopControl::Propagate(e)
}

impl Executor {
    pub(crate) fn execute_if_statement(&mut self, if_stmt: IfStatement) -> Result<ExecutionResult> {
        match if_stmt.condition {
            IfCondition::Commands(condition_stmts) => {
                // Shell-style: evaluate condition by running commands
                let condition_result = self.evaluate_condition_commands(&condition_stmts)?;

                if condition_result {
                    return self.execute_block(if_stmt.then_block);
                }

                // Try elif clauses
                for elif in if_stmt.elif_clauses {
                    let elif_result = self.evaluate_condition_commands(&elif.condition)?;
                    if elif_result {
                        return self.execute_block(elif.body);
                    }
                }

                // Else block
                if let Some(else_block) = if_stmt.else_block {
                    return self.execute_block(else_block);
                }

                Ok(ExecutionResult::default())
            }
            IfCondition::Expression(expr) => {
                // Rust-style: evaluate expression for truthiness
                let condition = self.evaluate_expression(expr)?;

                if self.is_truthy(&condition) {
                    self.execute_block(if_stmt.then_block)
                } else if let Some(else_block) = if_stmt.else_block {
                    self.execute_block(else_block)
                } else {
                    Ok(ExecutionResult::default())
                }
            }
        }
    }

    /// Evaluate a list of condition commands. Returns true if last command exits 0.
    pub(crate) fn evaluate_condition_commands(&mut self, commands: &[Statement]) -> Result<bool> {
        let mut last_exit_code = 0;
        for statement in commands {
            let result = self.execute_statement(statement.clone())?;
            last_exit_code = result.exit_code;
        }
        Ok(last_exit_code == 0)
    }

    /// Execute a block of statements and return the combined result.
    pub(crate) fn execute_block(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        for statement in statements {
            let result = self.execute_statement(statement)?;
            accumulated_stdout.push_str(&result.stdout());
            accumulated_stderr.push_str(&result.stderr);
            last_exit_code = result.exit_code;
        }

        Ok(ExecutionResult {
            output: Output::Text(accumulated_stdout),
            stderr: accumulated_stderr,
            exit_code: last_exit_code,
            error: None,
        })
    }

    /// Execute a loop body, handling break/continue signal propagation.
    ///
    /// Returns `LoopControl::Continue` when all statements ran or a continue
    /// signal was caught, `LoopControl::Break` on a break signal, or
    /// `LoopControl::Propagate` for multi-level signals and other errors.
    pub(crate) fn execute_loop_body(
        &mut self,
        body: &[Statement],
        accumulated_stdout: &mut String,
        accumulated_stderr: &mut String,
        last_exit_code: &mut i32,
    ) -> LoopControl {
        for statement in body {
            match self.execute_statement(statement.clone()) {
                Ok(result) => {
                    accumulated_stdout.push_str(&result.stdout());
                    accumulated_stderr.push_str(&result.stderr);
                    *last_exit_code = result.exit_code;
                }
                Err(e) => {
                    return match handle_loop_signal(
                        e,
                        accumulated_stdout,
                        accumulated_stderr,
                        *last_exit_code,
                    ) {
                        LoopControl::Continue => LoopControl::Continue,
                        ctrl => ctrl,
                    };
                }
            }
        }
        LoopControl::Continue
    }

    pub(crate) fn execute_for_loop(&mut self, for_loop: ForLoop) -> Result<ExecutionResult> {
        // Build the list of items to iterate over
        let items: Vec<String> = if for_loop.words.is_empty() {
            // No word list: iterate over positional parameters ($@)
            self.runtime.get_positional_params().to_vec()
        } else {
            // Expand each word individually (handles variables, globs, etc.)
            self.expand_and_resolve_arguments(&for_loop.words)?
        };

        // Enter loop context for break/continue
        self.runtime.enter_loop();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        let result = (|| -> Result<ExecutionResult> {
            for item in items {
                self.runtime
                    .set_variable(for_loop.variable.clone(), item);
                match self.execute_loop_body(
                    &for_loop.body,
                    &mut accumulated_stdout,
                    &mut accumulated_stderr,
                    &mut last_exit_code,
                ) {
                    LoopControl::Continue => {}
                    LoopControl::Break(result) => return Ok(result),
                    LoopControl::Propagate(e) => return Err(e),
                }
            }
            Ok(ExecutionResult {
                output: Output::Text(accumulated_stdout),
                stderr: accumulated_stderr,
                exit_code: last_exit_code,
                error: None,
            })
        })();

        // Exit loop context
        self.runtime.exit_loop();

        result
    }

    pub(crate) fn execute_while_loop(&mut self, while_loop: WhileLoop) -> Result<ExecutionResult> {
        // Enter loop context for break/continue
        self.runtime.enter_loop();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        let result = (|| -> Result<ExecutionResult> {
            loop {
                // Evaluate condition
                let mut condition_exit_code = 0;
                for statement in &while_loop.condition {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            condition_exit_code = result.exit_code;
                        }
                        Err(e) => return Err(e),
                    }
                }

                // While loop continues while condition is true (exit code 0)
                if condition_exit_code != 0 {
                    break;
                }

                // Execute body
                match self.execute_loop_body(
                    &while_loop.body,
                    &mut accumulated_stdout,
                    &mut accumulated_stderr,
                    &mut last_exit_code,
                ) {
                    LoopControl::Continue => {}
                    LoopControl::Break(result) => return Ok(result),
                    LoopControl::Propagate(e) => return Err(e),
                }
            }
            Ok(ExecutionResult {
                output: Output::Text(accumulated_stdout),
                stderr: accumulated_stderr,
                exit_code: last_exit_code,
                error: None,
            })
        })();

        self.runtime.exit_loop();
        result
    }

    pub(crate) fn execute_until_loop(&mut self, until_loop: UntilLoop) -> Result<ExecutionResult> {
        // Enter loop context for break/continue
        self.runtime.enter_loop();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        let result = (|| -> Result<ExecutionResult> {
            loop {
                // Evaluate condition
                let mut condition_exit_code = 0;
                for statement in &until_loop.condition {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            condition_exit_code = result.exit_code;
                        }
                        Err(e) => return Err(e),
                    }
                }

                // Until loop continues until condition is true (exit code 0)
                // So we break when exit code is 0
                if condition_exit_code == 0 {
                    break;
                }

                // Execute body
                match self.execute_loop_body(
                    &until_loop.body,
                    &mut accumulated_stdout,
                    &mut accumulated_stderr,
                    &mut last_exit_code,
                ) {
                    LoopControl::Continue => {}
                    LoopControl::Break(result) => return Ok(result),
                    LoopControl::Propagate(e) => return Err(e),
                }
            }
            Ok(ExecutionResult {
                output: Output::Text(accumulated_stdout),
                stderr: accumulated_stderr,
                exit_code: last_exit_code,
                error: None,
            })
        })();

        self.runtime.exit_loop();
        result
    }

    pub(crate) fn execute_match(&mut self, match_expr: MatchExpression) -> Result<ExecutionResult> {
        let value = self.evaluate_expression(match_expr.value)?;

        for arm in match_expr.arms {
            if self.pattern_matches(&arm.pattern, &value) {
                for statement in arm.body {
                    self.execute_statement(statement)?;
                }
                break;
            }
        }

        Ok(ExecutionResult::default())
    }

    pub(crate) fn execute_case(&mut self, case_stmt: CaseStatement) -> Result<ExecutionResult> {
        // Evaluate the word to match against
        let word_value = self.evaluate_expression(case_stmt.word)?;
        let word = word_value.trim();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;
        let mut matched = false;

        // Try each case arm in order
        for arm in case_stmt.arms {
            // Check if any of the patterns match
            for pattern_str in &arm.patterns {
                if self.case_pattern_matches(pattern_str, word) {
                    matched = true;

                    // Execute the body statements
                    for statement in &arm.body {
                        let result = self.execute_statement(statement.clone())?;
                        accumulated_stdout.push_str(&result.stdout());
                        accumulated_stderr.push_str(&result.stderr);
                        last_exit_code = result.exit_code;
                    }

                    // Break from this arm after execution (POSIX: only first match executes)
                    break;
                }
            }

            // If we found a match, don't check remaining arms
            if matched {
                break;
            }
        }

        // POSIX: exit code is last command in executed list, or 0 if no match
        Ok(ExecutionResult {
            output: Output::Text(accumulated_stdout),
            stderr: accumulated_stderr,
            exit_code: if matched { last_exit_code } else { 0 },
            error: None,
        })
    }

    /// Match a pattern against a word for case statements
    /// Supports glob-style patterns: *, ?, [...]
    pub(crate) fn case_pattern_matches(&self, pattern: &str, word: &str) -> bool {
        // Use glob crate's Pattern for matching
        match glob::Pattern::new(pattern) {
            Ok(glob_pattern) => glob_pattern.matches(word),
            Err(_) => {
                // If pattern is invalid, fall back to literal match
                pattern == word
            }
        }
    }

    pub(crate) fn execute_conditional_and(&mut self, cond_and: ConditionalAnd) -> Result<ExecutionResult> {
        // Execute left side
        let left_result = self.execute_statement(*cond_and.left)?;
        self.runtime.set_last_exit_code(left_result.exit_code);
        
        // Only execute right side if left succeeded (exit code 0)
        if left_result.exit_code == 0 {
            let right_result = self.execute_statement(*cond_and.right)?;
            self.runtime.set_last_exit_code(right_result.exit_code);

            Ok(ExecutionResult {
                output: Output::Text(format!("{}{}", left_result.stdout(), right_result.stdout())),
                stderr: format!("{}{}", left_result.stderr, right_result.stderr),
                exit_code: right_result.exit_code,
                error: right_result.error,
            })
        } else {
            // Left failed, return its result
            Ok(left_result)
        }
    }

    pub(crate) fn execute_conditional_or(&mut self, cond_or: ConditionalOr) -> Result<ExecutionResult> {
        // Execute left side
        let left_result = self.execute_statement(*cond_or.left)?;
        self.runtime.set_last_exit_code(left_result.exit_code);
        
        // Only execute right side if left failed (exit code != 0)
        if left_result.exit_code != 0 {
            let right_result = self.execute_statement(*cond_or.right)?;
            self.runtime.set_last_exit_code(right_result.exit_code);

            Ok(ExecutionResult {
                output: Output::Text(format!("{}{}", left_result.stdout(), right_result.stdout())),
                stderr: format!("{}{}", left_result.stderr, right_result.stderr),
                exit_code: right_result.exit_code,
                error: right_result.error,
            })
        } else {
            // Left succeeded, return its result
            Ok(left_result)
        }
    }
}
