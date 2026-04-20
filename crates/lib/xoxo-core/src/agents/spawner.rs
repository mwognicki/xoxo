use crate::agents::{AgentHandle, HandleError, HandleRegistry};
use crate::bus::{BusEnvelope, BusEvent, BusPayload, Command};
use crate::chat::structs::{
    ApiCompatibility, ApiProvider, BranchId, Chat, ChatAgent, ChatBranch, ChatEvent,
    ChatEventBody, ChatLogEntry, ChatPath, ChatTextMessage, ChatTextRole, ChatToolCallId,
    MessageContextState, MessageId, ModelConfig, ToolCallCompleted, ToolCallEvent, ToolCallFailed,
    ToolCallKind, ToolCallStarted,
};
use crate::config::ProviderConfig;
use crate::llm::{LlmCompletionRequest, LlmCompletionResponse, LlmFacade, LlmToolCall};
use crate::tooling::{
    BashOptions, ToolContext, ToolError, ToolExecutionContext, ToolRegistry, ToolSet,
};
use futures::future::BoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use uuid::Uuid;

pub type SpawnFuture<'a, T> = BoxFuture<'a, T>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SpawnError {
    #[error("spawner is unavailable")]
    Unavailable,
    #[error("invalid spawn input: {message}")]
    InvalidInput { message: String },
    #[error("failed to initialize root execution context: {message}")]
    ExecutionContextInitialization { message: String },
    #[error("subagent handoff channel closed")]
    HandoffChannelClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffKind {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubagentHandoff {
    pub kind: HandoffKind,
    pub chat: Chat,
    pub summary: Option<String>,
    pub observability: Option<crate::chat::structs::CostObservability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSubagentSpec {
    pub task: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnInput {
    pub inline_spec: InlineSubagentSpec,
    pub initial_prompt: String,
    pub parent_path: ChatPath,
}

impl SpawnInput {
    pub fn inline(
        inline_spec: InlineSubagentSpec,
        initial_prompt: impl Into<String>,
        parent_path: ChatPath,
    ) -> Self {
        Self {
            inline_spec,
            initial_prompt: initial_prompt.into(),
            parent_path,
        }
    }
}

pub trait Spawner: Send + Sync {
    fn spawn(&self, input: SpawnInput) -> SpawnFuture<'_, Result<Arc<dyn AgentHandle>, SpawnError>>;
}

pub struct AgentSpawner {
    self_weak: Weak<Self>,
    registry: Arc<Mutex<HandleRegistry>>,
    persisted_chats: Arc<Mutex<HashMap<Uuid, Chat>>>,
    tree_execution_contexts: Arc<Mutex<HashMap<Uuid, Arc<ToolExecutionContext>>>>,
    tool_registry: Arc<ToolRegistry>,
    events: broadcast::Sender<BusEvent>,
}

impl AgentSpawner {
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(256);

        Self::new_with_events(events)
    }

    pub fn new_with_events(events: broadcast::Sender<BusEvent>) -> Arc<Self> {

        Arc::new_cyclic(|self_weak| Self {
            self_weak: self_weak.clone(),
            registry: Arc::new(Mutex::new(HandleRegistry::new())),
            persisted_chats: Arc::new(Mutex::new(HashMap::new())),
            tree_execution_contexts: Arc::new(Mutex::new(HashMap::new())),
            tool_registry: Arc::new(ToolRegistry::new()),
            events,
        })
    }

    pub fn as_dyn_spawner(&self) -> Arc<dyn Spawner> {
        self.self_weak
            .upgrade()
            .expect("AgentSpawner self weak reference must upgrade")
    }

    pub fn tool_context(&self) -> ToolContext {
        ToolContext {
            execution_context: None,
            spawner: Some(self.as_dyn_spawner()),
        }
    }

    pub async fn ensure_root_execution_context(
        &self,
        root_chat_id: Uuid,
    ) -> Result<Arc<ToolExecutionContext>, SpawnError> {
        if let Some(context) = self
            .tree_execution_contexts
            .lock()
            .await
            .get(&root_chat_id)
            .cloned()
        {
            return Ok(context);
        }

        let context = Arc::new(
            ToolExecutionContext::new(BashOptions::default())
                .await
                .map_err(|error| SpawnError::ExecutionContextInitialization {
                    message: error.to_string(),
                })?,
        );

        let mut contexts = self.tree_execution_contexts.lock().await;
        Ok(contexts
            .entry(root_chat_id)
            .or_insert_with(|| context.clone())
            .clone())
    }

    async fn do_spawn(&self, input: SpawnInput) -> Result<Arc<dyn AgentHandle>, SpawnError> {
        self.do_spawn_with_handoff(input, None).await
    }

    pub async fn spawn_root(
        &self,
        chat_id: Uuid,
        blueprint: ChatAgent,
        initial_message: ChatTextMessage,
        provider_config: ProviderConfig,
    ) -> Result<Arc<dyn AgentHandle>, SpawnError> {
        let path = ChatPath(vec![chat_id]);
        let execution_context = Some(self.ensure_root_execution_context(chat_id).await?);
        let resolved_tools = self.resolve_tools(&blueprint.allowed_tools)?;
        let root_chat = self.build_root_chat_shell(chat_id, &blueprint);
        let (command_tx, command_rx) = mpsc::channel(64);

        self.persisted_chats
            .lock()
            .await
            .insert(chat_id, root_chat.clone());

        let handle: Arc<dyn AgentHandle> =
            Arc::new(LocalAgentHandle::new(chat_id, path.clone(), command_tx.clone()));
        self.registry.lock().await.insert(handle.clone());

        let runner = AgentRunner {
            inbound: command_rx,
            history: root_chat,
            blueprint,
            tool_set: resolved_tools,
            tool_context: ToolContext {
                execution_context: execution_context.clone(),
                spawner: Some(self.as_dyn_spawner()),
            },
            events: self.events.clone(),
            path: path.clone(),
            handoff_tx: None,
            provider_config: Some(provider_config),
        };

        let spawner = self
            .self_weak
            .upgrade()
            .expect("AgentSpawner self weak reference must upgrade");
        tokio::spawn(async move {
            let final_chat = runner.run().await;
            spawner
                .cleanup_after_runner(final_chat, chat_id, path, execution_context)
                .await;
        });

        command_tx
            .send(Command::SendUserMessage {
                path: ChatPath(vec![chat_id]),
                message: initial_message,
            })
            .await
            .map_err(|_| SpawnError::Unavailable)?;

        Ok(handle)
    }

    pub async fn spawn_and_await(
        &self,
        input: SpawnInput,
    ) -> Result<SubagentHandoff, SpawnError> {
        let (handoff_tx, handoff_rx) = oneshot::channel();
        let _handle = self.do_spawn_with_handoff(input, Some(handoff_tx)).await?;
        handoff_rx.await.map_err(|_| SpawnError::HandoffChannelClosed)
    }

    async fn do_spawn_with_handoff(
        &self,
        input: SpawnInput,
        handoff_tx: Option<oneshot::Sender<SubagentHandoff>>,
    ) -> Result<Arc<dyn AgentHandle>, SpawnError> {
        let child_chat_id = Uuid::new_v4();
        let child_path = input.parent_path.child(child_chat_id);
        let child_chat = self.build_chat_shell(&input, child_chat_id);
        let resolved_tools = self.resolve_tools(&input.inline_spec.tools)?;
        let (command_tx, command_rx) = mpsc::channel(64);
        let shared_execution_context = self
            .tree_execution_contexts
            .lock()
            .await
            .get(input.parent_path.root_id())
            .cloned();

        self.persisted_chats
            .lock()
            .await
            .insert(child_chat_id, child_chat.clone());

        let handle: Arc<dyn AgentHandle> = Arc::new(LocalAgentHandle::new(
            child_chat_id,
            child_path.clone(),
            command_tx.clone(),
        ));
        self.registry.lock().await.insert(handle.clone());

        let runner = AgentRunner {
            inbound: command_rx,
            history: child_chat,
            blueprint: self.build_blueprint(&input),
            tool_set: resolved_tools,
            tool_context: ToolContext {
                execution_context: shared_execution_context.clone(),
                spawner: Some(self.as_dyn_spawner()),
            },
            events: self.events.clone(),
            path: child_path.clone(),
            handoff_tx,
            provider_config: None,
        };

        let spawner = self
            .self_weak
            .upgrade()
            .expect("AgentSpawner self weak reference must upgrade");
        let cleanup_path = child_path.clone();
        tokio::spawn(async move {
            let final_chat = runner.run().await;
            spawner
                .cleanup_after_runner(final_chat, child_chat_id, cleanup_path, shared_execution_context)
                .await;
        });

        command_tx
            .send(Command::SendUserMessage {
                path: child_path,
                message: ChatTextMessage {
                    role: ChatTextRole::User,
                    content: input.initial_prompt,
                },
            })
            .await
            .map_err(|_| SpawnError::Unavailable)?;

        Ok(handle)
    }

    fn resolve_tools(&self, names: &[String]) -> Result<ToolSet, SpawnError> {
        self.tool_registry
            .resolve_set(names)
            .map_err(|error| SpawnError::InvalidInput {
                message: error.to_string(),
            })
    }

    fn build_chat_shell(&self, input: &SpawnInput, child_chat_id: Uuid) -> Chat {
        Chat {
            title: Some(input.inline_spec.task.clone()),
            id: child_chat_id,
            parent_chat_id: Some(*input.parent_path.current()),
            spawned_by_tool_call_id: None,
            path: format!("chats/{child_chat_id}.json"),
            agent: self.build_blueprint(input),
            observability: None,
            active_branch_id: BranchId("main".to_string()),
            branches: vec![ChatBranch {
                id: BranchId("main".to_string()),
                name: "Main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: None,
                active_snapshot_id: None,
            }],
            snapshots: Vec::new(),
            events: Vec::new(),
        }
    }

    fn build_blueprint(&self, input: &SpawnInput) -> ChatAgent {
        ChatAgent {
            id: None,
            name: Some("subagent".to_string()),
            model: ModelConfig {
                model_name: input
                    .inline_spec
                    .model
                    .clone()
                    .unwrap_or_else(|| "stub-subagent-model".to_string()),
                provider: ApiProvider {
                    name: "stub-provider".to_string(),
                    compatibility: ApiCompatibility::OpenAiLike,
                },
            },
            base_prompt: input.inline_spec.system_prompt.clone(),
            allowed_tools: input.inline_spec.tools.clone(),
            allowed_skills: Vec::new(),
        }
    }

    fn build_root_chat_shell(&self, chat_id: Uuid, blueprint: &ChatAgent) -> Chat {
        Chat {
            title: Some(
                blueprint
                    .name
                    .clone()
                    .unwrap_or_else(|| "root-agent".to_string()),
            ),
            id: chat_id,
            parent_chat_id: None,
            spawned_by_tool_call_id: None,
            path: format!("chats/{chat_id}.json"),
            agent: blueprint.clone(),
            observability: None,
            active_branch_id: BranchId("main".to_string()),
            branches: vec![ChatBranch {
                id: BranchId("main".to_string()),
                name: "Main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: None,
                active_snapshot_id: None,
            }],
            snapshots: Vec::new(),
            events: Vec::new(),
        }
    }

    async fn cleanup_after_runner(
        &self,
        final_chat: Chat,
        chat_id: Uuid,
        path: ChatPath,
        execution_context: Option<Arc<ToolExecutionContext>>,
    ) {
        self.persisted_chats.lock().await.insert(chat_id, final_chat);

        let subtree_empty = {
            let mut registry = self.registry.lock().await;
            registry.remove(&chat_id);
            registry.subtree(path.root_id()).is_empty()
        };

        if subtree_empty {
            if let Some(context) = execution_context {
                context.shutdown().await;
            }
            self.tree_execution_contexts
                .lock()
                .await
                .remove(path.root_id());
        }
    }
}

impl Spawner for AgentSpawner {
    fn spawn(
        &self,
        input: SpawnInput,
    ) -> SpawnFuture<'_, Result<Arc<dyn AgentHandle>, SpawnError>> {
        async move { self.do_spawn(input).await }.boxed()
    }
}

struct LocalAgentHandle {
    chat_id: Uuid,
    path: ChatPath,
    sender: mpsc::Sender<Command>,
}

impl LocalAgentHandle {
    fn new(chat_id: Uuid, path: ChatPath, sender: mpsc::Sender<Command>) -> Self {
        Self { chat_id, path, sender }
    }
}

impl AgentHandle for LocalAgentHandle {
    fn chat_id(&self) -> &Uuid {
        &self.chat_id
    }

    fn path(&self) -> &ChatPath {
        &self.path
    }

    fn send(&self, cmd: Command) -> SpawnFuture<'_, Result<(), HandleError>> {
        async move {
            match &cmd {
                Command::SendUserMessage { .. } if self.path.depth() > 0 => {
                    return Err(HandleError::NonRootUserMessage);
                }
                Command::SubmitUserMessage { .. } => {}
                _ => {}
            }

            self.sender.send(cmd).await.map_err(|_| HandleError::Closed)
        }
        .boxed()
    }

    fn shutdown(&self) -> SpawnFuture<'_, Result<(), HandleError>> {
        async move {
            self.sender
                .send(Command::Shutdown {
                    path: self.path.clone(),
                })
                .await
                .map_err(|_| HandleError::Closed)
        }
        .boxed()
    }
}

