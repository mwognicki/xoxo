# xoxo

`xoxo` is a personal AI assistant that lives in your terminal.

It is primarily a **coding assistant** — pair-programming, reading your project, running tools, and helping you get work done at the keyboard. With an opt-in extension, it can also act as a broader **personal assistant** for tasks outside of code.

It draws inspiration from Mistral Le Vibe, OpenAI Codex, and OpenCode, with a handful of personal tweaks on top.

## Status

> **Early days.** The repository is at version `0.0.1` and the design is still consolidating. The core runtime (bus, agents/subagents, LLM facade, persistence, coding tools) is in place and partially wired, but the end-to-end experience is a work in progress — rough edges and missing pieces are expected.

## Features

### Two assistants, one binary

- **`nerd`** — always-on coding assistant. Reads code, edits files, runs commands, answers repo-scoped questions.
- **`concierge`** — opt-in broader personal assistant. Compiled in only when you ask for it.
- **Headless daemon (`xoxo daemon`)** or **embedded TUI (`xoxo tui`)** — same engine, same in-process bus, same persisted state.
- **Dev utilities (`xoxo dev …`)** — dump a raw persisted chat snapshot or purge the local store.

### Agents and subagents as a tree

- **One chat = one agent.** Agent identity is the chat's `Uuid`; no parallel ID space to maintain.
- **Root agents orchestrate; specialised subagents do bounded sub-tasks.** A root can call `spawn_subagent` with an explicit toolset, an optional model override, and a short brief.
- **Structured handoff, not streaming.** A subagent runs its own loop to completion and returns a `SubagentHandoff` (completed / failed / cancelled + the full transcript + a summary). The parent LLM sees exactly one tool-result turn.
- **Depth capped by construction.** Children only receive `spawn_subagent` while the depth budget allows it — leaves literally cannot ask.
- **Full observability.** Subagent activity streams on the same bus as the root's, stamped with a `ChatPath` so any client can render the tree.
- **Every chat persisted in full**, subagents included — sessions resume from disk; a tree can be reconstructed from any node.

### Coding tools that aren't just text search

