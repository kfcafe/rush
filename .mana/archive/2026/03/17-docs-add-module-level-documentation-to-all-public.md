---
id: '17'
title: 'docs: Add module-level documentation to all public modules'
slug: docs-add-module-level-documentation-to-all-public
status: closed
priority: 2
created_at: '2026-02-19T10:21:22.705373Z'
updated_at: '2026-03-02T02:31:35.270727Z'
notes: ''
closed_at: '2026-03-02T02:31:35.270727Z'
is_archived: true
---

2026-02-19T10:22:53.117112+00:00
  Superseded or completed by other beans
  ## Attempt 1 — 2026-03-02T02:31:23Z
  Exit code: 1

  ```

  ```
verify: cd /Users/asher/rush && cargo doc --no-deps 2>&1 | tail -1 | grep -q Finished && for f in src/lexer/mod.rs src/parser/mod.rs src/executor/mod.rs src/runtime/mod.rs src/signal.rs; do head -1 $f | grep -q '//!' || exit 1; done
attempts: 1
tokens: 98878
tokens_updated: '2026-02-19T10:22:53.119552Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:31:20.764852Z'
  finished_at: '2026-03-02T02:31:23.326249Z'
  duration_secs: 2.561
  result: fail
  exit_code: 1
---

Most public modules in Rush lack //! module-level doc comments. Add a brief //! doc comment at the top of each module describing its purpose. Key modules to document: src/lexer/mod.rs (tokenizer), src/parser/mod.rs (AST parser), src/executor/mod.rs (command execution engine), src/runtime/mod.rs (shell runtime state), src/signal.rs (signal handling), src/jobs/mod.rs (job control), src/completion/mod.rs (tab completion), src/history/mod.rs (command history), src/correction/mod.rs (command correction/suggestions). Keep each doc comment to 2-3 lines. Do not change logic.
