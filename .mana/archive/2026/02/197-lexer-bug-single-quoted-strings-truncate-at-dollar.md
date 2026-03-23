id: '197'
title: 'LEXER-BUG: Single-quoted strings truncate at dollar sign'
slug: lexer-bug-single-quoted-strings-truncate-at-dollar
status: closed
priority: 1
created_at: 2026-02-02T04:21:16.175403Z
updated_at: 2026-02-02T09:29:26.610833Z
description: "## Problem\nSingle-quoted strings are truncated or empty when they contain a dollar sign.\n\nExamples:\n- echo 'hello' works, outputs hello\n- echo '\\' is EMPTY (should output literal \\)  \n- echo 'before\\' outputs just 'before' (truncated)\n\nSingle quotes should preserve ALL characters literally including dollar signs.\n\n## Also Broken\nBackslash-escaping in double quotes does not work properly.\n\n## Root Cause\nLikely in lexer string parsing - the dollar sign triggers variable expansion even inside single quotes.\n\n## Acceptance Criteria\n- Single-quoted dollar signs are preserved literally\n- Backslash-dollar in double quotes works\n\n## Files\n- src/lexer/mod.rs (string parsing)\n- possibly src/executor for expansion"
notes: |-
  ---
  2026-02-02T04:27:20.007337+00:00
  Additional finding: Single quotes expand variables like double quotes. Example: x=hello; echo 'val: \' outputs 'val: hello' instead of literal '\'. This is a more fundamental issue than just truncation at dollar sign.
closed_at: 2026-02-02T09:29:26.610833Z
verify: rush -c "echo hello" | grep hello
claimed_at: 2026-02-02T09:29:26.594088Z
is_archived: true
