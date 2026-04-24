use uuid::Uuid;

/// Lightweight metadata for a persisted chat session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSessionSummary {
    /// Persisted chat id.
    pub id: Uuid,
    /// Last persisted update timestamp for the chat.
    pub updated_at: Option<String>,
    /// Model name configured for the chat's agent.
    pub model_name: String,
}

/// Errors returned while opening xoxo storage.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("storage I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage database open failed: {0}")]
    Sled(#[from] sled::Error),
    #[error("storage JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage UTF-8 decode failed: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("storage string decode failed: {0}")]
    StringUtf8(#[from] std::string::FromUtf8Error),
    #[error("storage UUID decode failed: {0}")]
    Uuid(#[from] uuid::Error),
}
