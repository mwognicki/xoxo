//! In-process message bus.

use crate::chat::structs::{ChatPath, ChatTextMessage, ToolCallEvent};
use crate::llm::LlmFinishReason;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

/// Bounded capacity for the commands channel.
pub const COMMANDS_CAPACITY: usize = 64;

/// Bounded capacity for the events broadcast.
pub const EVENTS_CAPACITY: usize = 256;

/// Bounded capacity for the logs broadcast.
pub const LOGS_CAPACITY: usize = 1024;

pub type UserMessage = ChatTextMessage;
pub type BusEvent = BusEnvelope<BusPayload>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusEnvelope<T> {
    pub path: ChatPath,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BusPayload {
    Message(ChatTextMessage),
    ToolCall(ToolCallEvent),
    Turn(TurnEvent),
    AgentShutdown,
    Error(ErrorPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TurnEvent {
    Started,
    Finished { reason: LlmFinishReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    SubmitUserMessage {
        active_chat_id: Option<Uuid>,
        message: UserMessage,
    },
    SendUserMessage {
        path: ChatPath,
        message: UserMessage,
    },
    Shutdown {
        path: ChatPath,
    },
}

/// Severity carried on a [`LogRecord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// A log line produced somewhere in the daemon and routed through the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

/// Shared handle distributed to clients.
#[derive(Debug, Clone)]
pub struct Bus {
    commands: mpsc::Sender<Command>,
    events: broadcast::Sender<BusEvent>,
    logs: broadcast::Sender<LogRecord>,
}

impl Bus {
    pub fn new() -> (Self, CommandInbox) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMANDS_CAPACITY);
        let (evt_tx, _) = broadcast::channel(EVENTS_CAPACITY);
        let (log_tx, _) = broadcast::channel(LOGS_CAPACITY);

        let bus = Self {
            commands: cmd_tx,
            events: evt_tx,
            logs: log_tx,
        };
        let inbox = CommandInbox { rx: cmd_rx };
        (bus, inbox)
    }

    pub async fn send_command(&self, command: Command) -> Result<(), BusError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| BusError::DaemonGone)
    }

    pub fn publish_event(&self, event: BusEvent) -> usize {
        self.events.send(event).unwrap_or(0)
    }

    pub fn publish_log(&self, record: LogRecord) -> usize {
        self.logs.send(record).unwrap_or(0)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BusEvent> {
        self.events.subscribe()
    }

    pub fn events_sender(&self) -> broadcast::Sender<BusEvent> {
        self.events.clone()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<LogRecord> {
        self.logs.subscribe()
    }

    pub fn logs_sender(&self) -> broadcast::Sender<LogRecord> {
        self.logs.clone()
    }
}

/// Daemon-side receiver for commands.
#[derive(Debug)]
pub struct CommandInbox {
    rx: mpsc::Receiver<Command>,
}

impl CommandInbox {
    pub async fn recv(&mut self) -> Option<Command> {
        self.rx.recv().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("daemon is not accepting commands (inbox dropped)")]
    DaemonGone,
}
