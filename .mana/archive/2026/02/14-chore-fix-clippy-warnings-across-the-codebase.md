id: '14'
title: 'chore: Fix clippy warnings across the codebase'
slug: chore-fix-clippy-warnings-across-the-codebase
status: closed
priority: 2
created_at: 2026-02-19T09:03:33.769069Z
updated_at: 2026-02-19T10:09:57.072586Z
description: 'Rush has ~70 clippy warnings. Fix them. Start with `cargo clippy --fix --lib -p rush --allow-dirty` for auto-fixable ones. Then fix remaining manually: replace push_str single-char with push(char), use std::io::Error::other(), implement FromStr trait instead of inherent from_str, remove empty lines after doc comments, replace map_or with simpler form, etc. Do NOT change behavior. Run `cargo clippy 2>&1 | grep warning:` to see all warnings.'
closed_at: 2026-02-19T10:09:57.072586Z
verify: cd /Users/asher/rush && test $(cargo clippy 2>&1 | grep -c 'warning:') -lt 10
claimed_at: 2026-02-19T10:09:57.056072Z
is_archived: true
tokens: 122
tokens_updated: 2026-02-19T09:03:33.771635Z
