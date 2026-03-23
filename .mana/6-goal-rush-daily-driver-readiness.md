---
id: '6'
title: 'goal: Rush daily-driver readiness'
slug: goal-rush-daily-driver-readiness
status: open
priority: 0
created_at: 2026-03-17T05:54:52.686692Z
updated_at: 2026-03-17T05:54:52.686692Z
labels:
- shell
- daily-driver
verify: cargo test --test quoting_tests 2>&1 | grep -q "0 failed"
tokens: 66
tokens_updated: 2026-03-17T05:54:52.689387Z
---

## Problem
Rush cannot be used as a login shell due to critical missing functionality.

## Tiers (in dependency order)
1. Double-quote expansion (blocks everything)
2. Job control wiring
3. Pipeline hardening
4. Non-interactive mode
