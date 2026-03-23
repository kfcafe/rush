---
id: '15'
title: 'bug: Add SAFETY comments to all unsafe blocks'
slug: bug-add-safety-comments-to-all-unsafe-blocks
status: closed
priority: 2
created_at: '2026-02-19T10:10:22.319293Z'
updated_at: '2026-03-02T02:28:24.032554Z'
notes: ''
closed_at: '2026-03-02T02:28:24.032554Z'
is_archived: true
---

2026-02-19T10:22:53.106287+00:00
  Superseded or completed by other beans
verify: cd /Users/asher/rush && ! grep -Pzo 'unsafe\s*\{[^}]*\}' src/**/*.rs 2>/dev/null | grep -v SAFETY | grep -q unsafe; test $? -eq 1 && cargo build 2>&1 | tail -1 | grep -q Finished
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
claimed_by: pi-agent
claimed_at: '2026-03-02T02:27:18.231143Z'
tokens: 151
tokens_updated: '2026-02-19T10:22:53.108123Z'
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-02T02:27:18.231143Z'
---

Several unsafe blocks in the codebase lack // SAFETY comments explaining why they're sound. Add a brief // SAFETY: comment above every unsafe block in src/ that doesn't already have one. Run `grep -rn 'unsafe' src/ --include='*.rs' | grep -v '// SAFETY'` to find them. Common justifications: 'libc FFI call with valid fd', 'signal handler uses async-signal-safe operations', 'fork safety handled by single-threaded context', 'mmap of read-only file with valid fd'. Do not change logic.
