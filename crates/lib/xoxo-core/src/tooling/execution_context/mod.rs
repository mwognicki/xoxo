mod bash_session;
mod process_registry;

pub use bash_session::{BashOptions, BashSession, BashSessionError};
pub use process_registry::{*};

use tokio::sync::Mutex;

use crate::config::{load_config, Config};

/// Agent-scoped execution environment.
///
/// Only created for agents whose toolset includes shell tools (e.g. `exec`).
/// Agents that use only stateless tools (e.g. `http_request`) never receive
/// one, and no bash process is spawned on their behalf.
///
/// Passed into tool invocations via [`ToolContext::execution_context`] as
/// `Option<Arc<AgentExecutionContext>>`. Stateful tools that require a session
/// return an error if the option is `None` (meaning the agent blueprint did
/// not grant shell access).
///
/// [`ToolContext::execution_context`]: crate::types::ToolContext::execution_context
pub struct ToolExecutionContext {
    /// The agent's persistent bash session. Locked per command invocation.
    pub bash: Mutex<BashSession>,
    pub process_registry: ProcessRegistry,
    pub config: Config,
}

impl ToolExecutionContext {
    /// Spawn a new execution context for an agent.
    ///
    /// Pass [`BashOptions::default()`] for a standard unrestricted session,
    /// or set `restricted: true` for agents that must operate in a confined shell.
    pub async fn new(options: BashOptions) -> Result<Self, BashSessionError> {
        let bash = BashSession::spawn(options).await?;
        Ok(Self {
            bash: Mutex::new(bash),
            process_registry: ProcessRegistry::new(),
            config: load_config(),
        })
    }

    /// Shut down the execution context.
    ///
    /// Kills the bash session and terminates all managed processes.
    /// Must be called when the agent shuts down — including on crash or
    /// timeout — to ensure no child processes are orphaned.
    pub async fn shutdown(&self) {
        self.bash.lock().await.kill().await;
    }
}

#[cfg(test)]
mod tests;
