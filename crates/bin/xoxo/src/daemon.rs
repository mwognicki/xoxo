use anyhow::Result;
use agentix::skills::discover_available_skills;
use nerd::build_base_prompt;
use std::collections::HashMap;
use std::sync::Arc;
use xoxo_core::agents::{AgentHandle, AgentSpawner};
use xoxo_core::bus::{Bus, BusEnvelope, BusPayload, Command, CommandInbox, ErrorPayload};
use xoxo_core::chat::structs::{
    ApiCompatibility, ApiProvider, ChatAgent, ChatPath, ChatTextMessage, ModelConfig,
};
use xoxo_core::config::{ProviderConfig, load_config};
use xoxo_core::storage::Storage;
use xoxo_core::tooling::ToolRegistry;
use uuid::Uuid;

#[cfg(feature = "tui")]
use tokio::sync::broadcast;

pub async fn run_daemon(
    bus: Bus,
    mut inbox: CommandInbox,
    storage: Arc<Storage>,
) -> Result<()> {
    let spawner = AgentSpawner::new_with_events_and_storage(bus.events_sender(), storage.clone());
    let mut root_handles: HashMap<Uuid, Arc<dyn AgentHandle>> = HashMap::new();

    while let Some(command) = inbox.recv().await {
        let result = match command {
            Command::SubmitUserMessage {
                active_chat_id,
                message,
            } => {
                handle_submit_user_message(
                    &mut root_handles,
                    &spawner,
                    storage.as_ref(),
                    active_chat_id,
                    message,
                )
                .await
            }
            Command::SendUserMessage { path, message } => {
                publish_message(&bus, path, message);
                Ok(())
            }
            Command::Shutdown { path } => {
                bus.publish_event(BusEnvelope {
                    path,
                    payload: BusPayload::AgentShutdown,
                });
                Ok(())
            }
        };

        if let Err(error) = result {
            publish_error(&bus, ChatPath(vec![Uuid::nil()]), error.to_string());
        }
    }

    Ok(())
}

async fn handle_submit_user_message(
    root_handles: &mut HashMap<Uuid, Arc<dyn AgentHandle>>,
    spawner: &Arc<AgentSpawner>,
    storage: &Storage,
    active_chat_id: Option<Uuid>,
    message: ChatTextMessage,
) -> Result<()> {
    let config = load_config();
    let provider_config = resolve_current_provider_config(&config)?;
    let current_model = current_model_from_config(&config);
    let tool_registry = ToolRegistry::new();

    if let Some(chat_id) = active_chat_id {
        if let Some(handle) = root_handles.get(&chat_id) {
            storage.set_last_used_chat_id(chat_id)?;
            return handle
                .send(Command::SendUserMessage {
                    path: handle.path().clone(),
                    message,
                })
                .await
                .map_err(anyhow::Error::from);
        }

        if let Some(handle) = spawner.restore_root(chat_id, provider_config.clone()).await? {
            storage.set_last_used_chat_id(chat_id)?;
            root_handles.insert(chat_id, handle.clone());
            return handle
                .send(Command::SendUserMessage {
                    path: handle.path().clone(),
                    message,
                })
                .await
                .map_err(anyhow::Error::from);
        }
    }

    let chat_id = Uuid::new_v4();
    let tool_names = tool_registry.all_tool_names();
    let allowed_skills = discover_available_skills()
        .into_iter()
        .map(|skill| skill.name)
        .collect();
    let has_available_mcp_servers = !config.mcp_servers().is_empty();
    let system_prompt = build_base_prompt(
        &current_model.model_name,
        &tool_registry.all_schemas(),
        has_available_mcp_servers,
    );
    let blueprint = ChatAgent {
        id: None,
        name: Some("nerd".to_string()),
        model: current_model,
        base_prompt: system_prompt,
        allowed_tools: tool_names,
        allowed_skills,
    };
    let handle = spawner
        .spawn_root(chat_id, blueprint, message, provider_config)
        .await?;
    storage.set_last_used_chat_id(chat_id)?;
    root_handles.insert(chat_id, handle);

    Ok(())
}

fn resolve_current_provider_config(
    config: &xoxo_core::config::Config,
) -> Result<ProviderConfig> {
    let current_provider = config.current_provider();

    if let Some(provider) = config.provider(&current_provider.name) {
        return Ok(provider.clone());
    }

    match current_provider.name.as_str() {
        "openrouter" => Ok(ProviderConfig::built_in(
            "openrouter",
            None,
            std::env::var("OPENROUTER_API_KEY")?,
        )),
        other => Err(anyhow::anyhow!(
            "missing provider config for current provider {other}"
        )),
    }
}

fn current_model_from_config(config: &xoxo_core::config::Config) -> ModelConfig {
    let current_provider = config.current_provider();

    ModelConfig {
        model_name: config.current_model().model_name.clone(),
        provider: ApiProvider {
            name: current_provider.name.clone(),
            compatibility: parse_compatibility(&current_provider.compatibility),
        },
    }
}

fn parse_compatibility(raw: &str) -> ApiCompatibility {
    match raw {
        "open_router" | "openrouter" => ApiCompatibility::OpenRouter,
        "open_ai" | "openai" | "open_ai_like" => ApiCompatibility::OpenAiLike,
        "anthropic" | "anthropic_like" => ApiCompatibility::AnthropicLike,
        _ => ApiCompatibility::OpenAiLike,
    }
}

fn publish_message(bus: &Bus, path: ChatPath, message: ChatTextMessage) {
    bus.publish_event(BusEnvelope {
        path,
        payload: BusPayload::Message(message),
    });
}

fn publish_error(bus: &Bus, path: ChatPath, message: String) {
    bus.publish_event(BusEnvelope {
        path,
        payload: BusPayload::Error(ErrorPayload { message }),
    });
}

pub async fn run_daemon_until_shutdown(
    bus: Bus,
    inbox: CommandInbox,
    storage: Arc<Storage>,
) -> Result<()> {
    let daemon = tokio::spawn(run_daemon(bus, inbox, storage));
    tokio::select! {
        result = daemon => result??,
        _ = tokio::signal::ctrl_c() => {}
    }
    Ok(())
}

#[cfg(any(feature = "tui", feature = "acp"))]
pub fn spawn_daemon(
    bus: Bus,
    inbox: CommandInbox,
    storage: Arc<Storage>,
) -> tokio::task::JoinHandle<Result<()>> {
    tokio::spawn(run_daemon(bus, inbox, storage))
}

#[cfg(feature = "tui")]
pub fn ignore_lagged<T>(result: Result<T, broadcast::error::TryRecvError>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(broadcast::error::TryRecvError::Empty) => None,
        Err(broadcast::error::TryRecvError::Lagged(_)) => None,
        Err(broadcast::error::TryRecvError::Closed) => None,
    }
}
