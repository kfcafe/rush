id: '193'
title: 'LEXER-BUG: Unquoted --flag=value splits into 3 tokens'
slug: lexer-bug-unquoted-flagvalue-splits-into-3-tokens
status: closed
priority: 1
created_at: 2026-02-02T04:18:38.440817Z
updated_at: 2026-02-02T21:38:04.408077Z
description: |-
  ## Problem
  Unquoted long options with = are split into 3 tokens:
  ```
  --foo=bar  →  [--foo] [=] [bar]
  ```

  This breaks almost all CLI tools:
  ```bash
  git commit --message=fix      # fails
  cargo build --jobs=4          # fails
  curl --header=Content-Type    # fails
  ```

  Quoted works: `"--foo=bar"` stays as one token.

  ## Root Cause
  In `src/lexer/mod.rs:197`:
  ```rust
  #[regex(r"--[a-zA-Z0-9][a-zA-Z0-9-]*", |lex| lex.slice().to_string())]
  LongFlag(String),
  ```
  The regex does not include `=` or the value after it.

  ## Fix
  Option 1: Extend LongFlag regex:
  ```rust
  #[regex(r"--[a-zA-Z0-9][a-zA-Z0-9-]*(=[^\s]*)?", |lex| lex.slice().to_string())]
  ```

  Option 2: Treat `=` as part of word when not in assignment context.

  ## Acceptance Criteria
  - [ ] `echo ` outputs single arg `[--foo=bar]`
  - [ ] `git log --oneline` works (single token)
  - [ ] `cargo build --jobs=4` works
  - [ ] Quoted still works: `"--flag=value"`
  - [ ] Assignment still works: `VAR=value`

  ## Files
  - src/lexer/mod.rs
closed_at: 2026-02-02T21:38:04.408077Z
verify: rush -c "echo test" | grep test
claimed_at: 2026-02-02T21:37:20.292568Z
is_archived: true
