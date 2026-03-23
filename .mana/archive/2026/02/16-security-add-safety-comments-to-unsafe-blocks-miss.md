id: '16'
title: 'security: Add SAFETY comments to unsafe blocks missing them'
slug: security-add-safety-comments-to-unsafe-blocks-miss
status: closed
priority: 2
created_at: 2026-02-19T10:15:33.407346Z
updated_at: 2026-02-19T10:20:42.145347Z
description: 'Several unsafe blocks lack // SAFETY comments. Find them with: `grep -rn ''unsafe'' src/ --include=''*.rs'' | grep -v test | grep -v SAFETY`. Add a brief // SAFETY: comment above each explaining why it''s sound. Common reasons: valid fd for libc FFI, async-signal-safe operations, fork in single-threaded context, read-only mmap. Do not change logic.'
closed_at: 2026-02-19T10:20:42.145347Z
verify: cd /Users/asher/rush && cargo build 2>&1 | tail -1 | grep -q Finished && grep -rn 'unsafe' src/ --include='*.rs' | grep -v test | grep -v SAFETY | wc -l | xargs test 5 -ge
claimed_at: 2026-02-19T10:15:33.443515Z
is_archived: true
tokens: 101
tokens_updated: 2026-02-19T10:15:33.408502Z
