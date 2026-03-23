id: '7'
title: Replace deprecated atty crate with std::io::IsTerminal
slug: replace-deprecated-atty-crate-with-stdioisterminal
status: closed
priority: 2
created_at: 2026-02-19T08:25:46.264303Z
updated_at: 2026-02-19T08:27:07.923702Z
description: The atty crate is deprecated. Replace all uses with std::io::IsTerminal (stable since Rust 1.70). Run `grep -rn atty src/ Cargo.toml` to find all call sites. Replace `atty::is(atty::Stream::Stdin)` with `std::io::stdin().is_terminal()`, etc. Remove atty from Cargo.toml deps. Add `use std::io::IsTerminal;` where needed.
closed_at: 2026-02-19T08:27:07.923702Z
verify: cd /Users/asher/rush && ! grep -q '^atty' Cargo.toml && cargo build 2>&1 | tail -1 | grep -q Finished
claimed_at: 2026-02-19T08:25:46.299478Z
is_archived: true
tokens: 621
tokens_updated: 2026-02-19T08:25:46.267067Z
