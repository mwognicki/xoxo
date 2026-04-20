use crate::app_state::AppState;
use crate::config::{load_config, xoxo_dir};
use std::fs;
use std::path::PathBuf;

/// File-backed repository for daemon-owned mutable application state.
pub struct AppStateRepository {
    path: PathBuf,
}

impl AppStateRepository {
    /// Creates a repository rooted at the default `~/.xoxo/app-state.json` path.
    pub fn new() -> Self {
        Self {
            path: xoxo_dir().join("app-state.json"),
        }
    }

    /// Creates a repository backed by an explicit path.
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Loads persisted state, creating the default file if it does not exist yet.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory cannot be created, the file
    /// cannot be read or written, or the JSON is invalid.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn load_or_create(&self) -> Result<AppState, AppStateRepositoryError> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(AppStateRepositoryError::from),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let state = AppState::from_config(&load_config());
                self.save(&state)?;
                Ok(state)
            }
            Err(error) => Err(AppStateRepositoryError::Io(error)),
        }
    }

    /// Persists the given application state.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory cannot be created, the file
    /// cannot be serialized, or the file cannot be written.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn save(&self, state: &AppState) -> Result<(), AppStateRepositoryError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(state)?;
        fs::write(&self.path, json)?;
        Ok(())
    }

    /// Returns the on-disk path used by this repository.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Default for AppStateRepository {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by [`AppStateRepository`].
#[derive(Debug, thiserror::Error)]
pub enum AppStateRepositoryError {
    #[error("app state I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("app state JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_create_writes_default_model_state() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let repository = AppStateRepository::from_path(tempdir.path().join("app-state.json"));

        let state = repository.load_or_create().expect("load_or_create");

        let config = load_config();
        assert_eq!(state, AppState::from_config(&config));
        assert!(repository.path().exists());
    }
}
