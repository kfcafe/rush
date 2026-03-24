---
id: '9'
title: 'bug: find -path flag not supported'
slug: bug-find-path-flag-not-supported
status: open
priority: 2
created_at: '2026-03-24T02:39:11.964693Z'
updated_at: '2026-03-24T02:39:11.964693Z'
labels:
- bug
- find
verify: ~/bin/rush -c 'find /tmp -name "*.txt" -path "*tmp*" -maxdepth 1' 2>&1 | grep -v "unknown flag"
---

## Bug

`find -path` is not recognized by rush's built-in find command.

### Repro

```sh
~/bin/rush -c 'find ~ -name "agent.rs" -path "*/imp-core/*"'
# Expected: finds matching files
# Actual: rush: find: unknown flag: -path
```

### Impact

`-path` is commonly used by agents and scripts to filter results by directory path. This is a standard POSIX find feature.

Discovered via imp bash tool dogfooding.
