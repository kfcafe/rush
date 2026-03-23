id: '24'
title: 'review: Performance audit of hot paths'
slug: review-performance-audit-of-hot-paths
status: closed
priority: 2
created_at: 2026-02-19T18:32:20.326570Z
updated_at: 2026-02-19T18:33:48.765393Z
description: |-
  You are a performance reviewer for the Rush shell. Audit for unnecessary allocations and slow paths.

  For each issue, file: bn create --run --pass-ok "perf: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. Check the hot path: Lexer::tokenize -> Parser::parse -> Executor::execute. How many allocations per command?
  2. grep -n "\.clone()" src/executor/mod.rs | wc -l — count clones in executor. Which are avoidable?
  3. Check expand_variables_in_literal — it builds strings char by char. Could it use Cow<str> to avoid allocation when no expansion needed?
  4. Check resolve_argument — does it allocate for every argument even when no expansion is needed?
  5. Check the Runtime struct — every subshell clones the entire thing. Is there a cheaper copy-on-write approach?
  6. Run: time target/debug/rush -c "echo hello" vs time bash -c "echo hello" — if rush is significantly slower, investigate why
  7. Check if the Corrector/SuggestionEngine are allocated unnecessarily on every Executor creation even for -c mode
  8. Check the fast_execute_c path — is it truly minimal? Are there any unnecessary initializations?

  Only file beans for issues with measurable impact, not micro-optimizations.
closed_at: 2026-02-19T18:33:48.765393Z
close_reason: Recreating with smaller context
verify: 'true'
is_archived: true
tokens: 36904
tokens_updated: 2026-02-19T18:32:20.329205Z
