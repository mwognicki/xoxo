# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Governing rule — maintainers lead

The `maintainer-interaction` skill is first-class here. Every interaction, slice, report, and commit complies with it. This file declares no exceptions to it.

- Agents operate as engineers: they propose, execute named work, and carry the mechanical load. They hold no final decision authority. Architecture decisions belong to the maintainer, always.
- ADR regime: anything architectural — the bus contract, agent/chat model, LLM facade shape, feature gating, storage — lands as an ADR in `docs/adr/` with maintainer sign-off before code. Agents may draft an ADR; they never self-approve one and never route around an existing one.

## Project

`xoxo` is a personal AI assistant inspired by Mistral Le Vibe, OpenAI Codex, and OpenCode, tailored with custom tweaks. It is primarily a coding assistant (`nerd`), with an **opt-in** path to act as a more versatile general-purpose assistant (`concierge`).

The repository is a Cargo workspace that produces one binary (`xoxo`). Behavior is varied through features on the binary crate and backend features on `xoxo-core`.

> The working directory is still named `vibin/`. The crates and binary have been renamed to `xoxo*`; the outer folder will catch up eventually.

## Workspace layout

```
crates/
  bin/
    xoxo/          # binary — thin CLI, wires subsystems only
  lib/
    xoxo-core/     # core library — bus, agents, LLM facade, config, storage, tool registry
    xoxo-tui/      # optional ratatui overlay; bus client only
  agents/
    agentix/       # shared agent/tool contracts, reusable tool-binding helpers
    nerd-ast/      # AST-backed code intelligence (Tree-sitter, 40+ grammars)
    nerd/          # coding-assistant toolkit — always compiled in
    concierge/     # broader personal-assistant toolkit — opt-in
```

Agents (`nerd`, `concierge`) expose their capabilities to the daemon as tools/hooks against contracts defined in `xoxo-core` (via the shared `agentix` crate). They do not reach into each other, and the daemon does not reach into their internals. `nerd-ast` is a leaf library of Tree-sitter bindings consumed by `nerd` for deterministic code tooling.

## Runtime topology

- The **daemon** is the core. It owns LLM sessions, tool execution, per-agent state, chat persistence, and the in-process **message bus** (tokio channels: `broadcast` for events, `mpsc` for commands).
- `xoxo daemon` runs the daemon headless.
- `xoxo tui` (feature `tui`) **embeds the daemon in-process** and attaches a ratatui overlay as a bus client. There is no socket/IPC layer — the TUI and daemon share the same process and the same in-memory bus.
- `xoxo dev …` exposes small developer utilities (e.g. dump a raw persisted chat snapshot, purge the local sled store). These go through `xoxo-core` APIs, not around them.
- The bus is the one and only integration seam between subsystems. UI, LLM adapters, agents, and persistence all produce/consume bus messages. Do not add direct function calls that bypass it for runtime coordination.

## Agents, subagents, and chats

The agent/subagent design is pinned by `docs/adr/0001-agents-and-subagents.md`; treat that ADR as the source of truth and read it before touching `xoxo-core::agents`, `xoxo-core::bus`, or anything that spawns or addresses agents. Key invariants to keep in mind while working:

- **One agent ↔ one chat.** Agent identity is `Chat.id: Uuid`. There is no separate `AgentId`.
- **`ChatPath`** (root-first `Vec<Uuid>`) carries lineage on every handle and every bus event. Depth, root, parent, and current id all derive from it.
- **Flat `HandleRegistry`** keyed by `Chat.id`. Tree shape is encoded on handles via `ChatPath`, not in the map.
- **External user input goes to roots only.** Ingress must check `handle.path().depth() == 0` before forwarding `Command::SendUserMessage`.
- **Subagent spawning is a tool call, not a bus command.** `spawn_subagent` (paired with `list_available_tools`) is the single entry point. It is not in the default toolset; it is granted only when `depth < max_depth`.
- **Handoff is a structured `SubagentHandoff`, not a string.** Subagents do not stream into the parent context.
- **Every chat is persisted in full**, subagent chats included. The transcript is the record — there is no separate "handoff" storage path.

## LLM facade

