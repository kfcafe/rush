id: '198'
title: 'BUILTIN-BUG: Stderr redirect (2>/dev/null) ignored for builtins'
slug: builtin-bug-stderr-redirect-2devnull-ignored-for-b
status: closed
priority: 2
created_at: 2026-02-02T04:21:59.454596Z
updated_at: 2026-02-02T09:35:15.366891Z
description: |-
  ## Problem
  Stderr redirections like 2>/dev/null are ignored for builtin commands:

  \`\`\`bash
  ls /nonexistent 2>/dev/null
  # Still shows: ls: /nonexistent: No such file or directory
  \`\`\`

  External commands work correctly:
  \`\`\`bash
  /bin/ls /nonexistent 2>/dev/null
  # Silent (correct)
  \`\`\`

  ## Impact
  - Cannot suppress errors from builtins
  - Scripts that check exit codes while hiding errors break
  - Affects: ls, grep, cat, and other builtins with error output

  ## Acceptance Criteria
  - ls /nonexistent 2>/dev/null produces no output
  - grep pattern /nofile 2>/dev/null produces no output
  - Exit codes still work correctly
  - Works with 2>&1 as well

  ## Files
  - src/builtins/*.rs (stderr handling)
  - src/executor/mod.rs (redirect setup for builtins)
closed_at: 2026-02-02T09:35:15.366891Z
verify: rush -c "ls /nonexistent 2>/dev/null" 2>&1 | wc -l | tr -d " " | grep -q "^0"
attempts: 2
claimed_at: 2026-02-02T09:29:37.754329Z
is_archived: true
