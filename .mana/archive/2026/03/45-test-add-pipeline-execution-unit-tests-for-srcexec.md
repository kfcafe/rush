---
id: '45'
title: 'test: Add pipeline execution unit tests for src/executor/pipeline.rs'
slug: test-add-pipeline-execution-unit-tests-for-srcexec
status: closed
priority: 2
created_at: '2026-02-19T21:06:18.623640Z'
updated_at: '2026-03-02T03:34:20.759578Z'
notes: "\n## Attempt 1 — 2026-03-02T03:33:48Z\nExit code: 2\n\n```\ngrep: invalid option -- P\nusage: grep [-abcdDEFGHhIiJLlMmnOopqRSsUVvwXxZz] [-A num] [-B num] [-C[num]]\n\t[-e pattern] [-f file] [--binary-files=value] [--color=when]\n\t[--context[=num]] [--directories=action] [--label] [--line-buffered]\n\t[--null] [pattern] [file ...]\ngrep: invalid option -- P\nusage: grep [-abcdDEFGHhIiJLlMmnOopqRSsUVvwXxZz] [-A num] [-B num] [-C[num]]\n\t[-e pattern] [-f file] [--binary-files=value] [--color=when]\n\t[--context[=num]] [--directories=action] [--label] [--line-buffered]\n\t[--null] [pattern] [file ...]\nsh: line 0: test: -ge: unary operator expected\n```\n"
closed_at: '2026-03-02T03:34:20.759578Z'
verify: test "$(cargo test --lib executor::pipeline 2>&1 | grep -o '[0-9]* passed' | grep -o '[0-9]*')" -ge 5
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
attempts: 1
claimed_by: pi-agent
claimed_at: '2026-03-02T03:30:37.379379Z'
is_archived: true
tokens: 6043
tokens_updated: '2026-02-19T21:06:18.634235Z'
history:
- attempt: 1
  started_at: '2026-03-02T03:33:48.670882Z'
  finished_at: '2026-03-02T03:33:48.837465Z'
  duration_secs: 0.166
  result: fail
  exit_code: 2
  output_snippet: "grep: invalid option -- P\nusage: grep [-abcdDEFGHhIiJLlMmnOopqRSsUVvwXxZz] [-A num] [-B num] [-C[num]]\n\t[-e pattern] [-f file] [--binary-files=value] [--color=when]\n\t[--context[=num]] [--directories=action] [--label] [--line-buffered]\n\t[--null] [pattern] [file ...]\ngrep: invalid option -- P\nusage: grep [-abcdDEFGHhIiJLlMmnOopqRSsUVvwXxZz] [-A num] [-B num] [-C[num]]\n\t[-e pattern] [-f file] [--binary-files=value] [--color=when]\n\t[--context[=num]] [--directories=action] [--label] [--line-buffered]\n\t[--null] [pattern] [file ...]\nsh: line 0: test: -ge: unary operator expected"
- attempt: 2
  started_at: '2026-03-02T03:34:20.760035Z'
  finished_at: '2026-03-02T03:34:20.978363Z'
  duration_secs: 0.218
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-02T03:30:37.379379Z'
  finished_at: '2026-03-02T03:34:20.759578Z'
---

src/executor/pipeline.rs has 0 tests despite being a critical module that handles:
- Multi-stage pipelines with streaming
- SIGPIPE / broken pipe handling
- Exit code propagation and pipefail
- PIPESTATUS tracking
- Glob expansion in pipeline arguments
- Alias expansion in pipelines
- Redirect application (stdout, stderr, append, both, heredoc)
- Subshell and compound command pipeline elements

Add unit tests covering at minimum:
1. Single command pipeline execution
2. Multi-command pipeline with data streaming
3. Redirect application (stdout to file, stderr to file, append, both, stderr-to-stdout merge)
4. Argument resolution (literals, variables, globs, command substitution)
5. Pipeline exit code with and without pipefail

File: src/executor/pipeline.rs