struct AgentRunner {
    inbound: mpsc::Receiver<Command>,
    history: Chat,
    blueprint: ChatAgent,
    tool_set: ToolSet,
    tool_context: ToolContext,
    events: broadcast::Sender<BusEvent>,
    path: ChatPath,
    handoff_tx: Option<oneshot::Sender<SubagentHandoff>>,
    provider_config: Option<ProviderConfig>,
}

impl AgentRunner {
    async fn run(mut self) -> Chat {
        while let Some(command) = self.inbound.recv().await {
            match command {
                Command::SubmitUserMessage { .. } => {}
                Command::SendUserMessage { message, .. } => {
                    self.push_message_event(message.clone());
                    let _ = self.events.send(BusEnvelope {
                        path: self.path.clone(),
                        payload: BusPayload::Message(message.clone()),
                    });

                    let mut next_message;
                    loop {
                        let completion = self.complete().await;
                        self.push_message_event(completion.message.clone());
                        let _ = self.events.send(BusEnvelope {
                            path: self.path.clone(),
                            payload: BusPayload::Message(completion.message.clone()),
                        });

                        if completion.tool_calls.is_empty() {
                            break;
                        }

                        let tool_result = self.dispatch_tool_calls(completion.tool_calls).await;
                        next_message = ChatTextMessage {
                            role: ChatTextRole::User,
                            content: tool_result,
                        };
                        self.push_message_event(next_message.clone());
                    }
                }
                Command::Shutdown { .. } => {
                    let _ = self.events.send(BusEnvelope {
                        path: self.path.clone(),
                        payload: BusPayload::AgentShutdown,
                    });
                    break;
                }
            }
        }

        if let Some(handoff_tx) = self.handoff_tx.take() {
            let summary = self
                .history
                .events
                .iter()
                .rev()
                .find_map(|entry| match &entry.event.body {
                    ChatEventBody::Message(message) if message.role == ChatTextRole::Agent => {
                        Some(message.content.clone())
                    }
                    _ => None,
                });

            let _ = handoff_tx.send(SubagentHandoff {
                kind: HandoffKind::Completed,
                chat: self.history.clone(),
                summary,
                observability: None,
            });
        }

        self.history
    }

