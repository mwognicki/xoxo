use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::chat::structs::{Chat, current_chat_timestamp};

use super::helpers::{
    CHATS_TREE, LAST_USED_CHAT_ID_KEY, METADATA_TREE, default_storage_path, parse_chat_snapshot,
};
use super::types::{ChatSessionSummary, StorageError};

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
    /// #     created_at: Some("2026-04-01T00:00:00Z".to_string()),
    /// #     updated_at: Some("2026-04-01T00:00:00Z".to_string()),
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
        let mut snapshot = chat.clone();
        snapshot
            .created_at
            .get_or_insert_with(current_chat_timestamp);
        snapshot.updated_at = Some(current_chat_timestamp());
        let encoded = serde_json::to_vec(&snapshot)?;
        chats.insert(snapshot.id.to_string().as_bytes(), encoded)?;
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

    /// Lists persisted chat sessions ordered from most recently updated to oldest.
    ///
    /// # Errors
    ///
    /// Returns an error when sled reads or chat snapshot deserialization fail.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # let storage = xoxo_core::storage::Storage::open_at(tempfile::tempdir()?.path().join("data"))?;
    /// let sessions = storage.list_chat_sessions()?;
    /// assert!(sessions.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn list_chat_sessions(&self) -> Result<Vec<ChatSessionSummary>, StorageError> {
        let chats = self.db.open_tree(CHATS_TREE)?;
        let mut sessions = chats
            .iter()
            .map(|entry| {
                let (_, encoded) = entry?;
                let raw = String::from_utf8(encoded.to_vec())?;
                let chat = parse_chat_snapshot(&raw)?;
                Ok(ChatSessionSummary {
                    id: chat.id,
                    updated_at: chat.updated_at,
                    model_name: chat.agent.model.model_name,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        sessions.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(sessions)
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
