---
id: '40'
title: 'review: Performance audit — allocations, clones, startup time'
slug: review-performance-audit-allocations-clones-startu
status: closed
priority: 2
created_at: '2026-02-19T19:43:08.082222Z'
updated_at: '2026-03-02T03:35:16.091970Z'
closed_at: '2026-03-02T03:35:16.091970Z'
verify: 'true'
is_archived: true
tokens: 2000
tokens_updated: '2026-02-19T19:43:08.091555Z'
---

Audit Rush for performance issues. For each issue: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

Steps:
1. Count clones in executor: grep -c "\.clone()" src/executor/mod.rs — which are avoidable?
2. Time startup: time target/debug/rush -c "echo hello" vs time bash -c "echo hello"
3. Check if Corrector/SuggestionEngine are allocated in fast_execute_c path — they shouldnt be
4. Check expand_variables_in_literal — it builds strings char-by-char. Could use Cow when no expansion needed
5. Check Runtime::clone for subshells — the entire history, all aliases, all functions get deep-copied
6. Check if there are HashMap lookups that could use a faster hasher

Only file beans for measurable improvements.
