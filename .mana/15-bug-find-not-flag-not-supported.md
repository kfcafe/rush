---
id: '15'
title: 'bug: find -not flag not supported'
slug: bug-find-not-flag-not-supported
status: open
priority: 2
created_at: '2026-03-24T03:28:10.413946Z'
updated_at: '2026-03-24T03:28:10.413946Z'
labels:
- bug
- find
verify: echo x > /tmp/rush-find-a.txt && echo x > /tmp/rush-find-b.log && ~/bin/rush -c 'find /tmp -maxdepth 1 -name "rush-find*" -not -name "*.log"' 2>&1 | grep rush-find-a
fail_first: true
---

## Bug

`find -not` not recognized by rush builtin find.

```sh
~/bin/rush -c 'find /tmp -name "*.txt" -not -name "rush*"'
# Actual: find: unknown flag: -not
```

Also need `!` as synonym. Discovered via imp dogfooding.
