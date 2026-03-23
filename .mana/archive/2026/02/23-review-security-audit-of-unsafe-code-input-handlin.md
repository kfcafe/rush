id: '23'
title: 'review: Security audit of unsafe code, input handling, and process control'
slug: review-security-audit-of-unsafe-code-input-handlin
status: closed
priority: 2
created_at: 2026-02-19T18:31:50.324314Z
updated_at: 2026-02-19T18:48:21.784415Z
description: |-
  You are a security reviewer for the Rush shell. Audit for security issues.

  For each issue, file: bn create --run --pass-ok "security: <title>" --verify "<test>" --description "<details>"

  Review plan:
  1. Run: grep -rn "unsafe" src/ --include="*.rs" | grep -v test — audit each unsafe block for soundness
  2. Check for command injection: does variable expansion in command names allow executing arbitrary commands? E.g. VAR="rm -rf /"; $VAR
  3. Check path traversal in builtins: do cd, cat, rm properly validate paths?
  4. Check the daemon (src/daemon/) — does it validate input from the Unix socket? Can a malicious client crash the daemon?
  5. Check env::set_var usage — this is unsound in multi-threaded Rust. Find all calls outside of tests: grep -rn "env::set_var\|env::remove_var" src/ --include="*.rs" | grep -v test
  6. Check fork() usage in daemon — is there proper cleanup? Can forked children access parent resources?
  7. Check signal handling for TOCTOU races
  8. Check if heredoc temp files (if any) are created securely

  Only file beans for REAL security issues, not theoretical.
closed_at: 2026-02-19T18:48:21.784415Z
verify: 'true'
claimed_at: 2026-02-19T18:48:21.776498Z
is_archived: true
tokens: 292
tokens_updated: 2026-02-19T18:31:50.326639Z
