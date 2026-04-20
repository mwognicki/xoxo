⏺ llm/ — LLM client + tool-calling loop

- LlmClient facade — single boundary between framework code and the underlying provider. Stateless w.r.t. model (model passed per call). Exposes from_config(&Config) -> Option<Self>,
  provider() -> &str, and complete(...).
- Provider selection — build_client(cfg) picks the first configured backend in priority order: OpenRouter → Requesty → Anthropic → OpenAI → Mistral. Requesty can speak either         
  Anthropic or OpenAI-compatible protocol based on config. Returns None when no API key is set.
- Provider-type isolation — llm_connector types never appear in the module's public surface.
- Model policy enforcement — validate_model checks ModelPolicy::AllowList / BlockList / ToolDetermined before the first API call. Emits LlmError::PolicyViolation.
- ChatMessage domain type — four variants: User, Assistant, ToolCall { id, name, arguments }, ToolResult { id, content }. The live loop only emits User/Assistant; the tool variants   
  exist for reconstructing history from persistent storage on resume.
- Tool-schema bridge — converts each ToolSchema to llm_connector::types::Tool::function(...) for inclusion in the chat request.
- Dynamic tool-descriptions prompt section — appends a name-sorted "## Available Tools" markdown block (**name** — description) to the system prompt so the LLM sees prose descriptions
  alongside the machine-readable tool list.
- Tool-calling loop (run_complete):
    - Builds initial Vec<LlmMessage> from system prompt + history mapped via to_llm_message.
    - Iterates up to MAX_ITERATIONS = 32. Each iteration sends the full message list + tools to the LLM.
    - If the response has no tool calls, returns the text → loop terminates.
    - If it has tool calls: appends the assistant turn, then per call: parses arguments JSON (malformed → Null), publishes a ToolCall wire event, dispatches via ToolSet::get, publishes
      a ToolResult wire event, appends a tool message binding the same id.
    - Unknown tool → synthesized error result (does not abort). Tool execution error → stringified into a tool result (does not abort).
    - Exceeding the cap returns LlmError::MaxIterationsExceeded.
- Store/NATS decoupling — both conversation-store appends and NATS publishes go through fire-and-forget helpers that only log at WARN, so telemetry/persistence outages never interrupt
  the loop.
- Tool-exchange persistence — each tool call is appended to the ConversationStore as a MessageRole::ToolExchange with {id, name, arguments, output, error}. This is the on-disk format
  the resume path reads back into ChatMessage::ToolCall + ToolResult pairs.
- Error taxonomy — LlmError::{PolicyViolation, Api, MaxIterationsExceeded}.

agent/ — blueprint storage, handles, spawner, runner

Blueprint store
- BlueprintStore trait (dyn-compatible via hand-rolled Pin<Box<Future>>): load, save, list, search. Backend-agnostic — can be Mongo, markdown, in-memory.
- StoreError::{NotFound, Backend}.
- BuiltinBlueprintStore — always-available, in-memory, read-only. Seeded with the built-in root agent (builtin:root, role: Entrypoint Orchestrator) and a large canned system prompt
  that explains Tool/Skill/Memory/Subagent concepts, with a {{tool_names}} placeholder substituted at spawn time.
- FallbackBlueprintStore — layers a primary store over the builtin: load falls back on NotFound only (other errors propagate); list/search merge + dedupe; save is primary-only.
- root_blueprint(model, system_prompt) — defines the root's default tool set (file/http/exec/eval, subagent spawn, memory CRUD, skills CRUD).

Agent handle + registry
- AgentHandle trait — deliberately narrow: id(), conversation_id(), send(AgentMessage), shutdown(). The only sanctioned way to reach a running agent from outside its task.
- HandleError::{AgentGone, Internal}.
- HandleRegistry — Mutex<HashMap<AgentId, Arc<dyn AgentHandle>>>. Operations: insert/remove/get/len/is_empty, plus get_by_conversation_id (O(n) scan) and shutdown_all (parallel
  shutdown() on every handle; errors logged).
- NatsAgentHandle — concrete impl. Delivers messages to the runner over an in-process mpsc::Sender<String> (no NATS round-trip on the hot path). shutdown() publishes an Error { code:
  "shutdown" } envelope on FRAMEWORK_EVENTS. Carries full CallStack ancestry for provenance.

Invariants around subagents
- Subagents are spawned via the spawn_subagent tool, not through handles. Parents receive results as plain tool return values.
- The registry still holds every agent at every nesting level for observability; logs must carry AgentId.

