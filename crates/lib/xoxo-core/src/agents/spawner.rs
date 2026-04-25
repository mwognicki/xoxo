use crate::agents::{AgentHandle, HandleRegistry};
use crate::bus::{BusEvent, Command};
use crate::chat::structs::{
    ApiCompatibility, ApiProvider, BranchId, Chat, ChatAgent, ChatBranch, ChatPath,
    ChatTextMessage, ChatTextRole, ModelConfig, current_chat_timestamp,
};
use crate::config::ProviderConfig;
use crate::storage::{Storage, bootstrap_storage};
use crate::tooling::{
    BashOptions, ToolContext, ToolExecutionContext, ToolRegistry, ToolSet,
};
use futures::future::BoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use uuid::Uuid;

use super::handles::LocalAgentHandle;
use super::structs::{AgentRunner, AgentSpawner, InlineSubagentSpec, SpawnInput, SubagentHandoff};

pub type SpawnFuture<'a, T> = BoxFuture<'a, T>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SpawnError {
    #[error("spawner is unavailable")]
    Unavailable,
    #[error("invalid spawn input: {message}")]
    InvalidInput { message: String },
    #[error("failed to initialize root execution context: {message}")]
    ExecutionContextInitialization { message: String },
    #[error("storage operation failed: {message}")]
    Storage { message: String },
    #[error("subagent handoff channel closed")]
    HandoffChannelClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffKind {
    Completed,
    Failed,
    Cancelled,
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

impl AgentSpawner {
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(256);

        Self::new_with_events(events)
    }

    pub fn new_with_events(events: broadcast::Sender<BusEvent>) -> Arc<Self> {
        let storage = bootstrap_storage()
            .expect("default xoxo storage must be available before creating AgentSpawner");
        Self::new_with_events_and_storage(events, Arc::new(storage))
    }

    pub fn new_with_events_and_storage(
        events: broadcast::Sender<BusEvent>,
        storage: Arc<Storage>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|self_weak| Self {
            self_weak: self_weak.clone(),
            registry: Arc::new(Mutex::new(HandleRegistry::new())),
            persisted_chats: Arc::new(Mutex::new(HashMap::new())),
            storage,
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
            available_tools: None,
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
        let available_tools = Arc::new(resolved_tools.clone());
        let root_chat = self.build_root_chat_shell(chat_id, &blueprint);
        let (command_tx, command_rx) = mpsc::channel(64);

        self.persist_chat_snapshot(&root_chat).await;
        let _ = self.storage.set_last_used_chat_id(chat_id);

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
                available_tools: Some(available_tools),
                spawner: Some(self.as_dyn_spawner()),
            },
            events: self.events.clone(),
            path: path.clone(),
            handoff_tx: None,
            provider_config: Some(provider_config),
            storage: self.storage.clone(),
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

    pub async fn restore_root(
        &self,
        chat_id: Uuid,
        provider_config: ProviderConfig,
    ) -> Result<Option<Arc<dyn AgentHandle>>, SpawnError> {
        let Some(chat) = self
            .storage
            .load_chat(chat_id)
            .map_err(|error| SpawnError::Storage {
                message: error.to_string(),
            })?
        else {
            return Ok(None);
        };

        if chat.parent_chat_id.is_some() {
            return Ok(None);
        }

        let path = ChatPath(vec![chat_id]);
        let execution_context = Some(self.ensure_root_execution_context(chat_id).await?);
        let resolved_tools = self.resolve_tools(&chat.agent.allowed_tools)?;
        let available_tools = Arc::new(resolved_tools.clone());
        let (command_tx, command_rx) = mpsc::channel(64);

        self.persist_chat_snapshot(&chat).await;
        let _ = self.storage.set_last_used_chat_id(chat_id);

        let handle: Arc<dyn AgentHandle> =
            Arc::new(LocalAgentHandle::new(chat_id, path.clone(), command_tx));
        self.registry.lock().await.insert(handle.clone());

        let runner = AgentRunner {
            inbound: command_rx,
            history: chat.clone(),
            blueprint: chat.agent.clone(),
            tool_set: resolved_tools,
            tool_context: ToolContext {
                execution_context: execution_context.clone(),
                available_tools: Some(available_tools),
                spawner: Some(self.as_dyn_spawner()),
            },
            events: self.events.clone(),
            path: path.clone(),
            handoff_tx: None,
            provider_config: Some(provider_config),
            storage: self.storage.clone(),
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

        Ok(Some(handle))
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
        let available_tools = Arc::new(resolved_tools.clone());
        let (command_tx, command_rx) = mpsc::channel(64);
        let shared_execution_context = self
            .tree_execution_contexts
            .lock()
            .await
            .get(input.parent_path.root_id())
            .cloned();

        self.persist_chat_snapshot(&child_chat).await;

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
                available_tools: Some(available_tools),
                spawner: Some(self.as_dyn_spawner()),
            },
            events: self.events.clone(),
            path: child_path.clone(),
            handoff_tx,
            provider_config: None,
            storage: self.storage.clone(),
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
        let timestamp = current_chat_timestamp();
        Chat {
            title: Some(input.inline_spec.task.clone()),
            id: child_chat_id,
            created_at: Some(timestamp.clone()),
            updated_at: Some(timestamp),
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
        let timestamp = current_chat_timestamp();
        Chat {
            title: Some(
                blueprint
                    .name
                    .clone()
                    .unwrap_or_else(|| "root-agent".to_string()),
            ),
            id: chat_id,
            created_at: Some(timestamp.clone()),
            updated_at: Some(timestamp),
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
        self.persisted_chats
            .lock()
            .await
            .insert(chat_id, final_chat.clone());
        let _ = self.storage.save_chat(&final_chat);

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

    async fn persist_chat_snapshot(&self, chat: &Chat) {
        self.persisted_chats.lock().await.insert(chat.id, chat.clone());
        let _ = self.storage.save_chat(chat);
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
