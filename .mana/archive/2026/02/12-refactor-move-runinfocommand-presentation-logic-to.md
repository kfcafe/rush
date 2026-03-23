id: '12'
title: 'refactor: Move run_info_command presentation logic to stats module'
slug: refactor-move-runinfocommand-presentation-logic-to
status: closed
priority: 2
created_at: 2026-02-19T08:48:45.536695Z
updated_at: 2026-02-19T08:50:58.209163Z
description: The run_info_command function in main.rs is ~200 lines of presentation logic (TUI box drawing, formatting stats into columns). Move the formatting into a new public function `pub fn format_info_output(builtin_stats, custom_stats, daemon_info, json_output) -> String` in the stats module. Keep run_info_command in main.rs but have it call the stats formatting function and print the result.
closed_at: 2026-02-19T08:50:58.209163Z
verify: cd /Users/asher/rush && grep -q 'pub fn format_info_output' src/stats/mod.rs && cargo build 2>&1 | tail -1 | grep -q Finished
claimed_at: 2026-02-19T08:48:45.575031Z
is_archived: true
tokens: 113
tokens_updated: 2026-02-19T08:48:45.539858Z
