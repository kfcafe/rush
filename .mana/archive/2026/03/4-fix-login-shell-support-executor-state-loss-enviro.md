---
id: '4'
title: 'Fix login shell support: executor state loss + environment setup'
slug: fix-login-shell-support-executor-state-loss-enviro
status: closed
priority: 0
created_at: '2026-03-03T09:33:50.299670Z'
updated_at: '2026-03-03T09:36:29.242187Z'
closed_at: '2026-03-03T09:36:29.242187Z'
verify: 'grep -q ''fn run_interactive_with_reedline.*executor'' src/main.rs && grep -q ''SHELL'' src/main.rs && grep -rq ''/etc/profile\|\.profile'' src/main.rs && timeout 60 cargo test --lib -- --skip executor::tests 2>&1 | grep -q ''test result: ok'''
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
claimed_by: pi-agent
claimed_at: '2026-03-03T09:33:54.484585Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-03T09:36:29.244824Z'
  finished_at: '2026-03-03T09:36:30.739067Z'
  duration_secs: 1.494
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-03T09:33:54.484585Z'
  finished_at: '2026-03-03T09:36:29.242187Z'
---

## Task
Fix the critical bug where login shell profile/rc sourcing is discarded, and add proper login shell environment setup.

## Bug: Executor State Loss
In `src/main.rs`, `run_interactive_with_init()` creates an executor, sources `~/.rush_profile` and `~/.rushrc` into it, then **discards it** and calls `run_interactive_with_reedline()` which creates a brand new executor. All sourced variables, functions, and aliases are lost.

### Current broken flow (src/main.rs ~line 450):
```rust
fn run_interactive_with_init(signal_handler, is_login, skip_rc) {
    let mut executor = Executor::new_with_signal_handler(signal_handler.clone());
    init_runtime_variables(executor.runtime_mut());
    // Sources ~/.rush_profile and ~/.rushrc into this executor
    executor.source_file(&profile); // ← state goes HERE
    executor.source_file(&rushrc);  // ← state goes HERE
    
    // But then calls this, which creates a NEW executor from scratch:
    run_interactive_with_reedline(signal_handler) // ← state LOST
}
```

### Fix
Refactor `run_interactive_with_reedline()` to accept an existing `Executor` instead of creating its own. Pass the executor from `run_interactive_with_init()` through.

Change signature:
```rust
fn run_interactive_with_reedline(executor: Executor) -> Result<()> {
    // Remove: let mut executor = Executor::new_with_signal_handler(signal_handler.clone());
    let mut executor = executor; // Use the one passed in
    // ... rest unchanged
}
```

And in `run_interactive_with_init()`:
```rust
    run_interactive_with_reedline(executor) // pass it through
```

Also fix `run_interactive()` which also calls `run_interactive_with_reedline` — it needs to create an executor and pass it.

## Feature: Standard File Sourcing
After fixing the executor bug, add standard profile sourcing for login shells:

### Login shell sourcing order:
1. `/etc/profile` (if exists)
2. `~/.rush_profile` (Rush-specific — preferred)
3. If no `~/.rush_profile`, fall back to `~/.profile` (POSIX standard)

### Interactive (non-login) sourcing:
1. `~/.rushrc` (Rush-specific)

This matches the POSIX/bash convention.

## Feature: Set $SHELL
When Rush starts, set `$SHELL` to its own path:
```rust
if let Ok(exe) = env::current_exe() {
    env::set_var("SHELL", &exe);
}
```
Do this in `init_environment_variables()`.

## Files
- `src/main.rs` (modify — refactor run_interactive_with_reedline to accept executor, add profile sourcing, set $SHELL)

## Don't
- Don't modify /etc/shells programmatically (requires sudo, should be manual)
- Don't break the non-interactive (script) code path
- Don't change the `-c` fast path
- Don't touch the executor internals — just pass the existing one through
