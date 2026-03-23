id: '43'
title: 'chore: Make reqwest an optional feature to cut ~185 transitive deps'
slug: chore-make-reqwest-an-optional-feature-to-cut-185
status: closed
priority: 2
created_at: 2026-02-19T20:34:22.694224Z
updated_at: 2026-02-19T21:38:07.381546Z
description: |-
  reqwest pulls in tokio, hyper, h2, futures, TLS — ~185 transitive crates — for a single builtin (src/builtins/fetch.rs).

  Steps:
  1. In Cargo.toml, make reqwest optional: reqwest = { version = "0.11", features = ["json", "blocking"], optional = true }
  2. Add feature: fetch = ["reqwest"] and add 'fetch' to default features list
  3. In src/builtins/fetch.rs, gate the entire module with #[cfg(feature = "fetch")]
  4. In src/builtins/mod.rs (or wherever fetch is registered), gate the fetch builtin registration with #[cfg(feature = "fetch")]
  5. Verify: cargo build --no-default-features should compile without reqwest; cargo build --features fetch should compile with it
closed_at: 2026-02-19T21:38:07.381546Z
verify: cargo build --no-default-features 2>&1 | tail -1 | grep -q Finished && cargo build --features fetch 2>&1 | tail -1 | grep -q Finished
claimed_at: 2026-02-19T20:34:22.789649Z
is_archived: true
tokens: 10439
tokens_updated: 2026-02-19T20:34:22.707434Z
