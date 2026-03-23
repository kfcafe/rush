id: '27'
title: 'review: Dependency audit — heavy, deprecated, or unnecessary deps'
slug: review-dependency-audit-heavy-deprecated-or-unnece
status: closed
priority: 2
created_at: 2026-02-19T18:32:50.415279Z
updated_at: 2026-02-19T18:48:21.854287Z
description: |-
  You are reviewing dependencies for the Rush shell. Check Cargo.toml for issues.

  For each issue, file: bn create --run --pass-ok "chore: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. Read Cargo.toml — list all dependencies
  2. Check for heavy deps: reqwest (HTTP client with TLS) is pulled in for a "fetch" builtin. Is this worth the compile time and binary size? Consider if it should be an optional feature.
  3. Check git2 — it links libgit2 (large C library). It is behind a feature flag already — verify the feature flag works: cargo build --no-default-features 2>&1 | tail -3
  4. Run: cargo tree --depth 1 2>&1 | wc -l — count direct+transitive deps
  5. Check for duplicated functionality: both fuzzy-matcher and strsim are used for string similarity. Can one be removed?
  6. Check if num_cpus is still needed — Rust std has std::thread::available_parallelism() since 1.59
  7. Check if terminal_size is still needed vs crossterm which is already a dep
  8. Check walkdir vs ignore — both are file walking crates. Are both needed?

  File beans only for changes that meaningfully reduce compile time or binary size.
closed_at: 2026-02-19T18:48:21.854287Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.845424Z
is_archived: true
tokens: 827
tokens_updated: 2026-02-19T18:32:50.416272Z