- All backend-specific code lives inside `xoxo-core::llm` behind a provider-neutral facade (`LlmCompletionRequest` / `LlmCompletionResponse`, `LlmFinishReason`, `LlmToolDefinition`, `LlmToolCall`, `LlmToolChoice`). The rest of the workspace talks to this facade, never to a concrete backend type.
- Two backend families currently sit behind the facade: `rig-core` and `ai-lib`. A selector picks the right one per provider based on a capability table.
- Built-in providers are catalogued through `inventory`-submitted `ProviderDefinition` entries (OpenAI, Anthropic, OpenRouter, Gemini, Mistral, xAI, DeepSeek, Ollama, Groq, Together, Perplexity, Moonshot, and others). Adding a new built-in provider means adding an `inventory::submit!` entry and, if needed, adapting the selector — not reshaping the facade.
- User-defined providers (`kind = "other"` in config) declare a wire-protocol compatibility (`open_ai` or `anthropic`) and are routed through the matching compatible backend.

## Cargo features

On `xoxo` (the binary):

- `tui` — pulls in `xoxo-tui` and enables the `Tui` subcommand. Must remain strictly optional; no non-TUI code may `use` ratatui/crossterm. Enabling `tui` also turns on `xoxo-core/log-broadcast` so the overlay can render a diagnostics pane.
- `concierge` — pulls in the `concierge` agent crate. When off, the binary contains coding-assistant capabilities only.

On `xoxo-core`:

- `openai` (default) — enables the OpenAI-compatible HTTP adapter (reqwest + eventsource-stream). Additional LLM protocol features will be added as sibling features as they stabilise, each independent and pluggable through the LLM facade. Built-in provider *definitions* (via `inventory`) are always present; feature flags gate the *transport code* behind them.
- `log-broadcast` — opt-in `tracing` layer that fans log records out over the bus. Enabled transitively by any client that wants to consume logs as bus messages (e.g. the TUI).

On `nerd`:

- `extension-language-detection` (default) — file-extension-based language detection for code-intelligence tools. Off-path language detection may replace or augment this later.

Invariant: the workspace must build with any combination of these features (including `--no-default-features`), and feature-gated code must not leak symbols into always-on modules.

## Persistence and config

- User config lives at `~/.xoxo/config.toml`. It is generated from `Config::toml_example()` on first launch and covers `code_quality`, `current_provider`, `current_model`, `[[providers]]`, and `[ui]`.
- Local data lives under `~/.xoxo/data/` as a sled database (chats tree + metadata tree). `Storage` in `xoxo-core::storage` is the only sanctioned entry point; the `xoxo dev dump-stored-chat` / `xoxo dev purge-storage` subcommands go through it too.
- App state (current provider, current model selection at runtime) is loaded via `AppStateRepository` and can diverge from config between launches.
- `~/.xoxo/` is also the root for any future per-user state — prefer adding new subdirectories here over scattering paths.

## Architectural rules specific to this project

- **Coding vs. concierge separation.** `nerd` (coding) is always compiled in. `concierge` (broader capabilities) is only compiled when the `concierge` feature is on. The rest of the workspace must build and run without it.
- **LLM adapters behind the facade.** All backend-specific code (HTTP, auth, streaming decode, provider quirks) stays inside `xoxo-core::llm`. Adding a backend = adding a provider definition (+ a feature if the transport is new) and, if needed, a selector branch. No changes elsewhere.
- **Bus messages are the public contract between subsystems.** Treat `BusPayload`, `Command`, `ToolCallEvent`, and `ToolCallKind` as a stable surface. Extend (new variants) rather than mutate existing ones. `ToolCallKind` in particular is the extension point for new "privileged" tool classes (subagent spawn today; chat compaction reserved).
- **Every bus event carries a `ChatPath`.** Do not introduce parallel ancestry fields. Observers that need tree grouping use `path.root_id()`.
- **Spawning is a tool call, not a bus command.** There is no `SubagentSpawned` / `SubagentCompleted` event. Spawn observability goes through `ToolCallStarted { kind: SpawnSubagent { .. } }` + the matching `Completed`/`Failed`.
- **TUI is a bus client, not a privileged one.** The TUI must not reach into daemon internals; it subscribes to events and sends commands like any other client. This keeps a future out-of-process TUI (over a socket) a drop-in change.
- **Agent crates depend on `xoxo-core` / `agentix` only.** They do not depend on the binary crate, on the TUI, or on each other. They expose capabilities through `xoxo-core` contracts. `nerd-ast` is the one exception in the reverse direction: `nerd` depends on it as a pure library.

