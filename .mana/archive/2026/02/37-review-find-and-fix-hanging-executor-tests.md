id: '37'
title: 'review: Find and fix hanging executor tests'
slug: review-find-and-fix-hanging-executor-tests
status: closed
priority: 2
created_at: 2026-02-19T18:48:54.658177Z
updated_at: 2026-02-19T18:56:22.368704Z
description: |-
  Run: timeout 30 cargo test --lib executor::tests 2>&1 | grep -E "FAILED|running for over"
  For each hanging or failing test, investigate why and file: bn create --run --pass-ok "bug: <title>" --verify "<verify>" --description "<details>"
  Key areas: while_true_break, until_with_break, while_loop_continue tests may hang due to infinite loops. Check if the condition evaluation or break signal handling is broken.
closed_at: 2026-02-19T18:56:22.368704Z
verify: 'true'
claimed_at: 2026-02-19T18:48:54.689084Z
is_archived: true
tokens: 113
tokens_updated: 2026-02-19T18:48:54.659191Z
