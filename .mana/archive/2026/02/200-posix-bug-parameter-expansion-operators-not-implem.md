id: '200'
title: 'POSIX-BUG: Parameter expansion operators not implemented'
slug: posix-bug-parameter-expansion-operators-not-implem
status: closed
priority: 2
created_at: 2026-02-02T04:23:54.076552Z
updated_at: 2026-02-02T21:36:52.326674Z
description: |-
  ## Problem
  POSIX parameter expansion operators are not working - they output literal text:

  \`\`\`bash
  x='hello world'
  echo \${#x}           # outputs: \${#x} (should be: 11)
  echo \${x:-default}   # outputs: \${x:-default} (should be: hello world)
  echo \${x:=value}     # outputs literal (should assign if unset)
  echo \${x:+alt}       # outputs literal
  echo \${x:?error}     # outputs literal
  \`\`\`

  Bash-style substring also broken:
  \`\`\`bash
  echo \${x:0:5}        # outputs literal (should be: hello)
  \`\`\`

  ## Required Operators (POSIX)
  - \${#var} - string length
  - \${var:-default} - use default if unset/null
  - \${var:=default} - assign default if unset/null
  - \${var:+alternate} - use alternate if set
  - \${var:?error} - error if unset/null
  - \${var%pattern} - remove suffix
  - \${var#pattern} - remove prefix

  ## Acceptance Criteria
  - \${#x} returns string length
  - \${x:-default} returns value or default
  - \${x:=val} assigns and returns
  - Pattern removal works

  ## Files
  - src/lexer/mod.rs (BracedVariable parsing)
  - src/executor/mod.rs (expansion logic)
closed_at: 2026-02-02T21:36:52.326674Z
verify: rush -c "x=hello; echo \${#x}" | grep -q 5
attempts: 1
claimed_at: 2026-02-02T09:38:39.996116Z
is_archived: true
