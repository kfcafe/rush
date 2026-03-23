---
id: '18'
title: 'docs: Add module-level doc comments to key modules'
slug: docs-add-module-level-doc-comments-to-key-modules
status: closed
priority: 2
created_at: '2026-02-19T10:21:31.811212Z'
updated_at: '2026-03-02T02:26:43.601582Z'
notes: |-
  ---
  2026-02-19T10:22:53.130823+00:00
  Superseded or completed by other beans
closed_at: '2026-03-02T02:26:43.601582Z'
verify: cd /Users/asher/rush && for f in src/lexer/mod.rs src/parser/mod.rs src/runtime/mod.rs src/signal.rs src/jobs/mod.rs; do head -1 $f | grep -q '//!' || exit 1; done && cargo build 2>&1 | tail -1 | grep -q Finished
is_archived: true
tokens: 62244
tokens_updated: '2026-02-19T10:22:53.132709Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:43.601801Z'
  finished_at: '2026-03-02T02:26:43.761452Z'
  duration_secs: 0.159
  result: pass
  exit_code: 0
---

Add //! module-level doc comments to these files that currently lack them. Read the first few lines of each file and add a brief 2-3 line //! comment at the very top describing the module's purpose. Only add doc comments, do not change any code. Modules: src/lexer/mod.rs, src/parser/mod.rs, src/runtime/mod.rs, src/signal.rs, src/jobs/mod.rs, src/completion/mod.rs, src/history/mod.rs, src/correction/mod.rs.
