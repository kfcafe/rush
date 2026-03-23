id: '42'
title: 'review: Dependency audit — reqwest bloat, duplicate crates, removable deps'
slug: review-dependency-audit-reqwest-bloat-duplicate-cr
status: closed
priority: 2
created_at: 2026-02-19T20:17:55.316885Z
updated_at: 2026-02-19T22:07:47.613239Z
description: |-
  Audit Rush dependencies. For each issue: bn create --run --pass-ok "chore: <title>" --verify "<test>" --description "<details>"

  Steps:
  1. Read Cargo.toml
  2. Run: cargo tree --depth 1 2>&1 | wc -l — count deps
  3. Check reqwest: pulls in tokio, hyper, TLS. Consider making it optional feature.
  4. Check num_cpus: Rust has std::thread::available_parallelism() since 1.59
  5. Check both fuzzy-matcher and strsim — do we need both?
  6. Check walkdir vs ignore — both walk files
  7. Run: cargo build --no-default-features 2>&1 | tail -3 — verify git feature flag works

  Only file beans for meaningful binary size or compile time reductions.
closed_at: 2026-02-19T22:07:47.613239Z
verify: 'true'
claimed_at: 2026-02-19T20:18:04.896726Z
is_archived: true
tokens: 703
tokens_updated: 2026-02-19T20:17:55.321504Z
