use anyhow::Result;

/// Bot configuration, loaded once from the environment.
#[derive(Clone)]
pub struct Config {
    /// Prefix for commands from the connected bot account (default `/`).
    pub owner_prefix: String,
    /// Prefix for user commands (default `.`).
    pub user_prefix: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            owner_prefix: std::env::var("OWNER_PREFIX").unwrap_or_else(|_| "/".to_string()),
            user_prefix: std::env::var("USER_PREFIX").unwrap_or_else(|_| ".".to_string()),
        })
    }
}
