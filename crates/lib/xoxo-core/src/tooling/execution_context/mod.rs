mod bash_session;
mod process_registry;

use std::collections::HashMap;

pub use bash_session::{BashOptions, BashSession, BashSessionError};
pub use process_registry::{*};

use tokio::sync::Mutex;

use crate::config::{load_config, Config};

/// MD5 digest captured for a file at a specific tool interaction point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionFileMd5 {
    /// Absolute or workspace-relative file path used by the tool.
    pub file_path: String,
    /// MD5 digest observed for the file at that point in time.
    pub md5: String,
}

/// Per-file registry entry for file tools involved in a chat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolExecutionFileRecord {
    /// MD5 captured when the file was read.
    pub read: Option<ToolExecutionFileMd5>,
    /// MD5 captured when the file was written.
    pub write: Option<ToolExecutionFileMd5>,
    /// MD5 captured after the file was patched.
    pub patch: Option<ToolExecutionFileMd5>,
}

/// Errors returned by the shared file registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionFileRegistryError {
    FileNotTracked { file_path: String },
    MissingReadEntry { file_path: String },
    Md5Mismatch {
        file_path: String,
        expected_md5: String,
        actual_md5: String,
    },
}

impl std::fmt::Display for ToolExecutionFileRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotTracked { file_path } => {
                write!(f, "File is not tracked: {file_path}")
            }
            Self::MissingReadEntry { file_path } => {
                write!(f, "File has no recorded read entry: {file_path}")
            }
            Self::Md5Mismatch {
                file_path,
                expected_md5,
                actual_md5,
            } => write!(
                f,
                "MD5 mismatch for {file_path}: expected {expected_md5}, got {actual_md5}"
            ),
        }
    }
}

/// Shared registry of files touched by file-oriented tools during a chat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolExecutionFileRegistry {
    files: HashMap<String, ToolExecutionFileRecord>,
}

impl ToolExecutionFileRegistry {
    /// Register a file as involved in the chat with its current MD5.
    pub fn upsert(&mut self, file_path: impl Into<String>, md5: impl Into<String>) {
        let file_path = file_path.into();
        let md5 = md5.into();
        let entry = self.files.entry(file_path.clone()).or_default();

        entry.read = Some(ToolExecutionFileMd5 {
            file_path,
            md5,
        });
    }

    /// Ensure the file was previously read and still matches the recorded MD5.
    pub fn ensure_read(
        &self,
        file_path: &str,
        md5: &str,
    ) -> Result<(), ToolExecutionFileRegistryError> {
        let entry = self
            .files
            .get(file_path)
            .ok_or_else(|| ToolExecutionFileRegistryError::FileNotTracked {
                file_path: file_path.to_string(),
            })?;

        let read = entry.read.as_ref().ok_or_else(|| {
            ToolExecutionFileRegistryError::MissingReadEntry {
                file_path: file_path.to_string(),
            }
        })?;

        if read.md5 != md5 {
            return Err(ToolExecutionFileRegistryError::Md5Mismatch {
                file_path: file_path.to_string(),
                expected_md5: read.md5.clone(),
                actual_md5: md5.to_string(),
            });
        }

        Ok(())
    }

    /// Update a tracked file to a new MD5 after a mutation.
    pub fn update(
        &mut self,
        file_path: &str,
        old_md5: &str,
        new_md5: impl Into<String>,
    ) -> Result<(), ToolExecutionFileRegistryError> {
        self.ensure_read(file_path, old_md5)?;

        let entry = self.files.get_mut(file_path).ok_or_else(|| {
            ToolExecutionFileRegistryError::FileNotTracked {
                file_path: file_path.to_string(),
            }
        })?;

        entry.patch = Some(ToolExecutionFileMd5 {
            file_path: file_path.to_string(),
            md5: new_md5.into(),
        });

        Ok(())
    }
}

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
    /// Shared registry of files read, written, or patched during this chat.
    pub file_registry: Mutex<ToolExecutionFileRegistry>,
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
            file_registry: Mutex::new(ToolExecutionFileRegistry::default()),
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
