id: '195'
title: 'SCRIPT-BUG: Line-by-line execution breaks multi-line constructs'
slug: script-bug-line-by-line-execution-breaks-multi-lin
status: closed
priority: 0
created_at: 2026-02-02T04:19:04.138669Z
updated_at: 2026-02-02T22:06:49.949327Z
description: "## Problem\nScripts are executed LINE BY LINE, making multi-line control structures impossible:\n\n```bash\n# This script FAILS:\nif true; then\n  echo yes\nfi\n# Error: Expected fi to close if statement\n```\n\nSame for loops:\n```bash\nfor i in a b c; do\n  echo \ndone  \n# Error: Expected done to close for loop\n```\n\nSingle-line versions work, but this breaks virtually all real shell scripts.\n\n## Root Cause\nIn `src/main.rs:200`:\n```rust\nfor (line_num, line) in script_content.lines().enumerate() {\n    // ... each line parsed independently\n    match execute_line_with_context(line, ...) {\n```\n\nEach line is parsed and executed independently. The parser sees `if true; then` and expects `fi` on the same line.\n\n## Impact\n- **CRITICAL**: Most shell scripts cannot run\n- Control flow must be single-line only\n- Loop variables dont bind correctly (related bug)\n\n## Fix\nParse the ENTIRE script as one unit, not line by line:\n1. Read full script content\n2. Parse complete AST (handling multi-line constructs)\n3. Execute the AST\n\nThis requires the parser to handle newlines as statement separators within control structures.\n\n## Acceptance Criteria\n- [ ] Multi-line if/then/fi works in scripts\n- [ ] Multi-line for/do/done works in scripts  \n- [ ] Multi-line while/do/done works in scripts\n- [ ] Loop variables bind correctly each iteration\n- [ ] Nested multi-line structures work\n- [ ] Error messages still show correct line numbers\n\n## Files\n- src/main.rs (run_script function, ~line 173-245)\n- src/parser/mod.rs (may need newline handling)\n\n## Test Script\n```bash\n#!/usr/bin/env rush\nfor x in A B C; do\n  echo \"x=\"\ndone\n# Expected: x=A, x=B, x=C\n# Current: x=, x=, x= or parser error\n```"
closed_at: 2026-02-02T22:06:49.949327Z
verify: printf "for i in 1 2 3; do\necho \"test-\$i\"\ndone\n" > /tmp/multiline_test.sh && rush /tmp/multiline_test.sh 2>&1 | grep -q "test-1"
attempts: 1
claimed_at: 2026-02-02T22:06:49.919161Z
is_archived: true
