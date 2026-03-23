---
id: '60'
title: 'P2: Interactive Polish — Right prompt, hooks, autoload, abbr'
slug: p2-interactive-polish-right-prompt-hooks-autoload
status: closed
priority: 2
created_at: '2026-03-03T07:20:48.917403Z'
updated_at: '2026-03-03T08:17:56.733250Z'
labels:
- interactive
- ux
- p2
closed_at: '2026-03-03T08:17:56.733250Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -rq 'right_prompt\|RPROMPT\|RightPrompt' src/ && grep -rq 'precmd\|preexec\|hook' src/
is_archived: true
---

Interactive features that make a shell feel polished: right prompt, precmd/preexec hooks, function autoloading, abbreviations. See docs/builtin-checklist.md §10-§12.
