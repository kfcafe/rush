id: '28'
title: 'review: Performance audit of hot paths'
slug: review-performance-audit-of-hot-paths
status: closed
priority: 2
created_at: 2026-02-19T18:34:02.000411Z
updated_at: 2026-02-19T18:34:08.201024Z
description: |-
  Performance reviewer for Rush shell. Find unnecessary allocations and slow paths.

  For each issue, file: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

  Steps:
  1. Count clones in executor: grep -n "\.clone()" src/executor/mod.rs | wc -l
  2. Read expand_variables_in_literal — could it use Cow<str>?
  3. Read resolve_argument — does it allocate when no expansion needed?
  4. Time: time target/debug/rush -c "echo hello" vs time bash -c "echo hello"
  5. Read fast_execute_c — any unnecessary init?
  6. Check if Corrector/SuggestionEngine are allocated in -c mode unnecessarily

  Only file beans for measurable impact.
closed_at: 2026-02-19T18:34:08.201024Z
close_reason: recreating
verify: 'true'
is_archived: true
tokens: 36764
tokens_updated: 2026-02-19T18:34:02.001430Z
