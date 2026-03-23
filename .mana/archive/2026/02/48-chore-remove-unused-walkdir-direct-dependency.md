id: '48'
title: 'chore: Remove unused walkdir direct dependency'
slug: chore-remove-unused-walkdir-direct-dependency
status: closed
priority: 2
created_at: 2026-02-19T21:54:15.267341Z
updated_at: 2026-02-19T22:07:13.178718Z
description: |-
  walkdir is listed as a direct dependency but is never imported in src/. All file-walking uses ignore::WalkBuilder instead. walkdir remains as a transitive dep via ignore, so no functionality is lost.

  Steps:
  1. Remove the line 'walkdir = "2"' from [dependencies] in Cargo.toml
  2. Verify no src/ files import walkdir (they don't — already confirmed)
closed_at: 2026-02-19T22:07:13.178718Z
verify: '! grep -q ''^walkdir'' Cargo.toml && cargo build 2>&1 | tail -1 | grep -q Finished'
claimed_at: 2026-02-19T21:54:15.298746Z
is_archived: true
tokens: 631
tokens_updated: 2026-02-19T21:54:15.269124Z
