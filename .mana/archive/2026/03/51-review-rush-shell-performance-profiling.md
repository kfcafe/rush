---
id: '51'
title: 'review: Rush shell performance profiling'
slug: review-rush-shell-performance-profiling
status: closed
priority: 2
created_at: '2026-02-19T22:57:18.998613Z'
updated_at: '2026-03-02T02:26:44.098607Z'
closed_at: '2026-03-02T02:26:44.098607Z'
verify: 'true'
is_archived: true
tokens: 36943
tokens_updated: '2026-02-19T22:57:19.000628Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:44.098820Z'
  finished_at: '2026-03-02T02:26:44.154543Z'
  duration_secs: 0.055
  result: pass
  exit_code: 0
---

Profile Rush shell performance and file beans for improvements.
For each issue: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

Do these steps in order:
1. Build release: cargo build --release
2. Time rush vs bash: time target/release/rush -c "echo hello" && time bash -c "echo hello"
3. Time 100 invocations: time for i in $(seq 100); do target/release/rush -c "true" >/dev/null 2>&1; done
4. Profile clones: grep -c "\.clone()" src/executor/mod.rs
5. Check Corrector and SuggestionEngine allocation — are these created for -c mode unnecessarily?
6. Check subshell runtime cloning overhead
7. File perf beans only for measurable wins
