id: '199'
title: 'PARSER-BUG: Command negation (!) not supported'
slug: parser-bug-command-negation-not-supported
status: closed
priority: 2
created_at: 2026-02-02T04:22:56.838230Z
updated_at: 2026-02-02T09:38:32.080962Z
description: |-
  ## Problem
  The ! operator for negating command exit status does not work:

  \`\`\`bash
  ! true
  # Error: Expected command name

  ! test -f /nonexistent
  # Error: Expected command name

  if ! grep -q pattern file; then echo 'not found'; fi
  # Error
  \`\`\`

  ## Expected Behavior
  \`\`\`bash
  ! true          # exit code 1
  ! false         # exit code 0
  ! test -f /x    # exit code 0 (file doesn't exist, negated)
  \`\`\`

  ## Impact
  - Cannot use ! in if conditions
  - Cannot invert command results
  - Many shell idioms broken

  ## Acceptance Criteria
  - ! true has exit code 1
  - ! false has exit code 0
  - if ! test -f /nonexistent; then echo yes; fi works
  - ! can be used with any command

  ## Files
  - src/parser/mod.rs
  - src/executor/mod.rs
closed_at: 2026-02-02T09:38:32.080962Z
verify: rush -c "! false && echo works" | grep -q works
attempts: 1
claimed_at: 2026-02-02T09:35:22.600548Z
is_archived: true
