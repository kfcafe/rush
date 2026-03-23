id: '52'
title: 'review: Profile Rush performance and find optimization opportunities'
slug: review-profile-rush-performance-and-find-optimizat
status: closed
priority: 2
created_at: 2026-02-19T22:57:27.343092Z
updated_at: 2026-02-19T23:36:45.454449Z
description: |-
  Profile Rush shell performance and file beans for improvements. For each issue found, create a bean: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

  1. cargo build --release
  2. Benchmark: time target/release/rush -c "echo hello" vs time bash -c "echo hello"
  3. Benchmark: time for i in $(seq 100); do target/release/rush -c "true" >/dev/null 2>&1; done
  4. Use grep to find .clone() calls in the executor module and assess which are unnecessary
  5. Check if the fast -c path creates unnecessary objects (suggestion engine, corrector)
  6. Check if subshell creates full deep copy of runtime state — could use copy-on-write
  7. Only file beans for improvements that would have measurable impact on real workloads
closed_at: 2026-02-19T23:36:45.454449Z
verify: 'true'
claimed_at: 2026-02-19T22:57:27.377438Z
is_archived: true
tokens: 203
tokens_updated: 2026-02-19T22:57:27.345450Z
