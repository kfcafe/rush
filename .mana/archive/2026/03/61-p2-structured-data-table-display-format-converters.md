---
id: '61'
title: 'P2: Structured Data — Table display, format converters'
slug: p2-structured-data-table-display-format-converters
status: closed
priority: 2
created_at: '2026-03-03T07:20:55.382926Z'
updated_at: '2026-03-03T08:21:16.646851Z'
labels:
- builtins
- structured-data
- p2
closed_at: '2026-03-03T08:21:16.646851Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -rq 'table_display\|render_table\|TableRenderer' src/ && grep -q '"from"' src/builtins/mod.rs
is_archived: true
---

Extend Rush's JSON story: table display for JSON arrays, `from csv`/`from yaml`/`from toml` converters. See docs/builtin-checklist.md §9.
