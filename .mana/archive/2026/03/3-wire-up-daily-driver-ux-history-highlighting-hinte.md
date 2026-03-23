---
id: '3'
title: 'Wire up daily driver UX: history, highlighting, hinter, improved prompt'
slug: wire-up-daily-driver-ux-history-highlighting-hinte
status: closed
priority: 0
created_at: '2026-03-03T09:21:14.380199Z'
updated_at: '2026-03-03T09:25:34.712219Z'
notes: |2

  ## Attempt 1 — 2026-03-03T09:25:31Z
  Exit code: 1

  ```

  ```
closed_at: '2026-03-03T09:25:34.712219Z'
verify: 'grep -rq ''FileBackedHistory\|with_history'' src/main.rs && grep -rq ''with_highlighter'' src/main.rs && grep -rq ''with_hinter'' src/main.rs && grep -rq ''git.*branch\|exit.*code\|exit_code'' src/main.rs && timeout 60 cargo test --lib -- --skip executor::tests 2>&1 | grep -q ''test result: ok'''
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
attempts: 1
claimed_by: pi-agent
claimed_at: '2026-03-03T09:21:16.770519Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-03T09:25:31.773325Z'
  finished_at: '2026-03-03T09:25:31.829715Z'
  duration_secs: 0.056
  result: fail
  exit_code: 1
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-03T09:21:16.770519Z'
  finished_at: '2026-03-03T09:25:34.712219Z'
---

## Task
Rush has all the pieces but they're not connected. Wire up the reedline instance in `src/main.rs:575` so the shell actually works as a daily driver.

## Changes Required

### 1. Persistent History (Ctrl+R works, history survives restart)
In `src/main.rs` around line 599 where `Reedline::create()` is called:

Replace:
```rust
let mut line_editor = Reedline::create().with_completer(completer);
```

With:
```rust
use reedline::FileBackedHistory;

let history = Box::new(FileBackedHistory::with_file(
    100,  // max items
    dirs::home_dir()
        .unwrap_or_default()
        .join(".rush_history"),
)?);

let mut line_editor = Reedline::create()
    .with_completer(completer)
    .with_history(history);
```

### 2. Syntax Highlighting (red for bad commands, green for builtins/executables)
The `src/highlight.rs` module exists (282 lines) but is dead code. Wire it:

In `src/main.rs`:
- Add `mod highlight;` near the other mod declarations (line ~18)
- After creating Reedline, add:
```rust
let highlighter = Box::new(highlight::RushHighlighter::new(builtins.clone()));
line_editor = line_editor.with_highlighter(highlighter);
```

### 3. Autosuggestions (fish-style ghost text)
Add hinter (should already be available from reedline):
```rust
use reedline::{DefaultHinter, CursorConfig};
let hinter = Box::new(DefaultHinter::default()
    .with_style(reedline::Style::new().fg(Color::DarkGray)));
line_editor = line_editor.with_hinter(hinter);
```

### 4. Improved Prompt
Replace the `RushPrompt::get_prompt_indicator()` function to include:
- Current git branch (if in a git repo) — use the existing `src/context.rs` module
- Exit code of last command (✓ for 0, ✗ for non-zero)
- Colors (bright cyan for prompt, yellow for git branch)
- Example: `~/project (main) ✓ > ` or `~/project ✗(42) > `

Use the `last_exit_code` that's already being tracked in the loop (line ~628).

### 5. Fix Failing Tests
Two tests fail:
1. `builtins::ls::tests::test_ls_nonexistent_path` — assertion message mismatch. Update the error message check.
2. `output::table::tests::test_truncate_long` — table truncation test. Fix the assertion.

Run with: `cargo test --lib -- --skip executor::tests --test-threads=1`

One test hangs:
- `executor::tests::test_until_with_break` — pre-existing infinite loop. This is known broken, safe to skip.

## Files to Modify
- `src/main.rs` (major — reedline setup, prompt improvements, add mod highlight)
- `src/highlight.rs` (minor — already exists, just needs to be used)
- `src/builtins/ls.rs` (minor — fix test assertion)
- `src/output/table.rs` (minor — fix test assertion)

## Implementation Notes

**History file location**: Use `~/.rush_history` (already done via Reedline's FileBackedHistory)

**Highlighter**: The `RushHighlighter` in src/highlight.rs already implements the reedline::Highlighter trait. Just create an instance and pass it.

**Hinter**: Use reedline's `DefaultHinter` which is built-in. It reads from history automatically.

**Prompt colors**: Use `reedline::Color` and `reedline::Style`. Example:
```rust
use reedline::{Color, Style};
let prompt_style = Style::new().fg(Color::Cyan).bold();
```

**Git branch**: The codebase already has `src/context.rs` with git detection. Use that or grep for current dir's `.git`.

**Progress**: This is a single "integration" bean, not decomposed. It touches many files but each change is surgical (no rewrites, just wiring).

## Verify Strategy
The verify command checks:
1. FileBackedHistory is imported/used
2. with_highlighter is called
3. with_hinter is called  
4. Prompt shows git branch or exit code (basic check — contains "git" or "exit_code")
5. Tests pass (except executor::tests which has a known hang)
