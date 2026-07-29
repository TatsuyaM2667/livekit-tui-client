use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub livekit_url: String,
    pub api_key: String,
    pub api_secret: String,
    pub last_username: String,
}

impl Default for Config {
    fn default() -> Self {
        // Fallback: try to load from .env
        dotenvy::dotenv().ok();
        Self {
            livekit_url: std::env::var("LIVEKIT_URL")
                .unwrap_or_else(|_| "wss://your-project.livekit.cloud".to_string()),
            api_key: std::env::var("LIVEKIT_API_KEY").unwrap_or_default(),
            api_secret: std::env::var("LIVEKIT_API_SECRET").unwrap_or_default(),
            last_username: String::new(),
        }
    }
}

/// Returns the path to the config file: ~/.config/livekit-tui-client/config.toml
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("livekit-tui-client").join("config.toml"))
}

/// Load config from file. Falls back to .env / defaults if not found.
pub fn load() -> Config {
    if let Some(path) = config_path() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<Config>(&contents) {
                return cfg;
            }
        }
    }
    Config::default()
}

/// Save config to ~/.config/livekit-tui-client/config.toml with 600 permissions.
pub fn save(cfg: &Config) -> Result<()> {
    let path = match config_path() {
        Some(p) => p,
        None => return Ok(()), // Cannot determine config dir; silently skip
    };

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let toml_str = toml::to_string_pretty(cfg)?;
    fs::write(&path, toml_str)?;

    // Set file permissions to 600 (owner read/write only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}
