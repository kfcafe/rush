id: '39'
title: 'review: Security audit — env::set_var unsoundness, input validation, daemon'
slug: review-security-audit-envsetvar-unsoundness-input
status: closed
priority: 2
created_at: 2026-02-19T19:42:45.578437Z
updated_at: 2026-02-19T23:36:58.390785Z
description: |-
  Audit Rush for security issues. For each issue: bn create --run --pass-ok "security: <title>" --verify "<test>" --description "<details>"

  Steps:
  1. Find env::set_var outside tests: grep -rn "env::set_var\|env::remove_var" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "mod tests" — these are unsound in multi-threaded Rust
  2. Check daemon socket permissions: read src/daemon/client.rs and src/daemon/server.rs — does it validate input? Can a malicious client crash it?
  3. Check if variable expansion can cause command injection: target/debug/rush -c "x=\"; rm -rf /\"; echo \$x" — is it safe?
  4. Check heredoc handling for temp file security issues
  5. Check signal handling for race conditions
  6. Check the exec builtin for path traversal or injection

  Only file beans for real, exploitable security issues.
closed_at: 2026-02-19T23:36:58.390785Z
verify: 'true'
claimed_at: 2026-02-19T23:36:58.384423Z
is_archived: true
tokens: 14424
tokens_updated: 2026-02-19T19:42:45.584961Z
