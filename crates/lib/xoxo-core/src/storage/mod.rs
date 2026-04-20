use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::chat::structs::Chat;
use crate::config::xoxo_dir;

const CHATS_TREE: &str = "chats";
const METADATA_TREE: &str = "metadata";
const LAST_USED_CHAT_ID_KEY: &[u8] = b"last_used_chat_id";

/// Process-wide sled storage handle rooted under the xoxo home directory.
#[derive(Clone)]
pub struct Storage {
    db: sled::Db,
    path: PathBuf,
}

impl Storage {
    /// Opens the default sled database at `~/.xoxo/data`.
    ///
    /// # Errors
    ///
    /// Returns an error when the storage directory cannot be created or sled
    /// cannot open the database.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let storage = xoxo_core::storage::Storage::open_default()?;
    /// # Ok::<(), xoxo_core::storage::StorageError>(())
    /// ```
    pub fn open_default() -> Result<Self, StorageError> {
        Self::open_at(default_storage_path())
    }

    /// Opens a sled database at the provided path.
    ///
    /// # Errors
    ///
    /// Returns an error when the storage directory cannot be created or sled
    /// cannot open the database.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let tempdir = tempfile::tempdir()?;
    /// let storage = xoxo_core::storage::Storage::open_at(tempdir.path().join("data"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn open_at(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let path = path.into();
        fs::create_dir_all(&path)?;
        let db = sled::open(&path)?;
        Ok(Self { db, path })
    }

    /// Persists the latest snapshot of a chat by chat id.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or sled writes fail.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use uuid::Uuid;
    /// # use xoxo_core::chat::structs::{ApiCompatibility, ApiProvider, BranchId, Chat, ChatAgent, ChatBranch, ModelConfig};
    /// # let storage = xoxo_core::storage::Storage::open_at(tempfile::tempdir()?.path().join("data"))?;
    /// # let chat = Chat {
    /// #     title: Some("Example".to_string()),
    /// #     id: Uuid::nil(),
    /// #     parent_chat_id: None,
    /// #     spawned_by_tool_call_id: None,
    /// #     path: "chats/example.json".to_string(),
    /// #     agent: ChatAgent {
    /// #         id: None,
    /// #         name: Some("nerd".to_string()),
    /// #         model: ModelConfig {
    /// #             model_name: "gpt-5.4".to_string(),
    /// #             provider: ApiProvider {
    /// #                 name: "OpenAI".to_string(),
    /// #                 compatibility: ApiCompatibility::OpenAi,
    /// #             },
    /// #         },
    /// #         base_prompt: "You are helpful.".to_string(),
    /// #         allowed_tools: Vec::new(),
    /// #         allowed_skills: Vec::new(),
    /// #     },
    /// #     observability: None,
    /// #     active_branch_id: BranchId("main".to_string()),
    /// #     branches: vec![ChatBranch {
    /// #         id: BranchId("main".to_string()),
    /// #         name: "Main".to_string(),
    /// #         parent_branch_id: None,
    /// #         forked_from_message_id: None,
    /// #         head_message_id: None,
    /// #         active_snapshot_id: None,
    /// #     }],
    /// #     snapshots: Vec::new(),
    /// #     events: Vec::new(),
    /// # };
    /// storage.save_chat(&chat)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn save_chat(&self, chat: &Chat) -> Result<(), StorageError> {
        let chats = self.db.open_tree(CHATS_TREE)?;
        let encoded = serde_json::to_vec(chat)?;
        chats.insert(chat.id.to_string().as_bytes(), encoded)?;
        self.db.flush()?;
        Ok(())
    }

    /// Loads a persisted chat snapshot by chat id.
    ///
    /// # Errors
    ///
    /// Returns an error when sled reads or deserialization fail.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use uuid::Uuid;
    /// # let storage = xoxo_core::storage::Storage::open_at(tempfile::tempdir()?.path().join("data"))?;
    /// let chat = storage.load_chat(Uuid::nil())?;
    /// assert!(chat.is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_chat(&self, chat_id: Uuid) -> Result<Option<Chat>, StorageError> {
        let chats = self.db.open_tree(CHATS_TREE)?;
        chats
            .get(chat_id.to_string().as_bytes())?
            .map(|encoded| {
                let raw = String::from_utf8(encoded.to_vec())?;
                parse_chat_snapshot(&raw)
            })
            .transpose()
    }

    /// Loads a persisted raw chat snapshot by chat id without deserializing it.
    ///
    /// # Errors
    ///
    /// Returns an error when sled reads fail or the raw bytes are not valid UTF-8.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use uuid::Uuid;
    /// # let storage = xoxo_core::storage::Storage::open_at(tempfile::tempdir()?.path().join("data"))?;
    /// let chat = storage.load_raw_chat(Uuid::nil())?;
    /// assert!(chat.is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_raw_chat(&self, chat_id: Uuid) -> Result<Option<String>, StorageError> {
        let chats = self.db.open_tree(CHATS_TREE)?;
        chats
            .get(chat_id.to_string().as_bytes())?
            .map(|encoded| String::from_utf8(encoded.to_vec()).map_err(StorageError::from))
            .transpose()
    }

    /// Persists the last used root chat id.
    ///
    /// # Errors
    ///
    /// Returns an error when sled writes fail.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use uuid::Uuid;
    /// # let storage = xoxo_core::storage::Storage::open_at(tempfile::tempdir()?.path().join("data"))?;
    /// storage.set_last_used_chat_id(Uuid::nil())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_last_used_chat_id(&self, chat_id: Uuid) -> Result<(), StorageError> {
        let metadata = self.db.open_tree(METADATA_TREE)?;
        metadata.insert(LAST_USED_CHAT_ID_KEY, chat_id.to_string().as_bytes())?;
        self.db.flush()?;
        Ok(())
    }

    /// Loads the last used root chat id, if any.
    ///
    /// # Errors
    ///
    /// Returns an error when sled reads fail or the stored chat id is invalid.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # let storage = xoxo_core::storage::Storage::open_at(tempfile::tempdir()?.path().join("data"))?;
    /// let chat_id = storage.last_used_chat_id()?;
    /// assert!(chat_id.is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn last_used_chat_id(&self) -> Result<Option<Uuid>, StorageError> {
        let metadata = self.db.open_tree(METADATA_TREE)?;
        metadata
            .get(LAST_USED_CHAT_ID_KEY)?
            .map(|value| {
                let text = std::str::from_utf8(&value)?;
                Uuid::parse_str(text).map_err(StorageError::from)
            })
            .transpose()
    }

    /// Deletes the entire sled storage directory for this handle.
    ///
    /// # Errors
    ///
    /// Returns an error when pending sled data cannot be flushed or the on-disk
    /// storage directory cannot be removed.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let tempdir = tempfile::tempdir()?;
    /// let storage = xoxo_core::storage::Storage::open_at(tempdir.path().join("data"))?;
    /// storage.purge()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn purge(self) -> Result<(), StorageError> {
        let path = self.path.clone();
        self.db.flush()?;
        drop(self.db);

        if path.exists() {
            fs::remove_dir_all(path)?;
        }

        Ok(())
    }

    /// Returns the on-disk directory backing this storage handle.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let storage = xoxo_core::storage::Storage::open_default()?;
    /// let path = storage.path();
    /// assert!(path.ends_with("data"));
    /// # Ok::<(), xoxo_core::storage::StorageError>(())
    /// ```
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the underlying sled database handle.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let storage = xoxo_core::storage::Storage::open_default()?;
    /// let _db = storage.db();
    /// # Ok::<(), xoxo_core::storage::StorageError>(())
    /// ```
    pub fn db(&self) -> &sled::Db {
        &self.db
    }
}

