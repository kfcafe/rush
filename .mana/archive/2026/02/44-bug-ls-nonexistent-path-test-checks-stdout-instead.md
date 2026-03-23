id: '44'
title: 'bug: ls nonexistent path test checks stdout instead of stderr'
slug: bug-ls-nonexistent-path-test-checks-stdout-instead
status: closed
priority: 2
created_at: 2026-02-19T20:34:26.583572Z
updated_at: 2026-02-19T20:50:38.965642Z
description: |-
  The test test_ls_nonexistent_path asserts result.stdout().contains("cannot access") but builtin_ls puts error messages in stderr, not stdout. Fix the test at src/builtins/ls.rs:681 to check result.stderr instead of result.stdout().

  File: src/builtins/ls.rs (line ~678-683)

  Current (broken):
    assert!(result.stdout().contains("cannot access"));

  Fix to:
    assert!(result.stderr.contains("cannot access"));
closed_at: 2026-02-19T20:50:38.965642Z
verify: 'cargo test --lib builtins::ls::tests::test_ls_nonexistent_path 2>&1 | grep -q ''test result: ok'''
claimed_at: 2026-02-19T20:34:26.703265Z
is_archived: true
tokens: 7146
tokens_updated: 2026-02-19T20:34:26.586591Z