- **AST-backed code intelligence** via Tree-sitter (40+ grammars: Rust, TypeScript/JavaScript, Python, Go, Java, C/C++, C#, Ruby, PHP, Swift, Kotlin, Scala, Haskell, Lua, Dart, Elixir, Erlang, Zig, Julia, Solidity, GraphQL, Proto, Nix, and more).
- **Deterministic code queries**: `find_symbol`, `find_references`, `find_tests_for_symbol`, `inspect_code_structure`.
- **Surgical edits**: `patch_symbol`, `rename_symbol`, `ensure_import` — targeted changes that don't disturb surrounding code.
- **Text-level fallbacks** for unsupported languages or broad work: `read_file`, `write_file`, `patch_file`, `find_files`, `find_patterns`.
- **Process and environment tools**: `exec`, `eval_script`, `process`, `http` — for running commands, probing services, and working outside the file system.

### LLM backends behind a single facade

- **Provider-neutral request/response shape** (`LlmCompletionRequest` / `LlmCompletionResponse`, `LlmFinishReason`, `LlmToolDefinition`, `LlmToolCall`, `LlmToolChoice`) — the rest of the workspace never touches vendor SDK types.
- **Dual backend crates under the hood** (`rig-core` + `ai-lib`). A capability-driven selector picks the right one per provider automatically.
- **Large built-in provider catalogue**: OpenAI, Anthropic, OpenRouter, Gemini, Mistral, xAI, DeepSeek, Ollama, Groq, Together, Perplexity, Moonshot, Cohere, Azure, Qwen, HuggingFace, Replicate, z.ai, Minimax, and more.
- **User-defined providers**: declare any OpenAI- or Anthropic-compatible endpoint in config; it slots into the same facade.
- **Local-friendly.** Ollama, llama.cpp, and arbitrary proxies work out of the box via OpenAI-compatible routes.

### Strict compile-time opt-in

- **Feature flags gate entire subsystems.** `tui`, `concierge`, `log-broadcast`, and per-backend features compose independently.
- **The feature matrix is an invariant.** `--no-default-features`, each subset, and `--all-features` all compile cleanly. No hidden coupling.
- **Your binary contains only what you asked for.** No pulled-in ratatui/crossterm without `tui`, no concierge surface without `concierge`, no backend transport you didn't enable.

### In-process bus as the only integration seam

- **Everything is messages.** Commands (clients → daemon), events (daemon → many clients), logs (opt-in). No direct function calls between UI, LLM adapters, agents, and persistence for runtime coordination.
- **`ChatPath` on every event** carries full lineage — root, parent, current, depth — so observers group and filter by tree cheaply.
- **Serde-ready payloads with no channel handles or closures** — the same enums can move over a future out-of-process transport without reshape.
- **TUI is just another bus client.** A remote/out-of-process UI is a natural extension, not a rewrite.

### Local, inspectable persistence

- **Everything under `~/.xoxo/`**: `config.toml` (commented example generated on first run) and `data/` (sled database for chats + metadata).
- **No cloud dependency.** Chats, subagent transcripts, and last-used chat state live on your disk.
- **Configurable per-provider credentials** plus a current-provider / current-model selector for new root chats.

## What makes xoxo unique

A handful of deliberate choices set xoxo apart from the many terminal coding assistants in this space:

- **Subagents are specialists, not copies.** Most assistants either run a single flat chat or stream a sub-loop inline. xoxo treats spawning as a tool call, forces the parent to name the child's toolset explicitly, and hands back a structured summary — so the parent's context stays focused and subagent output can't flood the main transcript.
- **Tree-shaped chats with one identifier.** `Chat.id` *is* the agent id. `ChatPath` carries the lineage. One flat registry, one canonical id, no `AgentId` / `ConversationId` / `CallStack` triple-bookkeeping.
- **Deterministic code intelligence first, embeddings later.** `nerd` answers structural questions ("where is this defined?", "what does this file export?", "rename this symbol") through Tree-sitter ASTs, not fuzzy search. Vector search, when added, will be for candidate discovery only — the agent still verifies against concrete AST facts before editing.
- **Two-backend LLM strategy.** `rig-core` and `ai-lib` each cover a different slice of the provider landscape; xoxo keeps both behind one facade and routes per provider, so you get broad coverage without vendor-specific code leaking into the rest of the workspace.
- **In-process bus, remotable by design.** The TUI embeds the daemon today, but every message that flows between subsystems is already `serde`-clean and path-stamped — there's no in-memory-only shortcut to rip out when a future remote UI shows up.
- **Opt-in everything.** Concierge, TUI, broadcast logging, individual backends — all behind features. The default build is a small, coding-focused binary. You never pay for what you don't use.
- **Your machine, your data.** All state is local (sled under `~/.xoxo/data/`), all config is local (`~/.xoxo/config.toml`), and the transcript format is the record — no separate "handoff-only" or "memory-only" stores to drift out of sync.

## Installing

Not yet published. Once the project reaches a usable state, installation will be via `cargo install` from this repository, or by building from source:

```sh
# Headless daemon, OpenAI-compatible backend (core default)
cargo build --release

# With the terminal UI
cargo build --release -p xoxo --features tui

# With the terminal UI and the broader personal-assistant capabilities
cargo build --release -p xoxo --features "tui concierge"
```

The resulting binary is `xoxo`. Requires a recent Rust toolchain (`rust-version = "1.94"`, edition 2024).

## Configuration

Configuration lives in `~/.xoxo/config.toml` and is written with a fully-commented example on first launch. The main sections today:

- `[code_quality]` — soft rules applied by local tooling (e.g. `max_lines_in_file`).
- `[current_provider]` / `[current_model]` — which provider/model to use for newly created root chats.
- `[[providers]]` — credentials and base URLs for built-in providers, plus user-defined `kind = "other"` entries with OpenAI- or Anthropic-compatible wire protocols.
- `[ui]` — TUI-specific preferences.

For a quick start without writing config, setting `OPENROUTER_API_KEY` in the environment is enough to get the default provider selection working.

## Workspace layout

```
crates/
  bin/
    xoxo/          # binary — thin CLI, wires subsystems only
  lib/
    xoxo-core/     # core library — bus, agents, LLM facade, config, storage, tool registry
    xoxo-tui/      # optional ratatui overlay; pure bus client
  agents/
    agentix/       # shared agent/tool contracts, reusable tool bindings
    nerd-ast/      # AST-backed code intelligence (Tree-sitter, 40+ grammars)
    nerd/          # coding-assistant toolkit — always on
    concierge/     # broader personal-assistant toolkit — opt-in
```

Design notes worth reading before touching the internals:

- `docs/adr/0001-agents-and-subagents.md` — agent/subagent model, identity, spawning, handoff.
- `docs/adr/0002-deterministic-code-intelligence-for-nerd.md` — why AST-first, where embeddings fit.
- `crates/lib/xoxo-core/docs/bus.md` — bus message reference.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
