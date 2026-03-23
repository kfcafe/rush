id: '201'
title: 'ALIAS-BUG: Aliases not expanded in pipelines'
slug: alias-bug-aliases-not-expanded-in-pipelines
status: closed
priority: 2
created_at: 2026-02-02T04:29:07.010749Z
updated_at: 2026-02-02T21:42:40.409615Z
description: "## Problem\nAliases are not expanded when used in a pipeline:\n\n```bash\nalias ll='ls -la'\nll              # works - shows file listing  \nll | head -3    # FAILS - 'Failed to spawn ll: No such file or directory'\n```\n\n## Expected Behavior\nAliases should expand in all contexts where commands are valid, including:\n- First command in pipeline\n- After && or ||\n- In subshells\n- In command substitution\n\n## Current Behavior\nOnly works when alias is the sole command on the line.\n\n## Acceptance Criteria\n- alias ll='ls -la'; ll | head works\n- alias g=grep; echo test | g test works\n- alias e=echo; e foo && e bar works\n- alias x='ls'; $(x) in command substitution\n\n## Files\n- src/executor/mod.rs (alias expansion before pipeline setup)\n- src/parser/mod.rs (possibly expand during parse)"
closed_at: 2026-02-02T21:42:40.409615Z
verify: rush -c 'alias ll="ls -la"; ll | head -1' 2>&1 | grep -v 'Failed to spawn'
attempts: 2
claimed_at: 2026-02-02T21:39:00.026866Z
is_archived: true