Spawner (AgentSpawner)
- Central spawning authority. Both entry points route here:
    - NATS SpawnRequest (listener side).
    - spawn_subagent tool via Arc<dyn Spawner> in ToolContext.
- Built as Arc::new_cyclic holding a Weak<Self>, so each runner's ToolContext can carry an Arc<dyn Spawner> without an ownership cycle.
- Spawner trait methods: spawn(input) -> ConversationId (fire-and-forget) and spawn_and_await(input) -> (ConversationId, first_response) via a oneshot::Sender<String> threaded into
  the runner.
- do_spawn dispatches to resolve_new vs resolve_resume, then runs the shared do_wire tail.
- resolve_new — inline blueprint used as-is, otherwise loaded from the store (fallback to BUILTIN_ROOT_ID); generates a fresh ConversationId; empty history.
- resolve_resume — requires a ConversationStore; loads a ConversationRecord and rebuilds Vec<ChatMessage>. User/Agent → text messages; each ToolExchange → one ChatMessage::ToolCall +
  one ChatMessage::ToolResult pair (preserving tool-call IDs across multi-tool iterations). Malformed entries are skipped with WARN.
- do_wire — shared tail:                                                                                                                                                               
  a. Resolve ToolSet from blueprint tool names.                                                                                                                                        
  b. Apply runtime system-prompt override (overrides.prompt(blueprint_id)) and substitute {{tool_names}} in the prompt. Skipped for inline: blueprints.                                
  c. Build ConversationPath (root vs parent.child(id)) and CallStack (append new AgentId to parent's).                                                                                 
  d. Execution-context gating — create a bash-backed AgentExecutionContext only if the tool set intersects a hardcoded SESSION_BACKED list (exec, eval_script, http_request, process).
  Creation failure is non-fatal — the agent continues with None and stateful tools error at call time.                                                                                   
  e. Persist a fresh ConversationRecord (skipped for resumes and inline: blueprints).                                                                                                  
  f. Build NatsAgentHandle on an mpsc::channel(CHANNEL_BUFFER = 16), insert into the registry, spawn AgentRunner::run on Tokio.                                                        
  g. Deliver initial_prompt as the first message (skipped if empty — resumes can pass empty prompts).

Agent runner (AgentRunner)
- One Tokio task per agent. Owns the LLM client, blueprint, chat history, tool set, spawner, optional execution context, optional conversation/memory/skill stores, and an optional    
  oneshot::Sender<String> for spawn_and_await.
- Title-generation gating — only for top-level (path.ids().len() == 1), non-inline, empty-history conversations. Triggered by the first non-transient message.
- Per-message loop:                                                                                                                                                                    
  a. Strip TRANSIENT_PREFIX from framework-injected messages (still sent to the LLM, but don't trigger title generation).                                                              
  b. Persist the user message to the store; push ChatMessage::User to history.                                                                                                         
  c. turn() — resolve model from ModelPolicy::AllowList[0] (BlockList/ToolDetermined not yet supported → error); build ToolContext (fresh call_id, agent_id, path, call_stack,         
  publisher, exec ctx, spawner, memory/skill stores); call llm.complete(...).                                                                                                            
  d. On success: persist + push assistant turn, kick off title generation if due, publish Message::AgentDone on conversation_out(path), deliver to response_tx (first response only).  
  e. On failure: publish ErrorPayload { code: "agent_error" }; drop response_tx so spawn_and_await doesn't hang.
- Shutdown (channel closed): close the conversation record with ended_at = now, then execution_context.shutdown() (kills bash + managed processes).
- Title generation — background Tokio task; randomly picks one of two free-tier models by subsecond parity; builds an inline:title-generator blueprint (empty tools); calls            
  spawner.spawn_and_await; persists title via ConversationStore::set_title and publishes Message::ConversationTitleSet on the parent conversation's outbound subject. Uses               
  overrides.prompt("title_generator") for override support. Uses the same Spawner interface as any other agent — title generation is itself an agent, not a special-case code path.

Cross-cutting design choices
- Inline blueprints (id prefix inline:) bypass: system-prompt overrides, {{tool_names}} substitution, conversation-record creation, message persistence, and title generation
  (recursion guard).
- Transient messages (TRANSIENT_PREFIX) reach the LLM but don't count for title generation or as "user-authored" for first-response semantics.
- The spawner exposes a single Spawner trait so subagents, title generation, and any future inline agent all share the same path.