id: '25'
title: 'review: Architecture audit — god modules and separation of concerns'
slug: review-architecture-audit-god-modules-and-separati
status: closed
priority: 2
created_at: 2026-02-19T18:32:20.377913Z
updated_at: 2026-02-19T18:33:48.776156Z
description: |-
  You are an architecture reviewer for the Rush shell. Audit for structural problems.

  For each issue, file: bn create --run --pass-ok "refactor: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. Run: wc -l src/executor/mod.rs src/parser/mod.rs src/main.rs — these are the largest files
  2. For the executor (3700+ lines): identify logical groupings that should be separate submodules. E.g., variable expansion, command execution, control flow, redirections could each be their own file.
  3. For the parser (2400+ lines): identify if parsing logic for different constructs (if/for/while/case/functions) should be split into submodules
  4. Check coupling: does the executor depend on too many other modules? Run: grep "^use crate::" src/executor/mod.rs
  5. Check if the Runtime struct has too many responsibilities — it handles variables, functions, aliases, jobs, history, traps, options, positional params, scoping all in one struct
  6. Check the daemon module structure — is the server/client/protocol separation clean?
  7. Look at the value module — there are TWO: src/value/ and src/executor/value/. Why? Should they be merged?

  File refactor beans with cargo build as verify. These are structural improvements, not bug fixes.
closed_at: 2026-02-19T18:33:48.776156Z
close_reason: Recreating with smaller context
verify: 'true'
is_archived: true
tokens: 68835
tokens_updated: 2026-02-19T18:32:20.379287Z
