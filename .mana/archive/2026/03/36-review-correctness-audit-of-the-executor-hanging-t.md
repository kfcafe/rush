---
id: '36'
title: 'review: Correctness audit of the executor — hanging tests and TODOs'
slug: review-correctness-audit-of-the-executor-hanging-t
status: closed
priority: 2
created_at: '2026-02-19T18:48:46.581955Z'
updated_at: '2026-03-02T02:26:43.860623Z'
closed_at: '2026-03-02T02:26:43.860623Z'
verify: 'true'
is_archived: true
tokens: 36832
tokens_updated: '2026-02-19T18:48:46.583709Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:43.860819Z'
  finished_at: '2026-03-02T02:26:43.916467Z'
  duration_secs: 0.055
  result: pass
  exit_code: 0
---

Audit the Rush shell executor for correctness bugs. For each bug found, file: bn create --run --pass-ok "bug: <title>" --verify "<test>" --description "<details>"

Steps:
1. Run: timeout 30 cargo test --lib executor::tests 2>&1 | grep -E "FAILED|running for over" — find hanging/failing tests and file bugs
2. Run: grep -n "TODO\|FIXME" src/executor/mod.rs — check each TODO for real bugs
3. Check evaluate_expression CommandSubstitution arm — does it actually execute the command?
4. Check for/while/until loops clone statements each iteration — is this correct?
5. Test: target/debug/rush -c "x=0; while [ \$x -lt 3 ]; do x=\$((x+1)); echo \$x; done" — does it terminate?
6. Test: target/debug/rush -c "f() { for i in 1 2; do if [ \$i -eq 1 ]; then return; fi; echo \$i; done; }; f; echo done" — return inside loop inside function

Only file beans for REAL bugs with verify commands.
