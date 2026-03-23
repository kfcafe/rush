---
id: '33'
title: 'refactor: Split parser/mod.rs (2393 lines) into submodules'
slug: refactor-split-parsermodrs-2393-lines-into-submodu
status: closed
priority: 2
created_at: '2026-02-19T18:38:04.325332Z'
updated_at: '2026-03-02T03:28:58.461312Z'
notes: |2

  ## Attempt 1 — 2026-03-02T03:28:43Z
  Exit code: 1

  ```

  ```
closed_at: '2026-03-02T03:28:58.461312Z'
verify: test -f src/parser/expressions.rs && cargo build
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
attempts: 1
claimed_by: pi-agent
claimed_at: '2026-03-02T03:21:21.579283Z'
is_archived: true
tokens: 22345
tokens_updated: '2026-02-19T18:38:04.326407Z'
history:
- attempt: 1
  started_at: '2026-03-02T03:28:43.827491Z'
  finished_at: '2026-03-02T03:28:43.884816Z'
  duration_secs: 0.057
  result: fail
  exit_code: 1
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-02T03:21:21.579283Z'
  finished_at: '2026-03-02T03:28:58.461312Z'
---

## Context
src/parser/mod.rs is 2393 lines with 79 functions. All parsing logic is in a single file.

## Task
Split into logical submodules within src/parser/:

1. **control_flow.rs** — Control flow parsing (~10 functions):
   - parse_if_statement, parse_shell_if_body
   - parse_for_loop, parse_for_word_list
   - parse_while_loop, parse_until_loop
   - parse_match_expression, parse_case_statement, parse_case_pattern
   - parse_pattern

2. **commands.rs** — Command/pipeline parsing (~10 functions):
   - parse_command_or_pipeline, parse_pipeline_element, parse_brace_group
   - statement_to_pipeline_element, parse_pipe_ask_prompt
   - parse_command, parse_argument, parse_subshell
   - match_redirect_token, parse_single_redirect, parse_redirect_target

3. **functions.rs** — Function/assignment parsing (~7 functions):
   - parse_assignment, parse_expression
   - parse_function_def, parse_bash_function_def, parse_posix_function_def
   - is_posix_function_def, parse_parameters
   - is_bare_assignment, parse_bare_assignment_or_command, parse_assignment_value

4. Keep in mod.rs: Parser struct, new(), parse(), parse_conditional_statement(),
   parse_statement(), parse_block(), peek(), advance(), match_token(), expect_token(),
   is_at_end(), utility functions (strip_outer_quotes, etc.), and tests.

## Files
- src/parser/mod.rs (split into submodules)
- src/parser/control_flow.rs (new)
- src/parser/commands.rs (new)
- src/parser/functions.rs (new)

## Approach
- Move functions to new files
- Use `impl Parser` blocks in each submodule with `use super::*`
- Keep pub API unchanged
- Run cargo build to verify
