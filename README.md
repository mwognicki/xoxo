# xoxo

`xoxo` is a personal AI assistant that lives in your terminal.

It is primarily a **coding assistant** — pair-programming, reading your project, running tools, and helping you get work done at the keyboard. With an opt-in extension, it can also act as a broader **personal assistant** for tasks outside of code.

It draws inspiration from Mistral Le Vibe, OpenAI Codex, and OpenCode, with a handful of personal tweaks on top.

## Status

> **Early days.** The repository is currently at version `0.0.1` and contains the workspace skeleton. The daemon, LLM integration, and agent capabilities are still being designed and are not yet functional. There is nothing to run end-to-end yet.
>
> This README describes what `xoxo` is *meant to be*. Treat it as a direction of travel, not a feature list.

## What it is

`xoxo` is a single binary with two faces:

- **`nerd`** — the coding assistant. Always available. Aimed at working inside a repository: reading code, editing files, running commands, and helping you think through changes.
- **`concierge`** — a broader personal assistant. Opt-in at build time. Aimed at tasks outside of pure coding.

Under the hood, both share the same core: one LLM session, one set of tools, one internal message bus. You choose which capabilities are compiled in; you are never forced to carry what you don't want.

## How you'll use it

Two ways to run it, sharing the same engine:

- **`xoxo daemon`** — runs headless. Useful for scripting, integrations, or attaching a UI of your own.
- **`xoxo tui`** — a terminal UI (ratatui) that embeds the daemon in the same process. This is the everyday interactive mode.

Both talk to the same internal bus, so a future remote/out-of-process UI is a natural extension rather than a rewrite.

## LLM backends

`xoxo` is designed to work with multiple LLM providers behind a single interface:

- **OpenAI-compatible** endpoints (OpenAI itself, Ollama, llama.cpp, local proxies) — included by default.
- **Anthropic**, **Mistral**, and others — planned, each as an independent, pluggable backend.

You pick backends at build time via Cargo features, so your binary only contains what you actually use.

## Installing

Not yet published. Once the project reaches a usable state, installation will be via `cargo install` from this repository, or by building from source:

```sh
# Default build: coding assistant + OpenAI-compatible backend
cargo build --release

# With the terminal UI
cargo build --release --features tui

# With the terminal UI and the broader personal-assistant capabilities
cargo build --release --features "tui concierge"
```

The resulting binary is `xoxo`.

## Configuration

Configuration (API keys, default model, enabled backends, etc.) will live in a user config file. The exact shape is not yet finalized — this section will be filled in as the project stabilizes.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
