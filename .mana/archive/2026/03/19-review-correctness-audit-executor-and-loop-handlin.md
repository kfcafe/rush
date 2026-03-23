---
id: '19'
title: 'review: Correctness audit — executor and loop handling'
slug: review-correctness-audit-executor-and-loop-handlin
status: closed
priority: 2
created_at: '2026-02-19T18:29:54.783425Z'
updated_at: '2026-03-02T03:35:16.074614Z'
closed_at: '2026-03-02T03:35:16.074614Z'
verify: 'true'
is_archived: true
tokens: 2000
tokens_updated: '2026-02-19T18:29:54.789725Z'
---

## Review Task
You are a code reviewer. Audit the Rush shell executor for correctness bugs. For each bug you find, file it with:
```
bn create --run --pass-ok "bug: <description>" --verify "<test command>" --description "<details>"
```

## What to Look For
1. **Hanging tests**: Run `timeout 30 cargo test --lib executor::tests 2>&1 | grep -E 'FAILED|running for over'` — some loop tests hang. Find why and file a bean.
2. **TODO/FIXME comments**: Run `grep -n 'TODO\|FIXME' src/executor/mod.rs` — each is a known bug. File beans for the real ones.
3. **Statement cloning in loops**: The for/while/until loops call `statement.clone()` on every iteration. Check if this causes correctness issues (e.g., mutable state in AST nodes).
4. **Subshell isolation**: `execute_subshell` clones the entire Runtime. Verify that variable changes in subshells don't leak to the parent.
5. **Error propagation**: Check that ExitSignal, ReturnSignal, BreakSignal all propagate correctly through nested constructs (e.g., break inside a function inside a loop).
6. **Command substitution**: The TODO at the `Expression::CommandSubstitution` match arm in `evaluate_expression` — does it actually execute the command or just return the string?

## Files to Read
- src/executor/mod.rs (main file — read sections with grep -n to find specific functions)
- src/executor/pipeline.rs
- src/builtins/break_builtin.rs, src/builtins/continue_builtin.rs, src/builtins/return_builtin.rs

## Rules
- Only file beans for REAL bugs that affect behavior, not style issues
- Each bean must have a verify command that would fail before the fix and pass after
- If you can't write a good verify, use `--pass-ok` with a build check
- Be specific in descriptions — include file, line number, and expected vs actual behavior
