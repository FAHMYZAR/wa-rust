use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use std::sync::{Mutex, OnceLock};

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotMode {
    Public,
    Private,
    Chat,
}

impl BotMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Chat => "chat",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "public" => Some(Self::Public),
            "private" => Some(Self::Private),
            "chat" => Some(Self::Chat),
            _ => None,
        }
    }

    pub const fn allows(self, is_owner: bool, is_group: bool) -> bool {
        match self {
            Self::Public => true,
            Self::Private => is_owner,
            Self::Chat => !is_group,
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Public => {
                "Semua orang dapat menggunakan bot di private chat maupun grup."
            }
            Self::Private => {
                "Hanya owner yang dapat menggunakan bot."
            }
            Self::Chat => {
                "Bot hanya merespons command di private chat dan mengabaikan grup."
            }
        }
    }
}

fn db() -> Result<&'static Mutex<Connection>> {
    if let Some(conn) = DB.get() {
        return Ok(conn);
    }

    let conn = Connection::open("settings.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value)
         VALUES ('bot_mode', 'public')",
        [],
    )?;

    let _ = DB.set(Mutex::new(conn));

    DB.get()
        .ok_or_else(|| anyhow!("failed to initialize settings database"))
}

pub fn get_mode() -> Result<BotMode> {
    let conn = db()?;

    let guard = conn
        .lock()
        .map_err(|e| anyhow!("settings database lock poisoned: {e}"))?;

    let value: String = guard.query_row(
        "SELECT value FROM settings WHERE key = ?",
        params!["bot_mode"],
        |row| row.get(0),
    )?;

    BotMode::parse(&value)
        .ok_or_else(|| anyhow!("invalid bot mode stored in database: {value}"))
}

pub fn set_mode(mode: BotMode) -> Result<()> {
    let conn = db()?;

    let guard = conn
        .lock()
        .map_err(|e| anyhow!("settings database lock poisoned: {e}"))?;

    guard.execute(
        "INSERT INTO settings (key, value)
         VALUES (?, ?)
         ON CONFLICT(key)
         DO UPDATE SET value = excluded.value",
        params!["bot_mode", mode.as_str()],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_allows_everywhere() {
        assert!(BotMode::Public.allows(false, false));
        assert!(BotMode::Public.allows(false, true));
        assert!(BotMode::Public.allows(true, false));
        assert!(BotMode::Public.allows(true, true));
    }

    #[test]
    fn private_only_allows_owner() {
        assert!(BotMode::Private.allows(true, false));
        assert!(BotMode::Private.allows(true, true));

        assert!(!BotMode::Private.allows(false, false));
        assert!(!BotMode::Private.allows(false, true));
    }

    #[test]
    fn chat_only_allows_private_chat() {
        assert!(BotMode::Chat.allows(false, false));
        assert!(BotMode::Chat.allows(true, false));

        assert!(!BotMode::Chat.allows(false, true));
        assert!(!BotMode::Chat.allows(true, true));
    }
}