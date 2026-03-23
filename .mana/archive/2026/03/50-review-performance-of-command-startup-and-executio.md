---
id: '50'
title: 'review: Performance of command startup and execution hot paths'
slug: review-performance-of-command-startup-and-executio
status: closed
priority: 2
created_at: '2026-02-19T22:57:10.915753Z'
updated_at: '2026-03-02T02:26:44.018054Z'
closed_at: '2026-03-02T02:26:44.018054Z'
verify: 'true'
is_archived: true
tokens: 45809
tokens_updated: '2026-02-19T22:57:10.917985Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:44.018257Z'
  finished_at: '2026-03-02T02:26:44.073897Z'
  duration_secs: 0.055
  result: pass
  exit_code: 0
---

Audit Rush performance. For each issue: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

Steps:
1. Time: time target/debug/rush -c "echo hello" vs time bash -c "echo hello"
2. Run: grep -c "\.clone()" src/executor/mod.rs — count clones on the hot path
3. Run: grep -n "SuggestionEngine\|Corrector" src/executor/mod.rs | head -5 — these are allocated per Executor even for simple commands
4. Run: grep -n "runtime.clone\|Runtime.*clone" src/executor/mod.rs — subshells clone entire runtime including history
5. Check if HashMap could use a faster hasher: grep -c "HashMap" src/runtime/mod.rs
6. Time 100 invocations: time for i in $(seq 100); do target/debug/rush -c "true" >/dev/null 2>&1; done

Only file perf beans for measurable improvements.
