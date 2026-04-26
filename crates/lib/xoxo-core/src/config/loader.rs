use std::{env, fs};
use std::path::PathBuf;
use toml_example::TomlExample;
use crate::config::structs::Config;

/// Returns the path to the `~/.xoxo/` base directory.
pub fn xoxo_dir() -> PathBuf {
    let home = env::var("HOME").expect("HOME environment variable not set");
    [home.as_str(), ".xoxo"].iter().collect()
}

pub fn resolve_home_path() -> PathBuf {
    let home = env::var("HOME").expect("HOME environment variable not set");
    [home.as_str(), ".xoxo", "config.toml"].iter().collect()
}


fn resolve_config_path() -> PathBuf {
    let home = env::var("HOME").expect("HOME environment variable not set");
    [home.as_str(), ".xoxo", "config.toml"].iter().collect()
}

fn resolve_config_contents() -> String {
    let absolute_path = resolve_config_path();
    match fs::read_to_string(&absolute_path) {
        Ok(contents) => contents,
        Err(_) => {
            fs::create_dir_all(absolute_path.parent().unwrap()).expect("failed to create config directory");
            fs::write(&absolute_path, Config::toml_example()).expect("failed to write boilerplatee config");
            fs::read_to_string(&absolute_path).expect("failed to read boilerplate")
        }
    }
}

pub fn load_config() -> Config {
    let config_string = resolve_config_contents();
    toml::from_str(config_string.as_str()).unwrap()
}

/// Persists the provided config to `~/.xoxo/config.toml`.
///
/// # Errors
///
/// Returns an error when serialization fails or the config file cannot be written.
///
/// # Panics
///
/// Never panics.
pub fn save_config(config: &Config) -> std::io::Result<()> {
    let absolute_path = resolve_config_path();
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = toml::to_string_pretty(config)
        .map_err(std::io::Error::other)?;
    fs::write(absolute_path, serialized)
}
