---
id: '17'
title: 'Memory audit fixes: cap unbounded buffers, reduce clone weight, limit agent history'
slug: memory-audit-fixes-cap-unbounded-buffers-reduce-cl
status: open
priority: 2
created_at: '2026-03-25T01:39:44.855043Z'
updated_at: '2026-03-25T01:39:44.855043Z'
verify: echo "feature"
---

Parent feature for all memory/resource audit fixes found in the March 2025 audit.

7 issues found, 5 actionable:
1. wait_with_output() buffers unlimited command output
2. Runtime cloned on every command substitution (heavy — includes history)
3. accumulated_stdout grows unbounded in execute()
4. Pipeline intermediate buffers held fully in memory
5. Agent conversation history grows unbounded

Issue 4 (pipeline streaming) is architectural — deferred to a future redesign.
Issue 7 (mem::forget in exec) is intentional and correct — no fix needed.
