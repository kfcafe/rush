id: '31'
title: 'review: Architecture audit — god modules and separation of concerns'
slug: review-architecture-audit-god-modules-and-separati
status: closed
priority: 2
created_at: 2026-02-19T18:36:18.293191Z
updated_at: 2026-02-19T18:48:21.927937Z
description: |-
  Architecture reviewer for Rush shell. Find structural problems in module organization.

  For each issue, file: bn create --run --pass-ok "refactor: <title>" --verify "cargo build" --description "<details>"

  Steps:
  1. Find the largest source files: find src -name "*.rs" -exec wc -l {} + | sort -n | tail -10
  2. For files over 1000 lines, identify logical groupings that should be separate submodules
  3. Check module coupling with grep for cross-module imports
  4. Check if the Runtime struct has too many responsibilities (variables, functions, aliases, jobs, history, traps, options, scoping)
  5. Look for duplicated modules (e.g. value types defined in multiple places)

  File refactor beans with cargo build as verify. Structural improvements only.
closed_at: 2026-02-19T18:48:21.927937Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.918696Z
is_archived: true
tokens: 204
tokens_updated: 2026-02-19T18:36:18.294131Z
