# ADR 0001 — Agents and subagents

- Status: proposed
- Date: 2026-04-19
- Scope: `xoxo-core` runtime, `xoxo` binary wiring, bus contracts

## Context

`xoxo` is built around LLM-backed *agents*. An agent is an isolated context
of user↔LLM interaction — a chat transcript, with interleaved tool calls and
their results, bounded by a system prompt and a toolset. The runtime must
allow more than one agent to exist concurrently, arranged as a tree: a **main
agent** can delegate parts of a task by spawning **subagents**, and those
subagents can in turn spawn their own.

`nerd` (always compiled) and `concierge` (feature-gated) are *main-agent
blueprints* — top-level orchestrators. Subagents are not additional
instances of those; they are specialised delegates constructed per-spawn,
with their own system prompt, toolset, and — optionally — a different model.
Reusing the same configuration as the parent would defeat the point.

This ADR pins down the shape of agents, subagents, spawning, and bus
interactions before any of it is implemented. The repository is at
`0.0.1` scaffolding; the architectural rules in `CLAUDE.md` forbid inventing
runtime contracts without prior design, and this document is that design.

A working solution from a sibling project (now placed under `.not-crates/`)
informed many decisions. It is fundamentally NATS-based and carries features
`xoxo` does not need yet (MongoDB persistence, blueprint store CRUD,
title-generator inline agents); its shape is translated here to the
in-process bus world described in `CLAUDE.md`.

## Decisions

### 1. Identity and addressing

- One agent ↔ one chat. Agent identity collapses into chat identity: an
  agent *is* addressed by its `Chat.id` (a `Uuid`). No dedicated
  `AgentId`/`ConversationId` type — that would duplicate the canonical id
  already owned by `Chat` and reintroduce the two-identifier problem we
  just collapsed. The reference implementation's `CallStack` is also
  redundant under this 1:1 mapping.
- Lineage is carried by a `ChatPath` on every handle and every bus event:
  ```rust
  struct ChatPath(Vec<Uuid>); // root-first, current is last
  ```
  Accessors (minimal set):
  - `root_id() -> &Uuid`
  - `parent_id() -> Option<&Uuid>`
  - `current() -> &Uuid`
  - `depth() -> usize` (0 at the root)
  - `child(id: Uuid) -> ChatPath` (push constructor used by the spawner)

  Callers that need a "root?" check write `path.depth() == 0` directly.
- The path is the only ancestry primitive. `Chat.parent_chat_id` (see §6)
  stores the same information in persisted form.

### 2. Runtime topology

- **Daemon** owns the LLM sessions, the tool registry, persistence handles,
  and the bus. It is always in-process. `xoxo daemon` runs it headless;
  `xoxo tui` embeds it and attaches the ratatui overlay as a bus client.
- **Agent loop** is a method `AgentRunner::run(self)` on a per-agent struct,
  spawned as one `tokio::task` per live agent. Runners are identical for
  roots and subagents — the spawn path does not parameterise the runner.
- **Bus** is tokio channels:
  - `broadcast::Sender<BusEvent>` for fan-out events (assistant messages,
    tool-call lifecycle, errors). Every agent runner holds a clone and
    stamps every publish with its `ChatPath`.
  - `mpsc::Sender<Command>` per **root agent only**, delivered through its
    `AgentHandle` (see §3). The runner consumes `Command`; user-authored
    chat content is carried inside `Command::SendUserMessage(UserMessage)`,
    recorded into conversation history, and then re-emitted through the
    normal bus/chat event path. Subagents do not receive commands; their
    input is the initial prompt given at spawn time, and their output is
    the handoff (see §4).
- **Spawner** (`Arc<dyn Spawner>`) is the sole creator of agents. It is held
  by the daemon and — via `ToolContext` — by every runner, so that the
  `spawn_subagent` tool can create children through the same path the
  daemon uses to create roots.
- **Shared per-tree execution context** is one `AgentExecutionContext` per
  chat tree, keyed by the tree's root id, `Arc`-shared into every runner
  in that tree. It owns long-lived process handles, bash sessions, and
  similar per-tree state. When a subagent wants to terminate hard (future
  work), anything it started is reachable through this shared registry.

