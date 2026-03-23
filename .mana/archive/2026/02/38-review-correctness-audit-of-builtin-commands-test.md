id: '38'
title: 'review: Correctness audit of builtin commands (test, read, printf, cd, trap)'
slug: review-correctness-audit-of-builtin-commands-test
status: closed
priority: 2
created_at: 2026-02-19T18:56:41.239747Z
updated_at: 2026-02-19T19:36:46.246088Z
description: |-
  Audit Rush shell builtins for correctness vs POSIX. For each bug: bn create --run --pass-ok "bug: <title>" --verify "<test>" --description "<details>"

  Steps:
  1. ls src/builtins/ to see all builtins
  2. Test the "test" builtin: target/debug/rush -c "[ 1 -eq 1 ] && echo pass" — does it handle -f, -d, -z, -n, -eq, -ne, -gt, -lt, =, != ?
  3. Test "read": echo "hello world" | target/debug/rush -c "read a b; echo a=\$a b=\$b" — does IFS splitting work?
  4. Test "printf": target/debug/rush -c "printf \"%s %d\n\" hello 42"
  5. Test "cd": target/debug/rush -c "cd /tmp && pwd" and target/debug/rush -c "cd - 2>&1"
  6. Test "trap": target/debug/rush -c "trap \"echo caught\" INT; kill -INT \$\$; echo after"
  7. Test "export": target/debug/rush -c "export FOO=bar; env | grep FOO"
  8. Compare behavior with bash for any differences found

  Only file beans for REAL POSIX compliance bugs.
closed_at: 2026-02-19T19:36:46.246088Z
verify: 'true'
claimed_at: 2026-02-19T19:36:46.238197Z
is_archived: true
tokens: 239
tokens_updated: 2026-02-19T18:56:41.240823Z