/// Opens the default process storage at `~/.xoxo/data`.
///
/// # Errors
///
/// Returns an error when the storage directory cannot be created or sled
/// cannot open the database.
///
/// # Panics
///
/// Never panics.
///
/// # Examples
///
/// ```rust
/// let _storage = xoxo_core::storage::bootstrap_storage()?;
/// # Ok::<(), xoxo_core::storage::StorageError>(())
/// ```
pub fn bootstrap_storage() -> Result<Storage, StorageError> {
    Storage::open_default()
}

/// Returns the default storage directory path under `~/.xoxo`.
///
/// # Errors
///
/// Never returns an error.
///
/// # Panics
///
/// Never panics.
///
/// # Examples
///
/// ```rust
/// let path = xoxo_core::storage::default_storage_path();
/// assert!(path.ends_with("data"));
/// ```
pub fn default_storage_path() -> PathBuf {
    xoxo_dir().join("data")
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

fn parse_chat_snapshot(raw: &str) -> Result<Chat, StorageError> {
    match serde_json::from_str(raw) {
        Ok(chat) => Ok(chat),
        Err(error)
            if error.to_string().contains("duplicate field `kind`")
                && raw.contains("\"status\":\"started\"")
                && raw.contains(",\"kind\":{\"kind\":") =>
        {
            let repaired = repair_duplicate_tool_call_kind(raw);
            serde_json::from_str(&repaired).map_err(StorageError::from)
        }
        Err(error) => Err(StorageError::from(error)),
    }
}

fn repair_duplicate_tool_call_kind(raw: &str) -> String {
    raw.replace(",\"kind\":{\"kind\":", ",\"tool_call_kind\":{\"kind\":")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::chat::structs::{
        ApiCompatibility, ApiProvider, BranchId, ChatAgent, ChatBranch, ChatEvent,
        ChatEventBody, ChatLogEntry, ChatToolCallId, MessageContextState, MessageId,
        ModelConfig, ToolCallEvent, ToolCallKind, ToolCallStarted,
    };

    fn sample_chat(chat_id: Uuid) -> Chat {
        Chat {
            title: Some("Example".to_string()),
            id: chat_id,
            parent_chat_id: None,
            spawned_by_tool_call_id: None,
            path: format!("chats/{chat_id}.json"),
            agent: ChatAgent {
                id: None,
                name: Some("nerd".to_string()),
                model: ModelConfig {
                    model_name: "gpt-5.4".to_string(),
                    provider: ApiProvider {
                        name: "OpenAI".to_string(),
                        compatibility: ApiCompatibility::OpenAi,
                    },
                },
                base_prompt: "You are helpful.".to_string(),
                allowed_tools: Vec::new(),
                allowed_skills: Vec::new(),
            },
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

    fn sample_started_tool_chat(chat_id: Uuid) -> Chat {
        let mut chat = sample_chat(chat_id);
        chat.events.push(ChatLogEntry {
            event: ChatEvent {
                id: MessageId("evt-tool-started-1".to_string()),
                parent_id: None,
                branch_id: BranchId("main".to_string()),
                body: ChatEventBody::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                    tool_call_id: ChatToolCallId("tool-1".to_string()),
                    tool_name: "find_patterns".to_string(),
                    arguments: serde_json::json!({ "pattern": "sled" }),
                    tool_call_kind: ToolCallKind::Generic,
                })),
                observability: None,
            },
            context_state: MessageContextState::Active,
        });
        chat.branches[0].head_message_id = Some(MessageId("evt-tool-started-1".to_string()));
        chat
    }

    #[test]
    fn open_at_creates_storage_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("data");

        let storage = Storage::open_at(&path).expect("storage");

        assert_eq!(storage.path(), path.as_path());
        assert!(path.exists());
    }

    #[test]
    fn save_chat_round_trips_chat_snapshot() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat = sample_chat(Uuid::new_v4());

        storage.save_chat(&chat).expect("save chat");

        let loaded = storage.load_chat(chat.id).expect("load chat");
        assert_eq!(loaded, Some(chat));
    }

    #[test]
    fn last_used_chat_id_round_trips() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat_id = Uuid::new_v4();

        storage
            .set_last_used_chat_id(chat_id)
            .expect("set last used chat id");

        let loaded = storage.last_used_chat_id().expect("load last used chat id");
        assert_eq!(loaded, Some(chat_id));
    }

    #[test]
    fn purge_removes_storage_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("data");
        let storage = Storage::open_at(&path).expect("storage");

        storage.purge().expect("purge storage");

        assert!(!path.exists());
    }

    #[test]
    fn save_chat_uses_tool_call_kind_field_name() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat = sample_started_tool_chat(Uuid::new_v4());

        storage.save_chat(&chat).expect("save chat");

        let raw = storage.load_raw_chat(chat.id).expect("load raw chat");
        let raw = raw.expect("raw chat");
        assert!(raw.contains("\"tool_call_kind\":{\"kind\":\"generic\"}"));
        assert!(!raw.contains(",\"kind\":{\"kind\":\"generic\"}"));
    }

    #[test]
    fn load_chat_repairs_legacy_duplicate_kind_payload() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat = sample_started_tool_chat(Uuid::new_v4());
        let chats = storage.db.open_tree(CHATS_TREE).expect("chats tree");

        let broken_raw = serde_json::to_string(&chat)
            .expect("serialize chat")
            .replace("\"tool_call_kind\":{\"kind\":\"generic\"}", "\"kind\":{\"kind\":\"generic\"}");
        chats
            .insert(chat.id.to_string().as_bytes(), broken_raw.as_bytes())
            .expect("insert broken raw chat");

        let loaded = storage.load_chat(chat.id).expect("load repaired chat");
        assert_eq!(loaded, Some(chat));
    }
}
