---
id: '53'
title: 'POSIX Compliance: Fix all gaps preventing rush from replacing bash as pi''s shell'
slug: posix-compliance-fix-all-gaps-preventing-rush-from
status: closed
priority: 2
created_at: '2026-02-23T08:32:55.774573Z'
updated_at: '2026-03-02T03:20:11.934328Z'
closed_at: '2026-03-02T03:20:11.934328Z'
close_reason: 'Auto-closed: all children completed'
verify: cd ~/rush && cargo test
is_archived: true
tokens: 68371
tokens_updated: '2026-02-23T08:32:55.776130Z'
---

## Parent Bean

Rush is a high-performance POSIX shell written in Rust. It needs the following fixes
to be usable as pi's default execution shell (via shellPath setting):

### Critical Gaps (agent uses constantly)
1. `>&2` / `1>&2` — fd duplication redirects (stdout→stderr) — BROKEN
2. `[ ]` — POSIX test command as `[` — BROKEN (lexer tokenization issue)
3. `echo -e` / `echo -n` — escape sequences and no-newline — flags ignored
4. `<<<` — here-strings — NOT IMPLEMENTED
5. Nested `$()` — command substitution — BROKEN (outputs literal $(echo ...))

### Important Gaps (common in scripts)
6. `[[ ]]` — extended test command — NOT IMPLEMENTED
7. `<()` — process substitution — NOT IMPLEMENTED

### Nice to Have
8. Bash arrays — NOT IMPLEMENTED (skip for now, lowest priority)

## Verify
```bash
cd ~/rush && cargo test
```

## Architecture
- Lexer: src/lexer/mod.rs (Token enum, Logos-based)
- Parser: src/parser/mod.rs + src/parser/ast.rs (AST types)
- Executor: src/executor/mod.rs (command execution, redirections)
- Builtins: src/builtins/ (echo, test, etc.)
