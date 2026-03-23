id: '22'
title: 'review: Correctness audit of builtin commands'
slug: review-correctness-audit-of-builtin-commands
status: closed
priority: 2
created_at: 2026-02-19T18:31:20.300272Z
updated_at: 2026-02-19T18:48:21.748831Z
description: |-
  You are a code reviewer for the Rush shell. Audit the builtin commands for correctness bugs.

  For each real bug, file: bn create --run --pass-ok "bug: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. List all builtins: ls src/builtins/
  2. For each major builtin (test, read, printf, cd, export, set, trap, kill), read the implementation and check against POSIX behavior
  3. Specifically check: does "test" handle all operators (-eq, -ne, -gt, -lt, -f, -d, -z, -n, =, !=)?
  4. Check "read" builtin: does it handle -r (raw), -p (prompt), IFS splitting, reading into multiple variables?
  5. Check "printf" against POSIX: format specifiers %s %d %x, escape sequences, missing args behavior
  6. Run: target/debug/rush -c "test 1 -eq 1 && echo pass || echo fail" — verify
  7. Check the cd builtin handles: cd -, cd ~, cd with CDPATH, cd to symlinks
  8. Check trap builtin: does it handle all signals? Does trap "" INT ignore SIGINT?

  Only file beans for REAL bugs.
closed_at: 2026-02-19T18:48:21.748831Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.742799Z
is_archived: true
tokens: 255
tokens_updated: 2026-02-19T18:31:20.301876Z
