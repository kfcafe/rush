id: '47'
title: 'chore: Replace num_cpus with std::thread::available_parallelism'
slug: chore-replace-numcpus-with-stdthreadavailableparal
status: closed
priority: 2
created_at: 2026-02-19T21:38:14.278306Z
updated_at: 2026-02-19T21:54:08.793559Z
description: |-
  num_cpus is used in only 2 places and can be replaced with std::thread::available_parallelism() (stable since Rust 1.59).

  Steps:
  1. In src/builtins/find.rs:399, replace num_cpus::get().min(4) with std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(4)
  2. In src/stats/mod.rs:270, replace num_cpus::get().to_string() with std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).to_string()
  3. Remove num_cpus from [dependencies] in Cargo.toml
  4. Remove any 'use num_cpus' or 'extern crate num_cpus' if present
closed_at: 2026-02-19T21:54:08.793559Z
verify: cargo build 2>&1 | tail -1 | grep -q Finished && ! grep -q 'num_cpus' Cargo.toml
claimed_at: 2026-02-19T21:38:14.344869Z
is_archived: true
tokens: 18008
tokens_updated: 2026-02-19T21:38:14.282828Z
