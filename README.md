# xoxo

`xoxo` is a personal AI assistant that lives in your terminal.

It is primarily a **coding assistant** — pair-programming, reading your project, running tools, and helping you get work done at the keyboard. With an opt-in extension, it can also act as a broader **personal assistant** for tasks outside of code.

It draws inspiration from Mistral Le Vibe, OpenAI Codex, and OpenCode, with a handful of personal tweaks on top.

## Status

> **Early days.** The repository is at version `0.0.1` and the design is still consolidating. The core runtime (bus, agents/subagents, LLM facade, persistence, coding tools) is in place and partially wired, but the end-to-end experience is a work in progress — rough edges and missing pieces are expected.
>
> Treat this README as a description of the system's direction and current shape, not a polished feature list.

## What it is

`xoxo` is a single binary with two faces:

- **`nerd`** — the coding assistant. Always compiled in. Aimed at working inside a repository: reading code, editing files, running commands, running AST-backed code-intelligence queries, and helping you think through changes.
- **`concierge`** — a broader personal assistant. Opt-in at build time. Aimed at tasks outside of pure coding. Currently a scaffolded crate; capabilities are still being filled in.

Under the hood, both share the same core: one daemon, one in-process message bus, one LLM facade, one tool registry, one persistence layer. You choose which capabilities are compiled in; you are never forced to carry what you don't want.

### Agents and subagents

An "agent" in `xoxo` is one isolated chat with its own system prompt and toolset. The runtime lets agents form a tree: a root agent can spawn specialised **subagents** for bounded sub-tasks through a `spawn_subagent` tool, collect a structured handoff, and fold the result back into its own transcript. Depth is capped, subagent toolsets are explicit (not inherited), and every subagent's activity is observable on the same bus as the root. See `docs/adr/0001-agents-and-subagents.md` for the full design.

## How you'll use it

Two ways to run it, sharing the same engine:

- **`xoxo daemon`** — runs the daemon headless. Useful for scripting, integrations, or attaching a UI of your own.
- **`xoxo tui`** — a terminal UI (ratatui) that embeds the daemon in the same process. This is the everyday interactive mode.

Both talk to the same internal bus, so a future remote/out-of-process UI is a natural extension rather than a rewrite.

There is also a `xoxo dev` subcommand group with small developer utilities, e.g. dumping a raw persisted chat snapshot from the local sled store or purging the store entirely.

## LLM backends

`xoxo` is designed to work with multiple LLM providers behind a single interface. Internally, a provider registry (populated via `inventory`) catalogues built-in providers (OpenAI, Anthropic, OpenRouter, Gemini, Mistral, xAI, DeepSeek, Ollama, Groq, Together, Perplexity, Moonshot, and more) and routes each to one of two backend crates (`rig-core` or `ai-lib`) depending on capabilities. User-defined providers can also be declared in config with either OpenAI- or Anthropic-compatible wire protocols.

Concretely:

- **OpenAI-compatible** endpoints (OpenAI itself, OpenRouter, Ollama, llama.cpp, local proxies, any OpenAI-compatible gateway) are usable out of the box.
- **Anthropic** and other mainstream hosted providers are reachable through the same facade.
- `openai` is the default Cargo feature on `xoxo-core` today; additional backends will be promoted to independent features as they stabilise.

All backend-specific code (HTTP, auth, streaming decode) is isolated behind a single facade. The rest of the workspace speaks a provider-neutral `LlmCompletionRequest` / `LlmCompletionResponse` shape.

## Code intelligence

`nerd` does not rely only on text search and full-file reads. It ships deterministic, AST-backed code-intelligence tools (Tree-sitter grammars for 40+ languages) for things like:

- Finding symbol definitions and references
- Inspecting a file's top-level structure
- Locating tests relevant to a symbol
- Patching or renaming a specific symbol in place
- Ensuring an import exists without disturbing the rest of the file

These sit alongside the usual text-level tools (`read_file`, `write_file`, `patch_file`, `find_files`, `find_patterns`, `exec`, `eval_script`, `process`, `http`, …). The rationale and layering are described in `docs/adr/0002-deterministic-code-intelligence-for-nerd.md`.

## Storage

Persistent state lives under `~/.xoxo/`:

- `~/.xoxo/config.toml` — user configuration (see below). Created with a commented example on first run.
- `~/.xoxo/data/` — a local [sled](https://github.com/spacejam/sled) database for chat transcripts and small app-state entries (e.g. the last-used chat id).

Chats are persisted in full, including subagent transcripts, so a session can be resumed from disk and a tree can be reconstructed from any node.

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

Configuration lives in `~/.xoxo/config.toml` and is written with a fully-commented example on first launch. The shape is still evolving, but the main sections today are:

- `[code_quality]` — soft rules applied by local tooling (e.g. `max_lines_in_file`).
- `[current_provider]` / `[current_model]` — which provider/model to use for newly created root chats.
- `[[providers]]` — credentials and base URLs for built-in providers, plus user-defined `kind = "other"` entries that declare OpenAI- or Anthropic-compatible wire protocols.
- `[ui]` — TUI-specific preferences.

For a quick start without writing config, setting `OPENROUTER_API_KEY` in the environment is enough to get the default provider selection working.

## Workspace layout

Cargo workspace, one binary, a small set of libraries, and a couple of agent crates:

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

## License

Dual-licensed under MIT or Apache-2.0, at your option.