### 3. Handles and registry

- `AgentHandle` is the only way to talk to an agent from outside its task:

  ```rust
  trait AgentHandle: Send + Sync {
      fn chat_id(&self) -> &Uuid;
      fn path(&self) -> &ChatPath;
      fn send(&self, cmd: Command) -> HandleFuture<'_, Result<(), HandleError>>;
      fn shutdown(&self) -> HandleFuture<'_, Result<(), HandleError>>;
  }
  ```

- `HandleRegistry` is **flat**: `HashMap<Uuid, Arc<dyn AgentHandle>>`.
  Every live agent at every depth is registered. Nestedness is not encoded
  in the map; it is encoded on the handles via `ChatPath`. Helpers:

  ```rust
  fn roots(&self) -> Vec<Arc<dyn AgentHandle>>;
  fn children_of(&self, parent: &Uuid) -> Vec<Arc<dyn AgentHandle>>;
  fn subtree(&self, root: &Uuid) -> Vec<Arc<dyn AgentHandle>>;
  ```

- **Routing rule**: external user input is only accepted for roots. The
  ingress caller checks `handle.path().depth() == 0` before `send`; `send`
  itself rejects `Command::SendUserMessage(..)` on non-root handles as a
  second line of defence.
- **Observability**: any UI/observer enumerates `HandleRegistry` and groups
  by `path().root_id()` to render the tree — no extra tree-tracking
  structure needed.

### 4. Spawning: tool-based, handoff as tool result

- Spawning is a **tool call**, not a bus command. The tool is
  `spawn_subagent`. It is not in the default toolset; agents only see it
  when the parent explicitly grants it at spawn time *and* the depth
  budget allows it (see §5). A tiny set of universally-granted tools
  exists, and `spawn_subagent` is deliberately not in it.
- Paired with `spawn_subagent`, the tool `list_available_tools` is granted.
  Its purpose is *preparation for spawning*: it reveals the grantable pool
  so the parent can pick a specialist's toolset. Agents that cannot spawn
  do not receive it — they already know every tool available to them from
  their own construction.
- The `spawn_subagent` tool's parameters (ad-hoc spec):

  ```json
  {
    "name": "spawn_subagent",
    "parameters": {
      "task": "string",               // what the subagent must accomplish
      "system_prompt": "string",      // role/instructions for the specialist
      "tools": ["<name>", ...],       // explicit allowlist from grantable pool
      "model": "string (optional)",   // defaults to config's default subagent model
      "context": "string (optional)"  // extra seed context (files, excerpts)
    }
  }
  ```

- **Non-streaming.** A subagent does not stream into the parent's context.
  It runs its own loop to completion and hands back a single tool result.
  The parent LLM sees exactly one tool-result turn for the spawn call.
- **Subagents still publish on the bus.** Their observability events
  (assistant messages, tool calls, completion) are broadcast and stamped
  with their `ChatPath`. This is one-sided: subagents emit, they do not
  consume commands.
