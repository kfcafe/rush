id: '196'
title: 'BUILTIN-BUG: grep missing common flags (-q, -r, -l, etc)'
slug: builtin-bug-grep-missing-common-flags-q-r-l-etc
status: closed
priority: 2
created_at: 2026-02-02T04:19:28.794613Z
updated_at: 2026-02-02T09:29:18.921353Z
description: "## Problem\nRush builtin grep is missing common POSIX/GNU flags:\n\n```bash\necho test | grep -q test    # Error: Unknown option: -q\ngrep -rn pattern src/       # Error: Unknown option: -rn  \ngrep -l pattern *.txt       # Error: Unknown option: -l\n```\n\n## Missing Flags (commonly used)\n- `-q` / `--quiet` - Quiet mode, exit status only\n- `-r` / `-R` - Recursive search\n- `-l` - List filenames only\n- `-c` - Count matches\n- `-v` - Invert match\n- `-w` - Word match\n- `-x` - Line match\n- `-A`/`-B`/`-C` - Context lines\n\n## Current Flags\nFrom `help grep`:\n- Pattern matching works\n- `-i` case insensitive\n- `-n` line numbers\n- Basic file/stdin support\n\n## Workaround\nUse system grep: `/usr/bin/grep -q` or `command grep -q`\n\n## Acceptance Criteria\n- [ ] `echo test | grep -q test` exits 0 silently\n- [ ] `echo test | grep -q nomatch` exits 1 silently\n- [ ] `grep -r pattern src/` recursively searches\n- [ ] `grep -l pattern *.txt` lists matching files\n- [ ] `grep -c pattern file` shows count\n\n## Files\n- src/builtins/grep.rs (or similar)"
closed_at: 2026-02-02T09:29:18.921353Z
verify: echo test | grep -q test 2>/dev/null
claimed_at: 2026-02-02T09:29:18.880084Z
is_archived: true
