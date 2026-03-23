---
id: '7'
title: 'goal: Rush programmatic/agent API'
slug: goal-rush-programmaticagent-api
status: closed
priority: 1
created_at: '2026-03-22T21:56:35.212428Z'
updated_at: '2026-03-22T23:38:11.409945Z'
labels:
- api
- agent
- programmatic
closed_at: '2026-03-22T23:38:11.409945Z'
close_reason: 'Auto-closed: all children completed'
verify: 'cd /Users/asher/rush && cargo test -p rush run_api agent_mode output_budget 2>&1 | grep -q "test result: ok"'
fail_first: true
is_archived: true
---

Make rush usable as a programmatic shell for AI agent workloads.

## Why

Tower's imp agent engine currently spawns `sh -c <cmd>` for every tool call — hundreds per session. Rush already has:
- 80+ built-in commands (ls, grep, cat, find, git) that avoid fork/exec
- Daemon mode (0.4ms startup vs 5ms cold)
- JSON output on built-ins
- A library crate with `Executor::new_embedded()`

But there's no clean one-shot API for programmatic use, no global agent mode, and no output budget control at the shell level. These three things would make rush the ideal shell backend for imp and any other agent system.

## Children
1. One-shot `rush::run()` API — single function call: string in, structured result out
2. Agent output mode — `RUSH_AGENT_MODE=1` env var for structured output by default
3. Shell-level output budget — `--max-output` to cap output size without the caller truncating
