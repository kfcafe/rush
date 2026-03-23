id: '202'
title: Rush Shell Bug Hunt - Session Summary
slug: rush-shell-bug-hunt-session-summary
status: closed
priority: 3
created_at: 2026-02-02T04:29:53.901364Z
updated_at: 2026-02-02T09:46:27.678684Z
description: "## Session Summary\nExploratory testing session to find Rush shell bugs. Created beans for discovered issues.\n\n## Beans Created This Session\n- **193**: Unquoted --flag=value splits into 3 tokens (LEXER)\n- **194**: Tilde (~) always tokenized separately (LEXER)\n- **195**: Line-by-line script execution breaks multi-line constructs (CRITICAL)\n- **195.1**: Loop variables empty/wrong in scripts (child of 195)\n- **196**: grep missing common flags -q, -r, -l (BUILTIN)\n- **197**: Single-quoted strings expand variables / truncate at $ (LEXER)\n- **198**: Stderr redirect (2>/dev/null) ignored for builtins (BUILTIN)\n- **199**: Command negation (!) not supported (PARSER)\n- **200**: Parameter expansion operators not implemented (${#x}, ${x:-default})\n- **201**: Aliases not expanded in pipelines (EXECUTOR)\n\n## Test Artifacts Created\n- /tmp/rush_doctor - diagnostic script\n- /tmp/RUSH_BUG_REPORT.md - detailed bug report\n- /tmp/argtest.sh - argument parsing test helper\n\n## Areas Still To Explore\n- Escape sequences in strings (\\\" not working)\n- Here-strings (<<<) \n- Process substitution <()\n- More builtin edge cases\n- Script sourcing\n- Signal handling edge cases\n- More complex pipelines\n\n## How to Continue\n```bash\nbn status                    # See all open bugs\nbn show 195                  # The critical script execution bug\nrush /tmp/rush_doctor        # Run diagnostic\n```"
notes: |-
  ---
  2026-02-02T04:43:59.211589+00:00
  ## Session 2 Findings (2026-02-02)

  8 new bugs found and documented (beans 203-210):

  ### Critical (P1)
  - 203: Unquoted colon in non-path arguments causes parse error
  - 204: Dot (.) source command not implemented
  - 206: exec command skips all prior commands in sequence
  - 208: Backslash-escaped quote in double strings not handled
  - 209: Pipes into while/compound commands not supported
  - 210: Input redirection (<) ignored for builtins

  ### High (P2)
  - 205: Backticks not expanded inside double-quoted strings
  - 207: set -- does not set positional parameters

  ### Summary
  Total known bugs now: 18 beans (193-210)
  - Lexer issues: 5 (193, 194, 197, 203, 205, 208)
  - Parser issues: 3 (199, 209, script execution 195)
  - Builtin issues: 4 (196, 198, 204, 207, 210)
  - Executor issues: 2 (201, 206)

  ### Areas Tested This Session
  - Control structures (for, while, until, case) - mostly work
  - Redirections - work for external commands, broken for builtins
  - Glob expansion - works
  - Special variables - mostly work
  - trap - works
  - Nested command substitution - works
  - Quote handling - several bugs

  ### Still To Test
  - Arithmetic operators beyond basic +,-,*,/
  - Array operations (likely not supported)
  - More complex here-doc scenarios
  - Signal handling during long operations
closed_at: 2026-02-02T09:46:27.678684Z
verify: 'true'
claimed_at: 2026-02-02T09:46:27.665870Z
is_archived: true
