---
id: '34'
title: 'refactor: Split runtime/mod.rs (1083 lines) into submodules by responsibility'
slug: refactor-split-runtimemodrs-1083-lines-into-submod
status: closed
priority: 2
created_at: '2026-02-19T18:38:16.820768Z'
updated_at: '2026-03-02T03:29:20.524523Z'
closed_at: '2026-03-02T03:29:20.524523Z'
verify: test -f src/runtime/variables.rs && cargo build
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
claimed_by: pi-agent
claimed_at: '2026-03-02T03:21:21.579283Z'
is_archived: true
tokens: 9337
tokens_updated: '2026-02-19T18:38:16.821849Z'
history:
- attempt: 1
  started_at: '2026-03-02T03:29:20.524970Z'
  finished_at: '2026-03-02T03:29:20.688806Z'
  duration_secs: 0.163
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-02T03:21:21.579283Z'
  finished_at: '2026-03-02T03:29:20.524523Z'
---

## Context
src/runtime/mod.rs is 1083 lines with 99 functions. The Runtime struct manages variables, functions,
aliases, shell options, positional params, history, undo, jobs, traps, redirections, and directory stack
— too many responsibilities in one file.

## Task
Split into submodules within src/runtime/:

1. **variables.rs** — Variable management (~15 functions):
   - set_variable, set_variable_checked, get_variable, remove_variable, get_variable_checked
   - set_local_variable, mark_readonly, is_readonly, get_readonly_vars
   - set_last_exit_code, get_last_exit_code, set_pipestatus, get_pipestatus
   - get_env, set_env, get_ifs, split_by_ifs

2. **scope.rs** — Scoping and call stack (~10 functions):
   - push_scope, pop_scope, push_call, pop_call, get_call_stack
   - enter_function_context, exit_function_context, in_function_context, in_function
   - enter_loop, exit_loop, get_loop_depth

3. **params.rs** — Positional parameters (~8 functions):
   - set_positional_params, get_positional_param, get_positional_params
   - shift_params, push_positional_scope, pop_positional_scope, param_count
   - update_positional_variables

4. **traps.rs** — Trap and redirect management (~10 functions):
   - set_trap, remove_trap, get_trap, get_all_traps, has_trap
   - set_permanent_stdout/stderr/stdin, get_permanent_stdout/stderr/stdin

5. Keep in mod.rs: Runtime struct definition, new(), ShellOptions, functions/aliases/history/
   undo/jobs/options/cwd accessors, dir_stack, piped_stdin, expand_variable, reset(), tests.

## Files
- src/runtime/mod.rs (split into submodules)
- src/runtime/variables.rs (new)
- src/runtime/scope.rs (new)
- src/runtime/params.rs (new)
- src/runtime/traps.rs (new)

## Approach
- Move functions to new files using `impl Runtime` blocks with `use super::*`
- Runtime struct stays in mod.rs, fields stay pub(crate) or pub(super) as needed
- Keep pub API unchanged
- Run cargo build to verify
