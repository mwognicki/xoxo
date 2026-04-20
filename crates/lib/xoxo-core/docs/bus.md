# Bus message reference

The in-process message bus (`crates/lib/xoxo-core/src/bus.rs`) is the one
integration seam between the daemon, the LLM adapters, the agents, and any UI
client. This document catalogs the message types that travel across it.

All payloads are plain `serde`-ready data — no channel handles, closures, or
other non-portable types — so the same enums can move over a future
out-of-process transport without surgery.

## Channels

| Channel          | Direction             | Kind                      | Capacity | Payload       |
| ---------------- | --------------------- | ------------------------- | -------- | ------------- |
| `user_messages`  | clients → daemon      | `tokio::mpsc`             | 64       | `UserMessage` |
| `agent_messages` | daemon → many clients | `tokio::broadcast`        | 256      | `AgentMessage`|
| `logs`           | daemon → many clients | `tokio::broadcast`        | 1024     | `LogRecord`   |

- `mpsc` gives back-pressure: when `user_messages` is full, senders `await`.
- `broadcast` drops the slowest — lagging subscribers receive
  `RecvError::Lagged` and resume from the newest message, so the daemon never
  blocks on a lazy UI.
- Request/reply is intentionally absent. The first command that needs a typed
  response will introduce its own correlation helper.

## Identifiers

Both identifiers are owned `String`s so values stay transport-neutral and
`serde`-friendly.

- **`WorkId(String)`** — stable id for one work item / assistant turn.
- **`ToolCallId(String)`** — stable id for one tool invocation inside a turn.
  A single turn may fan out several concurrent tool calls; this id ties a
  `ToolCallStarted` to its eventual `ToolCallCompleted` / `ToolCallFailed`.

## `UserMessage` — clients → daemon

Work-item conversation messages. Serialized as a tagged union
(`#[serde(tag = "kind", rename_all = "snake_case")]`).

| Variant          | Fields                           | Purpose                                                 |
| ---------------- | -------------------------------- | ------------------------------------------------------- |
| `SubmitWork`     | `work_id: WorkId`, `content: String` | Start or continue work on behalf of the human user. |
| `InterruptWork`  | `work_id: WorkId`                | Ask the daemon to interrupt the currently running item. |

## `AgentMessage` — daemon → clients

Assistant-side conversation output. Same tagged-union encoding. Only
`PartialEq` (not `Eq`), because `ToolCallStarted::arguments` carries a
`serde_json::Value`.

| Variant              | Fields                                                                                   | Purpose                                       |
| -------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------- |
| `WorkStarted`        | `work_id`                                                                                | Daemon accepted the work item.                |
| `TextDelta`          | `work_id`, `delta: String`                                                               | Streaming partial assistant output.           |
| `WorkCompleted`      | `work_id`, `content: String`                                                             | Final assistant response for the work item.   |
| `WorkFailed`         | `work_id`, `message: String`                                                             | Terminal failure for the work item.           |
| `ToolCallStarted`    | `work_id`, `tool_call_id`, `name: String`, `arguments: serde_json::Value`                | A tool call has been dispatched.              |
| `ToolCallCompleted`  | `work_id`, `tool_call_id`, `result_preview: String`                                      | Tool call finished successfully.              |
| `ToolCallFailed`     | `work_id`, `tool_call_id`, `message: String`                                             | Tool call failed terminally.                  |

`ToolCallCompleted::result_preview` is a **bounded, human-readable summary**,
not the full tool output. Broadcast payloads are cloned to every subscriber,
so the authoritative output lives in persistence and is fetched by
`tool_call_id` when a client needs the full result.

## `LogRecord` — daemon → clients

A log line produced somewhere in the daemon and routed through the bus.
Populated by `BusLogLayer` in `log_layer.rs` (behind the `log-broadcast`
feature). Consumers read the same struct whether they live in-process today
or out-of-process tomorrow.

| Field     | Type       | Notes                                                                       |
| --------- | ---------- | --------------------------------------------------------------------------- |
| `level`   | `LogLevel` | `Trace` / `Debug` / `Info` / `Warn` / `Error`. Mirrors `tracing::Level` but owns its representation so the payload stays `serde`-clean. |
| `target`  | `String`   | Tracing target (typically the module path).                                 |
| `message` | `String`   | Rendered log message.                                                       |

## Handles

- **`Bus`** — shared client handle, cheap to clone (each field is a `tokio`
  channel handle, internally an `Arc`). Clients hold this.
  - `send_user_message(UserMessage) -> Result<(), BusError>` — awaits when
    full; errors only when the daemon has dropped its inbox (`DaemonGone`).
  - `publish_agent_message(AgentMessage) -> usize` — daemon-side; returns the
    number of active subscribers. `Ok(0)` is normal.
  - `publish_log(LogRecord) -> usize` — used by `BusLogLayer`.
  - `subscribe_agent_messages()`, `subscribe_logs()` — each subscriber gets
    its own cursor.
  - `logs_sender()` — exposes the broadcast sender so the tracing layer can
    clone it without taking a full `Bus` clone.
- **`CommandInbox`** — daemon-side receiver for `user_messages`, held by
  exactly one consumer. `recv()` returns `None` once every `Bus` handle has
  been dropped; the daemon treats that as implicit shutdown.

## Errors

- **`BusError::DaemonGone`** — the only variant today: the daemon dropped its
  inbox, so the command will never be delivered and retrying is pointless.
