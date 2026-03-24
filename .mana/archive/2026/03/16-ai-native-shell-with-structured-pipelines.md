---
id: '16'
title: AI-native shell with structured pipelines
slug: ai-native-shell-with-structured-pipelines
status: closed
priority: 2
created_at: '2026-03-24T05:23:03.575247Z'
updated_at: '2026-03-24T16:35:07.664237Z'
labels:
- feature
closed_at: '2026-03-24T16:35:07.664237Z'
close_reason: verify passed (tidy sweep)
verify: echo feature
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T16:35:07.593951Z'
  finished_at: '2026-03-24T16:35:07.648852Z'
  duration_secs: 0.054
  result: pass
  exit_code: 0
outputs:
  text: feature
---

## Vision

Rush becomes the first POSIX-compatible shell with native structured data pipelines and an AI agent built into the prompt.

## Core Pillars
1. Structured Pipelines — builtins produce typed data, pipeline operators filter/transform
2. AI Agent — real agent with tool usage, not just command generation  
3. Lua Extensions — custom builtins, themes, completions, hooks
4. POSIX Compatible — your scripts still work
