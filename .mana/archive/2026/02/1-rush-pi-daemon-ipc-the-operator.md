id: '1'
title: 'Rush ↔ Pi Daemon IPC: The `|?` Operator'
slug: rush-pi-daemon-ipc-the-operator
status: closed
priority: 1
created_at: 2026-02-03T08:37:32.547499Z
updated_at: 2026-02-03T09:27:31.388005Z
description: "## Vision\n\nRush and Pi run as cooperating daemons, communicating over a Unix socket. This enables:\n\n1. **`|?` operator** - Pipe output to LLM with sub-millisecond dispatch\n   ```bash\n   git diff |? \"write a commit message\"\n   cargo build 2>&1 |? \"fix the error\"\n   ```\n\n2. **Conversational shell** - Pi maintains context across commands\n   ```bash\n   |? \"find all TODOs\"\n   |? \"now group by priority\"  # remembers previous\n   ```\n\n3. **Intent prefix** - Natural language to command\n   ```bash\n   ? deploy to staging\n   # Pi shows plan, you confirm\n   ```\n\n4. **Bidirectional** - Pi agents can invoke Rush for fast command execution\n\n5. **Shell context** - Pi knows cwd, recent commands, last error, env vars\n\n## Architecture\n\n```\n┌─────────────────┐     Unix Socket      ┌─────────────────┐\n│   Rush Daemon   │◄───────────────────►│    Pi Daemon    │\n│                 │  ~/.rush/pi.sock     │                 │\n│  • Shell exec   │                      │  • LLM provider │\n│  • Job control  │◄────── context ──────│  • Tool use     │\n│  • History      │                      │  • Memory       │\n│  • |? operator  │────── queries ──────►│  • Streaming    │\n└─────────────────┘                      └─────────────────┘\n```\n\n## Message Protocol (JSON over Unix socket)\n\n```typescript\n// Rush → Pi: LLM query\n{\n  \"type\": \"query\",\n  \"id\": \"uuid\",\n  \"prompt\": \"write a commit message\",\n  \"stdin\": \"diff --git a/...\",\n  \"context\": {\n    \"cwd\": \"/Users/asher/tt/rush\",\n    \"last_command\": \"git diff\",\n    \"last_exit_code\": 0,\n    \"shell_history\": [\"cargo build\", \"cargo test\", \"git diff\"]\n  }\n}\n\n// Pi → Rush: Streaming response\n{ \"type\": \"chunk\", \"id\": \"uuid\", \"content\": \"feat: \" }\n{ \"type\": \"chunk\", \"id\": \"uuid\", \"content\": \"add IPC\" }\n{ \"type\": \"done\", \"id\": \"uuid\" }\n\n// Pi → Rush: Tool call (bidirectional!)\n{\n  \"type\": \"tool_call\",\n  \"id\": \"uuid\", \n  \"tool\": \"bash\",\n  \"args\": { \"command\": \"ls -la\" }\n}\n\n// Rush → Pi: Tool result\n{\n  \"type\": \"tool_result\",\n  \"id\": \"uuid\",\n  \"output\": \"total 64\\ndrwxr-xr-x...\"\n}\n```\n\n## Files\n\n**Rush side:**\n- `src/lexer/mod.rs` - Add `PipeAsk` token for `|?`\n- `src/parser/mod.rs` - Parse `|?` into AST\n- `src/executor/mod.rs` - Execute `|?` via IPC\n- `src/daemon/pi_client.rs` - NEW: Unix socket client to Pi\n- `src/daemon/protocol.rs` - NEW: Message types\n\n**Pi side:**\n- Daemon mode with Unix socket listener\n- Shell context handler\n- Rush tool integration"
acceptance: |-
  1. `echo "hello" |? "translate to french"` returns LLM response
  2. Streaming output displays in real-time as chunks arrive
  3. Pi maintains conversation context across multiple `|?` calls in same session
  4. Works with rush daemon mode (sub-ms dispatch) and standalone rush (still works, just slower)
  5. Pi can call back to Rush to execute commands (bidirectional)
  6. Shell context (cwd, history, last error) is passed to Pi
closed_at: 2026-02-03T09:27:31.388005Z
close_reason: 'Auto-closed: all children completed'
is_archived: true
