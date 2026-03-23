id: '26'
title: 'review: Test coverage and test quality audit'
slug: review-test-coverage-and-test-quality-audit
status: closed
priority: 2
created_at: 2026-02-19T18:32:20.410254Z
updated_at: 2026-02-19T18:48:21.818464Z
description: |-
  You are a test quality reviewer for the Rush shell. Audit test coverage and fix test issues.

  For each issue, file: bn create --run --pass-ok "test: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. Run: timeout 60 cargo test --lib -- --list 2>&1 | tail -5 — count total tests
  2. Run: timeout 30 cargo test --lib 2>&1 | grep -E "FAILED|running for" — find broken/hanging tests. File bugs for each.
  3. Check which modules have NO tests: for f in src/executor/pipeline.rs src/daemon/client.rs src/daemon/pi_rpc.rs src/intent/mod.rs src/glob_expansion/mod.rs; do echo "=== $f ==="; grep -c "#\[test\]" $f 2>/dev/null || echo "0 tests"; done
  4. Check test isolation: do tests use env::set_var? That is UB in multi-threaded test runners. grep -rn "env::set_var" src/ --include="*.rs" | grep "#\[test\]" -A5
  5. Check if integration tests in tests/ directory can run: ls tests/*.rs
  6. Look for tests that test implementation details rather than behavior
  7. Check if any tests have #[ignore] and why

  File beans for: hanging tests (bugs), missing test coverage for critical modules (test beans), test isolation issues (bugs).
closed_at: 2026-02-19T18:48:21.818464Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.808531Z
is_archived: true
tokens: 16105
tokens_updated: 2026-02-19T18:32:20.411356Z
