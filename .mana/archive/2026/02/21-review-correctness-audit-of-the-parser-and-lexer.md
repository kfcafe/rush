id: '21'
title: 'review: Correctness audit of the parser and lexer'
slug: review-correctness-audit-of-the-parser-and-lexer
status: closed
priority: 2
created_at: 2026-02-19T18:30:56.057598Z
updated_at: 2026-02-19T18:48:21.710705Z
description: |-
  You are a code reviewer for the Rush shell. Audit the lexer and parser for correctness bugs.

  For each real bug, file: bn create --run --pass-ok "bug: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. Read the lexer (src/lexer/mod.rs) — check regex patterns for edge cases: does the Integer regex (-?[0-9]+) incorrectly match negative numbers that should be flags? Do glob patterns conflict with bracket expressions?
  2. Read the parser (src/parser/mod.rs) — check for/while/until/case parsing for edge cases
  3. Test edge cases manually: run target/debug/rush -c "echo -1" and similar
  4. Check heredoc resolution in the lexer — does it handle nested heredocs? Empty heredocs? Heredocs with the delimiter appearing in quoted strings?
  5. Check if the parser handles empty pipeline elements: "echo | | cat"
  6. Look at how the parser handles semicolons vs newlines — are "if true; then echo ok; fi" and the multi-line equivalent both handled?
  7. Test: target/debug/rush -c "for i in 1 2 3; do echo \$i; done" — verify output is correct

  Only file beans for REAL bugs, not style.
closed_at: 2026-02-19T18:48:21.710705Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.704695Z
is_archived: true
tokens: 29480
tokens_updated: 2026-02-19T18:30:56.059166Z
