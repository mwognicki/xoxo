# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
    xoxo-core/     # core library — bus, LLM trait, config, shared contracts
    xoxo-tui/      # optional TUI overlay (ratatui); bus client only
  agents/
    nerd/          # coding-assistant toolkit — always on
    concierge/     # broader personal-assistant toolkit — opt-in
```

Agents (`nerd`, `concierge`) expose their capabilities to the daemon as tools/hooks against contracts defined in `xoxo-core`. They do not reach into each other, and the daemon does not reach into their internals.

## Runtime topology

- The **daemon** is the core. It owns the LLM session, tool execution, and state, and always communicates internally via an in-process **message bus** (tokio channels: `broadcast` for events, `mpsc` for commands).
- `xoxo daemon` runs the daemon headless.
- `xoxo tui` (feature `tui`) **embeds the daemon in-process** and attaches a ratatui overlay as a bus client. There is no socket/IPC layer — the TUI and daemon share the same process and the same in-memory bus.
- The bus is the one and only integration seam between subsystems. UI, LLM adapters, agents, and persistence all produce/consume bus messages. Do not add direct function calls that bypass it for runtime coordination.

## Cargo features

On `xoxo` (the binary):

- `tui` — pulls in `xoxo-tui` and enables the `Tui` subcommand. Must remain strictly optional; no non-TUI code may `use` ratatui/crossterm.
- `concierge` — pulls in the `concierge` agent crate. When off, the binary contains coding-assistant capabilities only.

On `xoxo-core`:

- `openai` (default) — enables the OpenAI (and OpenAI-compatible: ollama, llama.cpp, proxies) LLM backend. Additional backends (Anthropic, Mistral) will be added as sibling features, each independent and pluggable through a single `LlmBackend` trait.

Invariant: the workspace must build with any combination of these features (including `--no-default-features`), and feature-gated code must not leak symbols into always-on modules.

## Architectural rules specific to this project

- **Coding vs. concierge separation.** `nerd` (coding) is always compiled in. `concierge` (broader capabilities) is only compiled when the `concierge` feature is on. The rest of the workspace must build and run without it.
- **LLM adapters behind a trait.** All backend-specific code (HTTP, auth, streaming decode) stays inside its adapter module in `xoxo-core`. The rest of the workspace talks to `dyn LlmBackend` only. Adding a backend = adding a feature + a module, no changes elsewhere.
- **Bus messages are the public contract between subsystems.** Treat the message enums as a stable surface. Extend (new variants) rather than mutate existing ones.
- **TUI is a bus client, not a privileged one.** The TUI must not reach into daemon internals; it subscribes to events and sends commands like any other client. This keeps a future out-of-process TUI (over a socket) a drop-in change.
- **Agent crates depend on `xoxo-core` only.** They do not depend on the binary crate, on the TUI, or on each other. They expose capabilities through `xoxo-core` contracts.

## Rust conventions

Rust style, error-handling, and architecture rules for this repo live in `.claude/skills/rust-best-practices/SKILL.md`. Load that skill when writing, reviewing, or refactoring Rust here. Highlights that frequently bite:

- `#![deny(warnings)]` at every crate root; no un-commented `#[allow(...)]`.
- `thiserror` for library errors, `anyhow` for binary/CLI glue.
- No `unwrap()`/`expect()` in production paths without a `// SAFETY:` or `// INVARIANT:` comment.
- 400-line soft cap per file; split before it hurts.
- No `sleep`-based synchronization in tests — use channels/barriers.

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

Repository is at initial scaffolding (version `0.0.1`): workspace + crate skeletons with empty `lib.rs` files. The bus, config shape, LLM trait, and agent contracts are **not yet designed** — do not invent them. Propose a design to the user before writing them.
