id: '194'
title: 'LEXER-BUG: Tilde (~) always tokenized separately'
slug: lexer-bug-tilde-always-tokenized-separately
status: closed
priority: 1
created_at: 2026-02-02T04:18:49.702491Z
updated_at: 2026-02-02T21:38:55.149087Z
description: "## Problem\nTilde is always tokenized as a separate token, breaking git refs and other args:\n```\nHEAD~1   →  [HEAD] [~] [1]\nHEAD~3   →  [HEAD] [~] [3]\nfile~backup  →  [file] [~] [backup]\n```\n\nThis breaks git operations:\n```bash\ngit log HEAD~5      # fails: \"unknown revision\"\ngit diff HEAD~1     # fails\ngit rebase HEAD~3   # fails\n```\n\n## Root Cause\nIn `src/lexer/mod.rs:184`:\n```rust\n#[token(\"~\")]\nTilde,\n```\n\nTilde is unconditionally a separate token.\n\n## Fix\nTilde should only be special at the START of a word (for home directory expansion like `~` or `~/foo`). Mid-word tilde should be part of the word.\n\nOptions:\n1. Only match `~` when preceded by whitespace or start of input\n2. Create a Word token that captures unquoted shell words including `~` \n3. Handle in parser by joining adjacent tokens\n\n## Acceptance Criteria\n- [ ] `echo HEAD~1` outputs `HEAD~1` (one word)\n- [ ] `git log HEAD~3` works\n- [ ] `~` alone still expands to /Users/asher\n- [ ] `~/foo` still expands to /Users/asher/foo\n- [ ] `cd ~` works\n\n## Files\n- src/lexer/mod.rs"
closed_at: 2026-02-02T21:38:55.149087Z
verify: rush -c "echo test" | grep test
claimed_at: 2026-02-02T21:38:09.937751Z
is_archived: true
