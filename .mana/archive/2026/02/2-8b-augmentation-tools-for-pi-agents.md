id: '2'
title: 8B Augmentation Tools for Pi Agents
slug: 8b-augmentation-tools-for-pi-agents
status: closed
priority: 2
created_at: 2026-02-03T08:50:05.721352Z
updated_at: 2026-02-03T08:52:13.990754Z
description: |-
  Build a suite of tools that leverage fast 8B inference (Cerebras @ 3000 t/s) to augment pi agent capabilities.

  ## Goal
  Give Opus/Sonnet agents cheap, fast 8B workers for:
  - File scouting and context compression
  - Parallel exploration and drafting
  - Validation and linting
  - Simple task execution

  ## Architecture
  ```
  ┌─────────────────┐
  │  Pi Extension   │  ← New tools exposed to agents
  │  (TypeScript)   │
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  Cerebras API   │  ← OpenAI-compatible, llama3.1-8b
  │  3000 t/s       │
  └─────────────────┘
  ```

  ## API Key
  Stored at: ~/.config/cerebras/api_key
  Model: llama3.1-8b
  Endpoint: https://api.cerebras.ai/v1/chat/completions

  ## Tools to Build
  1. `8b_scout` - Read files, return compressed relevant snippets
  2. `8b_draft` - Generate N candidate outputs in parallel
  3. `8b_lint` - Validate beans before spawning
  4. `8b_infer_deps` - Auto-detect produces/requires
  5. `8b_compress` - Summarize verbose output
  6. `8b_worker` - Simple task executor for trivial beans

  ## Integration
  - Build as pi extension (see ~/.pi/agent/extensions/)
  - Tools available to all agents via tool calls
  - Configurable model/endpoint for local fallback later
closed_at: 2026-02-03T08:52:13.990754Z
close_reason: |-
  Implemented 8B augmentation tools extension with all 6 tools:
  - 8b_scout: Reads files and returns compressed relevant snippets for a task
  - 8b_draft: Generates N candidate outputs in parallel for brainstorming
  - 8b_lint: Validates bean definitions before spawning agents
  - 8b_infer_deps: Auto-detects produces/requires artifacts from code context
  - 8b_compress: Summarizes verbose output (build logs, test results)
  - 8b_worker: Executes simple coding tasks directly

  All tools use Cerebras API (llama3.1-8b @ 3000 t/s) via OpenAI-compatible endpoint.
  Extension installed at ~/.pi/agent/extensions/8b-tools/
verify: test -f ~/.pi/agent/extensions/8b-tools/extension.js && node -c ~/.pi/agent/extensions/8b-tools/extension.js
claimed_at: 2026-02-03T08:50:20.727473Z
is_archived: true