- **Handoff shape.** The subagent's result is not a plain string. It is a
  structured envelope:

  ```rust
  struct SubagentHandoff {
      kind: HandoffKind,              // Completed, Failed, Cancelled
      chat: Chat,                     // the subagent's full persisted transcript
      summary: Option<String>,        // short message surfaced to the parent LLM
      observability: Option<CostObservability>,
  }
  ```

  The parent LLM sees the serialised form as the tool result (typically the
  `summary`, or a compact rendering of `chat`'s final assistant event). UI
  and persistence can walk `chat` for full detail.
- `HandoffKind::Cancelled` is part of the initial design, not a placeholder.
  It covers interrupted execution such as application shutdown while a
  subagent is in flight. Cancellation still persists the subagent chat up to
  the interruption boundary and returns a handoff the parent/runtime can
  reason about.

### 5. Depth policy

- `max_depth = 2` by default, `max_depth = 3` as the current hard cap.
  Configurable, bounded by the cap.
- Enforcement: the *spawning tool tool-set* is computed at spawn time.
  A child at depth `d` receives `spawn_subagent` + `list_available_tools`
  only when `d < max_depth`. Leaves (at the cap) simply do not have those
  tools, so they cannot ask.
- Depth is derived from `ChatPath::depth()`; no separate counter.

### 6. Subagents inherit tree state but not toolsets

- A subagent's toolset is built fresh from the `tools` allowlist in the
  spawn call (intersected with the grantable pool). It does **not** inherit
  the parent's toolset. The whole point of spawning is specialisation.
- Model resolution: `spec.model = request.model.or(config.default_subagent_model)`.
  The parent never silently inherits its own model to the child.
- A subagent **does** receive (via `ToolContext`):
  - The shared per-tree `AgentExecutionContext` (so started processes are
    tracked one-tree-wide).
  - The `Arc<dyn Spawner>` (so it can itself spawn, if granted).
  - A `broadcast::Sender<BusEvent>` clone stamped with its path.

### 7. Chat persistence and tree reconstruction

- Every chat is persisted, including subagent chats. Adding to
  `crates/lib/xoxo-core/src/chat/structs.rs`:
  - `Chat.parent_chat_id: Option<Uuid>` — self-describing transcripts; the
    tree can be reconstructed from any node without loading the root first.
  - `Chat.spawned_by_tool_call_id: Option<ChatToolCallId>` — pins the
    parent-side tool call that produced this subagent. Lets UI render the
    tool call as a "zoom into subagent" affordance.
  - `ToolCallEvent::Started` grows a `kind: ToolCallKind` field mirroring
    §8. The persisted transcript is isomorphic to the bus stream: replaying
    a stored chat must yield the same information a live observer had. No
    separate "spawned subagent" record exists in the transcript — the tool
    call *is* the record.
- The `SubagentHandoff` returned to the parent *is* a view of the persisted
  subagent `Chat` plus handoff-specific metadata. No separate "handoff-only"
  storage path; the transcript is the record.

### 8. Bus events

- Every broadcast event carries `ChatPath` (from which `root_id`,
  `parent_id`, `current`, and `depth` are derived). This applies to the
  client-observed bus stream, including chat/message events emitted by
  subagents while they run. No other ancestry fields are needed.
- Shape:

  ```rust
  struct BusEnvelope<T> {
      path: ChatPath,
      payload: T,
  }

  type BusEvent = BusEnvelope<BusPayload>;

  enum BusPayload {
      Message(ChatTextMessage),       // agent/user/system text turns
      ToolCall(ToolCallEvent),        // the only tool-call channel — see below
      AgentShutdown,
      Error(ErrorPayload),
  }

  enum ToolCallEvent {
      Started(ToolCallStarted),
      Completed(ToolCallCompleted),
      Failed(ToolCallFailed),
  }

  struct ToolCallStarted {
      tool_call_id: ChatToolCallId,
      tool_name: String,
      arguments: serde_json::Value,
      kind: ToolCallKind,             // discriminator
  }

  enum ToolCallKind {
      Generic,
      SpawnSubagent {
          child_chat_id: Uuid,
          child_path: ChatPath,
          spec_summary: String,       // short human-readable spec
      },
      // Reserved: CompactChat { .. } when compaction lands.
  }
  ```

- **Single source of truth for spawning.** Spawning *is* a tool call, so
  there is no parallel `SubagentSpawned` event. The fact that a subagent
  started is expressed as `ToolCallStarted { kind: SpawnSubagent { .. } }`.
  Tree-view consumers (UI, trace) match on `kind`; transcript-view consumers
  ignore it. Join by `tool_call_id` between `Started` and
  `Completed`/`Failed`.
- **`kind` lives on `Started` only.** The follow-up `Completed`/`Failed`
  carry `tool_call_id` + `tool_name`; re-declaring `kind` would duplicate
  information that is already 100% determined by the matching `Started`.
- Event enums are an extension point: new variants of `BusEvent` and new
  `ToolCallKind` subtypes are non-breaking additions; mutating existing
  variants is breaking. This is the stable public contract between
  subsystems (`CLAUDE.md` invariant).

### 9. Lifecycle

- **Spawn**: spawner resolves the spec, allocates a new `Uuid` chat id,
  builds child `ChatPath`, constructs `ToolContext` with inherited tree
  state and the child's path, creates an `AgentHandle` + inbound mpsc,
  registers the handle, persists the `Chat` shell, `tokio::spawn`s the
  runner task, sends the initial prompt. The parent-side
  `ToolCallEvent::Started { kind: SpawnSubagent { child_chat_id,
  child_path, .. }, .. }` is emitted by the `spawn_subagent` tool *before*
  it awaits the handoff — that event is the signal to observers that the
  child is live.
- **Run**: non-streaming LLM loop for subagents; events still published to
  the bus for observability of both roots and subagents. Processes the
  runner starts register themselves in the per-tree
  `AgentExecutionContext`.
- **Complete**: subagent returns its `SubagentHandoff` through a
  `oneshot::Sender<SubagentHandoff>` held by the spawn tool's `execute`
  call. The spawn tool serialises it into the parent's tool-result turn
  and emits `ToolCallEvent::Completed` (or `Failed`) for the spawn call.
- **Persist**: final `Chat` written. Handle removed from registry.
- **Terminate**: normal exit on handoff; error → handoff kind `Failed` and
  `ToolCallEvent::Failed`; cancellation/interrupted execution → handoff kind
  `Cancelled`, final `Chat` still persisted.
- **Shutdown cascade**: runner drop → `AgentExecutionContext::shutdown()` →
  bash killed, processes in registry terminated. The per-tree context only
  shuts down when its last agent exits.

### 10. Feature boundaries

- `xoxo-core` defines: `Chat` (extended), `ChatPath`, `AgentHandle`,
  `HandleRegistry`, `Spawner`, `ToolContext`, `AgentExecutionContext`,
  `BusEvent` (incl. `ToolCallEvent`, `ToolCallKind`), `Command`, and the
  `spawn_subagent` + `list_available_tools` tool structs/schemas.
- `nerd`, `concierge` depend on `xoxo-core` only, and plug in via tool and
  main-blueprint contracts. Neither knows about the daemon or TUI.
- `xoxo-tui` is a bus client: subscribes to events, sends commands. No
  privileged access. A future out-of-process TUI must be a drop-in change.
- No ratatui/crossterm imports outside `xoxo-tui`; no NATS/mongo anywhere;
  `--no-default-features` and `--all-features` both build.

## Alternatives considered

- **Spawn as a bus command rather than a tool.** Rejected: the LLM already
  emits tool calls; routing spawns through tools keeps the LLM contract
  narrow (tools in, tool results out), keeps the bus protocol small, and
  gives the parent a natural point in its transcript to hold the handoff.
- **Separate `AgentId` and chat id.** Rejected: 1:1 mapping holds here, so
  two identifiers only adds accounting cost without buying flexibility we
  need now. `Chat.id` is the agent id.
- **Parallel `SubagentSpawned` / `SubagentCompleted` bus events.**
  Rejected: violates single-source-of-truth — spawning *is* a tool call,
  so it must appear exactly once in the event stream. Instead, a
  `ToolCallKind` discriminator on `ToolCallStarted` carries the
  spawn-specific fields (child id, child path, spec summary). Tree-view
  consumers match on `kind`; transcript-view consumers ignore it. Future
  privileged tool classes (e.g. chat compaction) slot into the
  same enum without widening the event surface.
- **Nested registries (root registry + child tracking per root).**
  Rejected: flat registry + `ChatPath` on handles supports every
  required query (routing, subtree walks, per-tree render) with one data
  structure.
- **Per-kind spawn tools (`spawn_nerd`, `spawn_concierge`).** Rejected: the
  subagent abstraction is *ad-hoc specialists*, not "smaller copies of main
  agents". Main-agent blueprints are orchestrators, not subagent
  templates.
- **`spawn_subagent_from_blueprint` tool.** Rejected: blueprints, when
  introduced, fill in the existing `spawn_subagent` parameters from storage.
  One tool, one code path.
- **Streaming subagent output into parent context.** Rejected: contradicts
  the "tool-call-shaped handoff" invariant and forces the parent LLM to
  reason about an unbounded sub-stream mid-turn. Observability uses the bus
  instead.
- **Inheriting parent's toolset.** Rejected: specialisation is the point.
  Fresh, narrower toolsets are the default; parent must name each tool.

## Open points (deliberately deferred)

- **Hard cancellation policy details for in-flight subagents.** Cancellation
  is a valid v1 outcome and must persist chat state, but the exact mechanics
  (graceful drain vs. hard kill, cooperative cancellation tokens in tool
  calls, app-exit sequencing) are out of scope for the initial
  implementation.
- **Predefined subagent blueprints.** Ad-hoc spawn lands first. A
  `SubagentBlueprintStore` can come later and simply populates the same
  `spawn_subagent` parameters from storage.
- **Compatibility with `Chat` compaction (`MessageContextState::CompactedAway`).**
  Subagent chats are persisted in full; the parent's view only needs the
  summary. No interaction expected with compaction at first, but worth
  revisiting when compaction goes live.

## Incremental implementation plan

Each milestone leaves the workspace compiling under the invariant matrix
(`--no-default-features`, default, `--all-features`, `tui`, `concierge`,
`tui+concierge`). Each milestone adds tests; no sleep-based synchronisation.

### Milestone 1 — `xoxo-core` contracts

1. Add `ChatPath` (newtype over `Vec<Uuid>`) with accessors
   `root_id`, `parent_id`, `current`, `depth`, and the `child(id: Uuid)`
   push-constructor. No `ConversationId`/`ConversationPath`: chats *are*
   the agents and `Chat.id: Uuid` is the identity.
2. Extend `Chat` with `parent_chat_id: Option<Uuid>` and
   `spawned_by_tool_call_id: Option<ChatToolCallId>`. Extend
   `ToolCallEvent::Started` in `chat/structs.rs` with `kind: ToolCallKind`.
   Update `structs_tests.rs` round-trips.
3. Declare `BusEvent` and its nested enums per §8:
   - `BusEnvelope { path, payload }`
   - `BusPayload::{ Message, ToolCall, AgentShutdown, Error }`
   - `ToolCallEvent::{ Started, Completed, Failed }`
   - `ToolCallStarted { tool_call_id, tool_name, arguments, kind }`
   - `ToolCallKind::{ Generic, SpawnSubagent { child_chat_id, child_path,
     spec_summary } }` (reserve space for `CompactChat` as a later
     addition).

   And the `Command` enum (`SendUserMessage`, `Shutdown`). Every event and
   every command carries a `ChatPath`.
4. Declare `AgentHandle`, `HandleRegistry`, `HandleError` (no runtime yet).
   Unit-test `HandleRegistry` routing helpers (`roots`, `children_of`,
   `subtree`) with a `StubHandle`.
5. Declare `Spawner` trait + `SpawnInput` (ad-hoc only: `inline_spec`,
   `initial_prompt`, `parent_path`). No `BlueprintStore` yet.

**Exit criteria**: types compile, serde round-trip tests pass,
`HandleRegistry` tests pass. No behavioural code.

### Milestone 2 — `AgentRunner` and in-memory spawner

1. Implement `AgentRunner` as a struct owning the inbound `mpsc::Receiver`,
   history (`Chat`), blueprint, tool set, `ToolContext`, broadcast sender.
   Start with a stub `LlmBackend::complete` call — the LLM trait design is
   out of scope here; wire against whatever the LLM-trait ADR produces, and
   if it doesn't exist yet, block on it.
2. Implement `AgentSpawner`:
   - `Arc::new_cyclic` + `Weak<Self>` so `ToolContext` can carry
     `Arc<dyn Spawner>` without cycles.
   - `do_spawn` resolves the spec, allocates the child `Chat.id`, builds
     `ChatPath`, persists the `Chat` shell, creates handle + mpsc, inserts
     into `HandleRegistry`, spawns runner, sends initial prompt.
   - `spawn_and_await` adds a `oneshot::Sender<SubagentHandoff>` plumbed
     into the runner so non-streaming handoff works.
3. Shutdown cascade: when the runner task exits, persist final `Chat`,
   remove from registry, drop the shared per-tree exec context if it was
   the last user.
--- DONE TO THIS POINT ---
4. Integration test: spawn one root, send a user message through its
   handle, assert the runner reaches completion and publishes the expected
   events on the bus.

**Exit criteria**: a root agent can run end-to-end in tests, using a
faked `LlmBackend` that echoes or plays a scripted turn.

### Milestone 3 — `spawn_subagent` and `list_available_tools`

1. Define the two tools in `xoxo-core` with their JSON schemas. Grant them
   together, gated on `ChatPath::depth() < max_depth` at the
   spawn-site (i.e. when building the *child's* toolset, not the parent's).
2. `spawn_subagent::execute`:
   - Read parent path from `ToolContext`; reject if `depth + 1 > max_depth`.
   - Validate `tools` against the current grantable pool.
   - Resolve model via config fallback.
   - Call `spawner.spawn_and_await(SpawnInput::inline(...))`.
   - Serialise `SubagentHandoff` into the tool result.
3. `list_available_tools::execute`: returns the grantable pool as
   `Vec<ToolDescription>`.
4. Publish spawning strictly through `ToolCallEvent::Started { kind:
   SpawnSubagent { .. } }`; do not add parallel
   `SubagentSpawned`/`SubagentCompleted` bus payloads.
5. Integration tests:
   - Two-level tree (root spawns one subagent): handoff returns, parent's
     next turn sees the tool result, both chats persisted, registry returns
     two handles grouped by `root_id`.
   - Depth cap: at `max_depth`, children do not see `spawn_subagent`.
   - Toolset isolation: subagent's toolset is exactly the requested
     allowlist; parent's toolset unchanged.

**Exit criteria**: ad-hoc subagents work; non-streaming handoff verified;
depth policy enforced by construction, not by runtime refusal.

### Milestone 4 — Bus routing and user-input gating

1. Wire the daemon's command ingress: accept `Command::SendUserMessage`,
   look up handle by `chat_id`, check `path().depth() == 0`, forward via
   `handle.send(Command::SendUserMessage(..))`. Reject non-root targets with
   a typed error.
2. Subscribe a logging sink to the broadcast channel in the daemon for
   observability; make sure subagent events are captured with full path.
3. `xoxo-tui` behind `tui` feature: minimal renderer that groups events by
   `root_id` and shows subagent events as nested blocks. TUI sends
   `SendUserMessage` through the command channel like any other client.
4. Integration test: TUI-style client sends a message to a subagent id →
   rejected; sends to a root id → delivered.

**Exit criteria**: "user input to roots only" is enforced end-to-end;
subagent observability visible in TUI output.

### Milestone 5 — Shared execution context and cleanup

1. Move the per-tree `AgentExecutionContext` (bash session + process
   registry) behind the session-backed-tool detection from
   `.not-crates/agent/handle/spawner/wiring.rs`: create on demand when the
   root's resolved toolset includes session-backed tools; pass the same
   `Arc` into every child's `ToolContext`.
2. Implement `shutdown()` cascade from runner exit, honouring "last user
   wins" (don't tear down the context while any agent in the tree still
   runs).
