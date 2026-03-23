---
id: '49'
title: 'review: Performance — clones, startup time, allocation hot spots'
slug: review-performance-clones-startup-time-allocation
status: closed
priority: 2
created_at: '2026-02-19T22:41:49.603346Z'
updated_at: '2026-03-02T02:26:43.938864Z'
closed_at: '2026-03-02T02:26:43.938864Z'
verify: 'true'
is_archived: true
tokens: 46969
tokens_updated: '2026-02-19T22:41:49.607002Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:43.939094Z'
  finished_at: '2026-03-02T02:26:43.994849Z'
  duration_secs: 0.055
  result: pass
  exit_code: 0
---

Audit Rush performance. For each issue: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

Steps:
1. Time: time target/debug/rush -c "echo hello" vs time bash -c "echo hello" — measure startup overhead
2. Check fast_execute_c: grep -n "Corrector\|SuggestionEngine\|Runtime::new" src/main.rs — are unnecessary objects allocated for -c?
3. Count clones: grep -c "\.clone()" src/executor/mod.rs
4. Check subshell perf: grep -n "execute_subshell\|runtime.clone\|Runtime.*clone" src/executor/mod.rs — full runtime deep copy is expensive
5. Check if HashMap operations in runtime could benefit from FxHashMap
6. Test: time for i in $(seq 1 100); do target/debug/rush -c "echo $i" > /dev/null; done — 100 invocations

Only file beans for measurable wins.
