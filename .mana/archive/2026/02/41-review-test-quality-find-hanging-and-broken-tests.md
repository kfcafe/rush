id: '41'
title: 'review: Test quality — find hanging and broken tests'
slug: review-test-quality-find-hanging-and-broken-tests
status: closed
priority: 2
created_at: 2026-02-19T19:43:14.367091Z
updated_at: 2026-02-19T21:38:07.700191Z
description: |-
  Audit Rush tests. For each issue: bn create --run --pass-ok "test: <title>" --verify "<test>" --description "<details>"

  Steps:
  1. Count tests: timeout 60 cargo test --lib -- --list 2>&1 | tail -3
  2. Find hanging tests: timeout 60 cargo test --lib 2>&1 | grep -E "FAILED|running for over"
  3. Find modules with no tests: for d in src/executor/pipeline.rs src/intent/mod.rs src/glob_expansion/mod.rs; do echo "=== $d ==="; grep -c "#\[test\]" $d 2>/dev/null || echo "0"; done
  4. Check test isolation: grep -rn "env::set_var" src/ --include="*.rs" | grep -A2 "#\[test\]" | head -20
  5. Check for ignored tests: grep -rn "#\[ignore\]" src/ tests/ --include="*.rs"

  File beans for hanging tests (bugs), missing coverage (test beans), isolation issues (bugs).
closed_at: 2026-02-19T21:38:07.700191Z
verify: 'true'
claimed_at: 2026-02-19T20:18:04.902162Z
is_archived: true
tokens: 11319
tokens_updated: 2026-02-19T19:43:14.368952Z
