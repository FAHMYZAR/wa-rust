use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock};
use whatsapp_rust::Jid;

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();
static CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn db() -> Result<&'static Mutex<Connection>> {
    if let Some(conn) = DB.get() {
        return Ok(conn);
    }
    let conn = Connection::open("warns.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS contacts (
            user_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    let _ = DB.set(Mutex::new(conn));
    Ok(DB.get().expect("db initialized"))
}

fn cache() -> &'static RwLock<HashMap<String, String>> {
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Record a known push_name / display name for a user.
pub fn record_contact(jid: &Jid, name: &str) {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return;
    }

    let user_id = jid.user.as_str();
    if user_id.is_empty() {
        return;
    }

    // Update in-memory cache immediately
    if let Ok(mut map) = cache().write() {
        map.insert(user_id.to_string(), clean_name.to_string());
    }

    // Persist to SQLite
    if let Ok(conn) = db() {
        if let Ok(guard) = conn.lock() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let _ = guard.execute(
                "INSERT INTO contacts (user_id, name, updated_at) VALUES (?, ?, ?)
                 ON CONFLICT(user_id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at",
                params![user_id, clean_name, now],
            );
        }
    }
}

/// Get a known display name for a user ID (phone number or LID).
pub fn get_contact_name(user_id: &str) -> Option<String> {
    if user_id.is_empty() {
        return None;
    }

    // Check in-memory cache
    if let Ok(map) = cache().read() {
        if let Some(name) = map.get(user_id) {
            return Some(name.clone());
        }
    }

    // Check SQLite
    if let Ok(conn) = db() {
        if let Ok(guard) = conn.lock() {
            let mut stmt = guard
                .prepare("SELECT name FROM contacts WHERE user_id = ?")
                .ok()?;
            let name: String = stmt.query_row(params![user_id], |row| row.get(0)).ok()?;
            // Backfill in-memory cache
            if let Ok(mut map) = cache().write() {
                map.insert(user_id.to_string(), name.clone());
            }
            return Some(name);
        }
    }

    None
}