3. Integration test: root + subagent both execute tools that register
   processes; on root shutdown, all registered processes terminate.

**Exit criteria**: a subagent's started processes are reachable from the
tree-level context and terminated on shutdown.

### Milestone 6 — Hardening and docs

1. Error-path tests: LLM failure in subagent surfaces as `HandoffKind::Failed`
   with message propagated to the parent's tool result.
2. `#![deny(warnings)]` still clean; `cargo clippy --workspace
   --all-features -- -D warnings` green.
3. Feature-matrix CI job (or local script) that runs the invariant build
   matrix.
4. Update `AGENTS.md` / `CLAUDE.md` if any rules here need to be lifted into
   repo-wide invariants.

**Exit criteria**: the agent/subagent subsystem is the stable foundation
other crates (`nerd`, `concierge`, future blueprint stores, future remote
transports) can build on without touching runtime internals.

## References

- `.not-crates/agent/` — working reference implementation (NATS-based) from
  a sibling project; informed handle, runner, and spawner shapes.
- `crates/lib/xoxo-core/src/chat/structs.rs` — persisted chat types to be
  extended in Milestone 1.
- `CLAUDE.md` — project invariants (feature matrix, bus-centricity, agent
  separation, Rust conventions).
- `.claude/skills/rust-best-practices/SKILL.md` — coding rules applied
  throughout the implementation.
