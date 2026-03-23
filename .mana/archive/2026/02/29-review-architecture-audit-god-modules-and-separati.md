id: '29'
title: 'review: Architecture audit — god modules and separation of concerns'
slug: review-architecture-audit-god-modules-and-separati
status: closed
priority: 2
created_at: 2026-02-19T18:34:02.029618Z
updated_at: 2026-02-19T18:34:08.210920Z
description: |-
  Architecture reviewer for Rush shell. Find structural problems.

  For each issue, file: bn create --run --pass-ok "refactor: <title>" --verify "cargo build" --description "<details>"

  Steps:
  1. wc -l src/executor/mod.rs src/parser/mod.rs src/main.rs — find largest files
  2. Identify logical groupings in executor that should be submodules
  3. Check coupling: grep "^use crate::" src/executor/mod.rs
  4. Check if Runtime struct has too many responsibilities
  5. Look at value module duplication: src/value/ vs src/executor/value/

  File refactor beans with cargo build as verify.
closed_at: 2026-02-19T18:34:08.210920Z
close_reason: recreating
verify: 'true'
is_archived: true
tokens: 68666
tokens_updated: 2026-02-19T18:34:02.030703Z
