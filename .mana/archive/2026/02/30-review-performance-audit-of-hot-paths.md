id: '30'
title: 'review: Performance audit of hot paths'
slug: review-performance-audit-of-hot-paths
status: closed
priority: 2
created_at: 2026-02-19T18:34:18.286991Z
updated_at: 2026-02-19T18:48:21.894561Z
description: |-
  Performance reviewer for Rush shell. Find unnecessary allocations and slow paths.

  For each issue, file: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

  Steps:
  1. Count clones in the executor module: grep -c "\.clone()" on the main executor file
  2. Look at variable expansion functions — could they use Cow<str> to avoid allocating when no expansion is needed?
  3. Check resolve_argument — does it allocate even for simple literals?
  4. Benchmark: time target/debug/rush -c "echo hello" vs time bash -c "echo hello"
  5. Check the fast_execute_c path for unnecessary initialization
  6. Check if the command corrector and suggestion engine are allocated in non-interactive mode

  Only file beans for measurable impact, not micro-optimizations.
closed_at: 2026-02-19T18:48:21.894561Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.880902Z
is_archived: true
tokens: 204
tokens_updated: 2026-02-19T18:34:18.287893Z
