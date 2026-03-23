id: '10'
title: 'refactor: DRY up loop body execution in executor'
slug: refactor-dry-up-loop-body-execution-in-executor
status: closed
priority: 2
created_at: 2026-02-19T08:33:14.008308Z
updated_at: 2026-02-19T08:41:00.700338Z
description: The methods execute_for_loop, execute_while_loop, and execute_until_loop each have ~50 lines of identical break/continue signal handling code (checking for BreakSignal and ContinueSignal, decrementing levels, propagating). Extract a shared helper that all three loops call. The helper should take a statement result and return whether to break, continue, or propagate. Keep behavior identical. Only modify the executor module.
closed_at: 2026-02-19T08:41:00.700338Z
verify: cd /Users/asher/rush && grep -q 'fn execute_loop_body\|fn handle_loop_signal\|enum LoopControl' src/executor/mod.rs && cargo build 2>&1 | tail -1 | grep -q Finished
claimed_at: 2026-02-19T08:41:00.696043Z
is_archived: true
tokens: 118
tokens_updated: 2026-02-19T08:33:14.010787Z
