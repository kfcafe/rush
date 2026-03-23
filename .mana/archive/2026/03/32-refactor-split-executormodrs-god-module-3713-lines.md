---
id: '32'
title: 'refactor: Split executor/mod.rs god module (3713 lines) into submodules'
slug: refactor-split-executormodrs-god-module-3713-lines
status: closed
priority: 2
created_at: '2026-02-19T18:37:54.089105Z'
updated_at: '2026-03-02T03:49:52.958778Z'
closed_at: '2026-03-02T03:49:52.958778Z'
verify: test -f src/executor/expansion.rs && test -f src/executor/control_flow.rs && test -f src/executor/commands.rs && cargo build
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
claimed_by: pi-agent
claimed_at: '2026-03-02T03:35:36.704438Z'
is_archived: true
tokens: 3000
tokens_updated: '2026-02-19T18:37:54.090148Z'
history:
- attempt: 1
  started_at: '2026-03-02T03:49:52.961091Z'
  finished_at: '2026-03-02T03:49:53.123909Z'
  duration_secs: 0.162
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-02T03:21:21.579283Z'
- num: 2
  outcome: success
  agent: pi-agent
  started_at: '2026-03-02T03:35:36.704438Z'
  finished_at: '2026-03-02T03:49:52.958778Z'
---

## Context
src/executor/mod.rs is 3713 lines with 126 functions — a god module that handles everything from
statement execution to variable expansion to pattern matching.

## Task
Split into logical submodules within src/executor/:

1. **expansion.rs** — Variable expansion and resolution (~15 functions):
   - expand_string_value, expand_variables_in_literal, expand_braced_variable
   - expand_heredoc_body, expand_command_substitutions_in_string, expand_and_resolve_arguments
   - resolve_special_variable, resolve_argument, parse_braced_var_expansion
   - find_matching_paren_in_str, expand_tilde, literal_to_string
   - The static variants: resolve_argument_static, expand_and_resolve_arguments_static,
     expand_command_substitutions_in_string_static

2. **control_flow.rs** — Loop/conditional execution (~10 functions):
   - execute_if_statement, execute_for_loop, execute_while_loop, execute_until_loop
   - execute_match, execute_case, case_pattern_matches
   - execute_conditional_and, execute_conditional_or
   - execute_block, evaluate_condition_commands
   - handle_loop_signal, LoopControl enum

3. **commands.rs** — Command execution (~8 functions):
   - execute_command, execute_external_command, execute_user_function
   - execute_subshell, execute_brace_group, execute_background
   - execute_background_via_sh, extract_stdin_content, apply_redirects
   - is_exec_command

4. Keep in mod.rs: Executor struct, execute(), execute_statement(), execute_pipeline(),
   execute_parallel(), execute_assignment(), execute_function_def(), reset(), source_file(),
   execute_trap(), execute_exit_trap()

## Files
- src/executor/mod.rs (split into submodules)
- src/executor/expansion.rs (new)
- src/executor/control_flow.rs (new)
- src/executor/commands.rs (new)

## Approach
- Move functions to new files
- Use `impl Executor` blocks in each submodule with `use super::*`
- Keep pub API unchanged — all public types/functions remain accessible
- Run cargo build to verify no breakage
