Step 1 — Introduce LlmStreamEvent at the facade, keep call sites unchanged

Goal: make the facade streaming-capable without touching any caller.

- Add to crates/lib/xoxo-core/src/llm/facade.rs:                                                                                                                                       
  pub enum LlmStreamEvent {                                                                                                                                                              
  TextDelta(String),                                                                                                                                                                 
  ThinkingDelta(String),
  Final(LlmCompletionResponse),                                                                                                                                                      
  }
- Add a new method complete_streaming(...) -> impl Stream<Item = Result<LlmStreamEvent, LlmCompletionError>>.
- Keep complete(...) as-is; internally reimplement it as a thin wrapper that drives complete_streaming and returns the Final payload.
- Backends still call their blocking endpoints; complete_streaming emits zero deltas and one Final. Pure plumbing.

Ship criteria: all existing tests pass; complete() signature unchanged.


DONE claude --resume 63b6d367-901f-4139-a60c-99971d2af479
                                                                                                                                                                                         
---                                                                                                                                                                                    
Step 2 — Wire real streaming on one backend (rig, openrouter)

Goal: prove the contract with one adapter.

- In complete_with_rig, swap builder.send().await for builder.stream().await.
- While consuming the stream: forward text chunks as TextDelta, forward rig's reasoning chunks (if the model emits them) as ThinkingDelta, accumulate into local buffers.
- At stream end, assemble and yield Final with the same LlmCompletionResponse shape produced today (message, tool calls, finish reason, observability).
- ai-lib path stays blocking for now — it just emits Final. Wiring real
  streaming into ai-lib is explicitly deferred to a later step (not covered
  by this plan) and only worth doing if streaming-capable models are
  actually routed through it. The `complete_streaming` contract
  ("zero or more deltas, then exactly one Final") is fully satisfied by a
  backend that emits zero deltas, so ai-lib remains compliant without
  regression.

Ship criteria: rig/openrouter chats still behave end-to-end; deltas are produced but nobody reads them yet.
                                                                                                                                                                                         
---                                                                                                                                                                                    
Step 3 — Add bus variants, no consumer yet

Goal: make deltas transportable.

- In crates/lib/xoxo-core/src/bus.rs, extend BusPayload:                                                                                                                               
  TextDelta { delta: String },                                                                                                                                                           
  ThinkingDelta { delta: String },
- In the daemon's LLM driver, switch from complete to complete_streaming:
    - LlmStreamEvent::TextDelta(s) → publish BusPayload::TextDelta.
    - LlmStreamEvent::ThinkingDelta(s) → publish BusPayload::ThinkingDelta.
    - LlmStreamEvent::Final(resp) → existing code path (persisted Message + ToolCall events). Unchanged.
- Storage layer untouched — deltas never reach ChatEvent.

Ship criteria: chats still persist identically; bus now carries delta traffic that subscribers can ignore.
                  
---                                                                                                                                                                                    
Step 4 — TUI renders live text deltas

Goal: visible streaming of assistant text.

- In xoxo-tui, subscribe to TextDelta on the active ChatPath.
- Maintain a transient "in-flight assistant message" buffer per chat: append each delta; on the next Message event for the same chat, discard the buffer (canonical message takes      
  over).
- No new widget — reuse the assistant-message renderer with the live buffer as its content.

Ship criteria: user sees text appearing token-by-token; final rendering is identical to today's.

  ---
Step 5 — TUI renders live thinking deltas

Goal: visible streaming of reasoning.

- Separate in-flight thinking buffer, rendered above/before the in-flight text (dimmed, collapsible — minimal styling for v1).
- Cleared when Final arrives, same as text buffer. Not persisted.

Ship criteria: reasoning models stream their thinking visibly; nothing is written to sled.
                                                                                                                                                                                         
---
Step 6 (optional, later) — Persist thinking

Only if the user actually wants a record.

- Add ChatEventBody::Thinking { content: String } (accumulated, not per-delta).
- Daemon appends one such event per turn, alongside the Message event, when the backend produced thinking.
- Storage + replay handle it automatically via the existing snapshot mechanism. 