---
id: '6'
title: 'goal: Rush daily-driver readiness'
slug: goal-rush-daily-driver-readiness
status: open
priority: 0
created_at: '2026-03-17T05:54:52.686692Z'
updated_at: '2026-03-24T16:35:28.198658Z'
notes: |2

  ## Attempt 1 — 2026-03-24T16:34:56Z
  Exit code: 1

  ```

  ```

  ## Attempt 2 — 2026-03-24T16:35:28Z
  Exit code: 1

  ```

  ```
labels:
- shell
- daily-driver
verify: cargo test --test quoting_tests 2>&1 | grep -q "0 failed"
attempts: 2
history:
- attempt: 1
  started_at: '2026-03-24T16:34:40.026718Z'
  finished_at: '2026-03-24T16:34:56.603720Z'
  duration_secs: 16.577
  result: fail
  exit_code: 1
- attempt: 2
  started_at: '2026-03-24T16:35:27.986508Z'
  finished_at: '2026-03-24T16:35:28.198644Z'
  duration_secs: 0.212
  result: fail
  exit_code: 1
---

## Problem
Rush cannot be used as a login shell due to critical missing functionality.

## Tiers (in dependency order)
1. Double-quote expansion (blocks everything)
2. Job control wiring
3. Pipeline hardening
4. Non-interactive mode