## This file's job — entry point, not kitchen sink

Root agentic guidance does exactly two things:

1. **Research entry point.** Every codebase crawl (`deep-research` skill: top-down, spider-like) starts here. What this file reveals picks the next read.
2. **High-level map.** What has landed and how it fits together, at the level a first read needs.

What does not belong here:

- Template placeholders, proposed crate layouts, "future structure" notes. Structure is a maintainer decision with ADR provenance; guidance documents what exists, never what might. If it isn't landed, it isn't written down.
- Any rule that belongs to a skill. This file points at skills; it does not restate or override them.

**Reference over inline.** When material belongs at root visibility but would crowd this file — design deep-dives, long rationale — it moves to a nested file and root carries a one-line pointer. Design record lives under `docs/adr/`; dependency findings under `docs/references/` (per `deep-research`).

## Keeping guidance current

Guidance is living documentation, and the root file goes stale easiest because it sits farthest from the code. When any of these happen, the affected guidance file is updated **in the same slice**:

- a crate lands, moves, or leaves → update the map here
- the bus contract or facade shape changes → update here and the owning ADR
- guidance and reality disagree → reality wins; fix the guidance and say so in the slice report

## Skills

Skill content lives in `.agents/skills/` — the single source of truth, harness-agnostic. Harness-specific directories (`.claude/skills/`) hold **symlinks only**. To add a skill: put content in `.agents/skills/<name>/`, then symlink `../../.agents/skills/<name>` from each harness directory. Never place skill content directly in a harness directory.

Loaded skills:

- `maintainer-interaction` — every prompt. Slicing, green lights, commits, reports; all `reference/` files are mandatory reading.
- `rust-best-practices` — writing, reviewing, or refactoring Rust here. Highlights that frequently bite:
  - `#![deny(warnings)]` at every crate root; no un-commented `#[allow(...)]`.
  - `thiserror` for library errors, `anyhow` for binary/CLI glue.
  - No `unwrap()`/`expect()` in production paths without a `// SAFETY:` or `// INVARIANT:` comment.
  - 400-line soft cap per file; split before it hurts.
  - No `sleep`-based synchronization in tests — use channels/barriers.
- `architecture-best-practices` — architecture-level decisions across crates/modules. Default heading; `rust-best-practices` restates it Rust-idiomatically and may override individual rules.
- `deep-research` — any research-shaped task: codebase questions, cross-file reasoning, dependency discovery. Top-down spider crawl from entry points; no fan-out without approval.

## Build & test

Standard cargo. Useful invocations:

```
cargo check --workspace                        # default features
cargo check --workspace --all-features         # full matrix including tui + concierge
cargo build -p xoxo --features tui             # binary with the TUI overlay
cargo build -p xoxo --features "tui concierge"
cargo build -p xoxo --no-default-features      # binary-level default is empty; core keeps `openai`
cargo test --workspace --all-features
cargo test <test_name> -- --nocapture          # single test
cargo clippy --workspace --all-features -- -D warnings
cargo fmt
```

When adding a feature flag, verify the matrix: `--no-default-features`, each backend alone, and `--all-features` must all compile.

## Status

Repository is at `0.0.1`. The bus, chat model, `ChatPath`, `AgentHandle` / `HandleRegistry`, `AgentSpawner`, the LLM facade, sled persistence, and a sizeable set of `nerd` coding tools (text + AST-backed) are implemented and partially wired. Subagent support, the `spawn_subagent` tool, depth policy, and shared per-tree execution context are at various stages per the ADR's milestones — check `docs/adr/0001-agents-and-subagents.md` and recent `.changelogs/` entries before assuming behaviour.

When in doubt: read the ADRs, read the current code, and propose a design before inventing new contracts.
