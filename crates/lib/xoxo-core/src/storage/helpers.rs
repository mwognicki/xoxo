use std::path::PathBuf;

use crate::chat::structs::Chat;
use crate::config::xoxo_dir;

use super::api::Storage;
use super::types::StorageError;

pub(crate) const CHATS_TREE: &str = "chats";
pub(crate) const METADATA_TREE: &str = "metadata";
pub(crate) const LAST_USED_CHAT_ID_KEY: &[u8] = b"last_used_chat_id";

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

pub(crate) fn parse_chat_snapshot(raw: &str) -> Result<Chat, StorageError> {
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
