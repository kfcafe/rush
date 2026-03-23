---
id: '66'
title: Implement `wc` builtin
slug: implement-wc-builtin
status: closed
priority: 1
created_at: '2026-03-03T07:54:27.537625Z'
updated_at: '2026-03-03T07:55:11.725651Z'
closed_at: '2026-03-03T07:55:11.725651Z'
verify: 'grep -q ''"wc"'' src/builtins/mod.rs && grep -rq ''#\[test\]'' src/builtins/wc.rs && cargo test builtin_wc --lib 2>&1 | grep -q ''test result: ok'''
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-03T07:55:11.726157Z'
  finished_at: '2026-03-03T07:55:11.887747Z'
  duration_secs: 0.161
  result: pass
  exit_code: 0
---

## Task
Implement `wc` (word count) as a builtin — pipeline staple, low complexity.

## Behavior
- `wc [FILE...]` — print lines, words, bytes for each file
- `wc -l` — lines only
- `wc -w` — words only
- `wc -c` — bytes only
- `wc -m` — characters only
- Reads from stdin if no file given
- Print total line when multiple files

## Implementation
1. Create `src/builtins/wc.rs`
2. Single-pass counting: iterate bytes, count newlines/whitespace transitions/bytes
3. Support stdin via `execute_with_stdin` pattern (see how cat and grep do it in mod.rs)
4. Register in `src/builtins/mod.rs`: add `mod wc;` and `m.insert("wc", wc::builtin_wc);`
5. Add `execute_with_stdin` handler in mod.rs for `"wc"` (see the pattern for cat/grep)
6. Add tests

## Context — follow the existing pattern:
```rust
// In mod.rs BUILTIN_MAP:
m.insert("wc", wc::builtin_wc);

// In execute_with_stdin:
if name == "wc" {
    if let Some(stdin_data) = stdin {
        return wc::builtin_wc_with_stdin(&args, runtime, stdin_data);
    }
}
```

## Files
- `src/builtins/wc.rs` (create)
- `src/builtins/mod.rs` (modify — add mod, register, stdin handler)

## Don't
- Don't use ast_grep or sg commands — use standard file editing tools
- Don't modify other builtin files
