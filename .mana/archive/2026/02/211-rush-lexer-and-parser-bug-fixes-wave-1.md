id: '211'
title: Rush Lexer and Parser Bug Fixes - Wave 1
slug: rush-lexer-and-parser-bug-fixes-wave-1
status: closed
priority: 2
created_at: 2026-02-02T05:14:10.955165Z
updated_at: 2026-02-02T09:45:54.732946Z
description: "Parent bean for parallel bug fixing session. Contains 8 bugs found during exploratory testing.\n\n## Children (to be fixed in parallel)\n- 203: Colon parsing in non-path arguments\n- 204: Dot (.) source command  \n- 205: Backticks in double-quoted strings\n- 206: exec skips prior commands\n- 207: set -- positional parameters\n- 208: Backslash-escaped quotes\n- 209: Pipes into compound commands\n- 210: Input redirection for builtins\n\n## Strategy\nThese bugs touch different areas of the codebase:\n- Lexer: 203, 205, 208\n- Parser: 209\n- Builtins: 204, 207, 210\n- Executor: 206\n\nCan be parallelized safely."
closed_at: 2026-02-02T09:45:54.732946Z
verify: test $(ls .beans/archive/2026/02/211.* 2>/dev/null | wc -l) -ge 8
attempts: 1
is_archived: true
