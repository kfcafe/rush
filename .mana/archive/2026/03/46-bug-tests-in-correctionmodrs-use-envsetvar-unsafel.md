---
id: '46'
title: 'bug: Tests in correction/mod.rs use env::set_var unsafely (not thread-safe)'
slug: bug-tests-in-correctionmodrs-use-envsetvar-unsafel
status: closed
priority: 2
created_at: '2026-02-19T21:06:26.574324Z'
updated_at: '2026-03-02T02:30:05.998359Z'
closed_at: '2026-03-02T02:30:05.998359Z'
verify: '! grep -n ''env::set_var'' src/correction/mod.rs | grep -v ''// SAFETY'''
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
claimed_by: pi-agent
claimed_at: '2026-03-02T02:27:18.236742Z'
is_archived: true
tokens: 11751
tokens_updated: '2026-02-19T21:06:26.577662Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:30:05.999907Z'
  finished_at: '2026-03-02T02:30:06.056291Z'
  duration_secs: 0.056
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-02T02:27:18.236742Z'
  finished_at: '2026-03-02T02:30:05.998359Z'
---

Tests in src/correction/mod.rs (lines ~759-854) and src/error.rs (line ~249) use std::env::set_var inside #[test] functions. Since Rust runs tests in parallel by default, this is unsound — env::set_var is not thread-safe and was marked unsafe in Rust 1.66+.

Found 18 occurrences in src/correction/mod.rs and 2 in src/error.rs.

Fix options (pick one):
1. Use temp_env crate or similar test helper that restores env vars
2. Use serial_test crate to serialize these tests  
3. Refactor to pass config values directly instead of reading env vars in the functions under test

Files: src/correction/mod.rs, src/error.rs
