id: '13'
title: 'chore: Fix unused import and dead code warnings across the codebase'
slug: chore-fix-unused-import-and-dead-code-warnings-acr
status: closed
priority: 2
created_at: 2026-02-19T08:51:12.753123Z
updated_at: 2026-02-19T09:02:41.685368Z
description: 'Rush has ~200 compiler warnings, mostly unused imports and dead code. Run `cargo build 2>&1 | grep warning:` to see them all. Fix by: removing unused imports, prefixing unused variables with _, adding #[allow(dead_code)] for intentionally-reserved items. Use `cargo fix --allow-dirty` for auto-fixable ones first. Do NOT change any logic.'
closed_at: 2026-02-19T09:02:41.685368Z
verify: cd /Users/asher/rush && test $(cargo build 2>&1 | grep -c 'warning:') -lt 20
claimed_at: 2026-02-19T08:51:12.778343Z
is_archived: true
tokens: 101
tokens_updated: 2026-02-19T08:51:12.754210Z