    async fn complete(&self) -> LlmCompletionResponse {
        let request = LlmCompletionRequest {
            model: self.blueprint.model.clone(),
            messages: self.completion_messages(),
            tools: self
                .tool_set
                .iter()
                .map(|(name, tool)| crate::llm::LlmToolDefinition {
                    name: name.clone(),
                    description: Some(tool.schema().description.clone()),
                    parameters: tool.schema().parameters.clone(),
                })
                .collect(),
            tool_choice: None,
        };

        if let Some(provider_config) = &self.provider_config {
            return match LlmFacade::new().complete(provider_config, request.clone()).await {
                Ok(response) => response,
                Err(error) => LlmCompletionResponse {
                    message: ChatTextMessage {
                        role: ChatTextRole::Agent,
                        content: error.to_string(),
                    },
                    tool_calls: Vec::new(),
                    observability: None,
                },
            };
        }

        self.stub_complete(request).await
    }

    fn completion_messages(&self) -> Vec<ChatTextMessage> {
        let mut messages = vec![ChatTextMessage {
            role: ChatTextRole::System,
            content: self.blueprint.base_prompt.clone(),
        }];

        messages.extend(self.history.events.iter().filter_map(|entry| {
            match &entry.event.body {
                ChatEventBody::Message(history_message)
                    if history_message.role == ChatTextRole::User
                        || history_message.role == ChatTextRole::Agent =>
                {
                    Some(history_message.clone())
                }
                _ => None,
            }
        }));

        messages
    }

