id: '11'
title: 'refactor: Extract duplicated special variable handling into a shared method'
slug: refactor-extract-duplicated-special-variable-handl
status: closed
priority: 2
created_at: 2026-02-19T08:41:16.099331Z
updated_at: 2026-02-19T08:48:31.708996Z
description: 'In the executor module, special shell variables ($?, $$, $!, $#, $@, $*, $-, $_, $0) are handled with identical match arms in three places: resolve_argument(), evaluate_expression(), and expand_variables_in_literal(). Extract a shared `fn resolve_special_variable(&self, name: &str) -> Option<String>` that all three call. Keep behavior identical.'
closed_at: 2026-02-19T08:48:31.708996Z
verify: cd /Users/asher/rush && grep -q 'fn resolve_special_variable' src/executor/mod.rs && cargo build 2>&1 | tail -1 | grep -q Finished
claimed_at: 2026-02-19T08:48:31.701033Z
is_archived: true
tokens: 105
tokens_updated: 2026-02-19T08:41:16.100246Z