    async fn stub_complete(&self, request: LlmCompletionRequest) -> LlmCompletionResponse {
        let last_user_message = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == ChatTextRole::User)
            .map(|message| message.content.clone())
            .unwrap_or_else(|| "ready".to_string());

        LlmCompletionResponse {
            message: ChatTextMessage {
                role: ChatTextRole::Agent,
                content: format!("stub completion: {last_user_message}"),
            },
            tool_calls: Vec::new(),
            observability: None,
        }
    }

    async fn dispatch_tool_calls(&mut self, calls: Vec<LlmToolCall>) -> String {
        let mut rendered = String::new();
        for call in calls {
            let tool_call_id = ChatToolCallId(Uuid::new_v4().to_string());
            let arguments = call.arguments.clone().unwrap_or(serde_json::Value::Null);

            self.push_tool_call_event(ToolCallEvent::Started(ToolCallStarted {
                tool_call_id: tool_call_id.clone(),
                tool_name: call.name.clone(),
                arguments: arguments.clone(),
                kind: ToolCallKind::Generic,
            }));
            let _ = self.events.send(BusEnvelope {
                path: self.path.clone(),
                payload: BusPayload::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: call.name.clone(),
                    arguments: arguments.clone(),
                    kind: ToolCallKind::Generic,
                })),
            });

            let tool = self.tool_set.get(&call.name);
            let outcome = match tool {
                Some(tool) => tool.execute_erased(&self.tool_context, arguments).await,
                None => Err(ToolError::ExecutionFailed(format!(
                    "unknown tool: {}",
                    call.name
                ))),
            };

            let (chat_event, bus_event, rendered_line) = match outcome {
                Ok(value) => {
                    let preview = tool
                        .map(|tool| tool.map_to_preview(&value))
                        .unwrap_or_else(|| value.to_string());
                    let rendered_value = value.to_string();
                    (
                        ToolCallEvent::Completed(ToolCallCompleted {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result_preview: preview.clone(),
                        }),
                        ToolCallEvent::Completed(ToolCallCompleted {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result_preview: preview.clone(),
                        }),
                        format!("{}: {}", call.name, rendered_value),
                    )
                }
                Err(error) => {
                    let message = match error {
                        ToolError::InvalidInput(m) => format!("invalid input: {m}"),
                        ToolError::ExecutionFailed(m) => m,
                    };
                    (
                        ToolCallEvent::Failed(ToolCallFailed {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            message: message.clone(),
                        }),
                        ToolCallEvent::Failed(ToolCallFailed {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            message: message.clone(),
                        }),
                        format!("{} failed: {}", call.name, message),
                    )
                }
            };

            self.push_tool_call_event(chat_event);
            let _ = self.events.send(BusEnvelope {
                path: self.path.clone(),
                payload: BusPayload::ToolCall(bus_event),
            });

            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(&rendered_line);
        }
        rendered
    }

    fn push_tool_call_event(&mut self, event: ToolCallEvent) {
        let next_id = MessageId(Uuid::new_v4().to_string());
        let parent_id = self.history.events.last().map(|entry| entry.event.id.clone());

        self.history.events.push(ChatLogEntry {
            event: ChatEvent {
                id: next_id.clone(),
                parent_id,
                branch_id: self.history.active_branch_id.clone(),
                body: ChatEventBody::ToolCall(event),
                observability: None,
            },
            context_state: MessageContextState::Active,
        });

        if let Some(branch) = self
            .history
            .branches
            .iter_mut()
            .find(|branch| branch.id == self.history.active_branch_id)
        {
            branch.head_message_id = Some(next_id);
        }
    }

    fn push_message_event(&mut self, message: ChatTextMessage) {
        let next_id = MessageId(Uuid::new_v4().to_string());
        let parent_id = self.history.events.last().map(|entry| entry.event.id.clone());

        self.history.events.push(ChatLogEntry {
            event: ChatEvent {
                id: next_id.clone(),
                parent_id,
                branch_id: self.history.active_branch_id.clone(),
                body: ChatEventBody::Message(message),
                observability: None,
            },
            context_state: MessageContextState::Active,
        });

        if let Some(branch) = self
            .history
            .branches
            .iter_mut()
            .find(|branch| branch.id == self.history.active_branch_id)
        {
            branch.head_message_id = Some(next_id);
        }
    }
}
