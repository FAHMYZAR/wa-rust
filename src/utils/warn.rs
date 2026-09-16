use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::{Mutex, OnceLock};

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

pub const MAX_WARNS: u32 = 3;

#[derive(Debug, Clone)]
pub struct WarnEntry {
    pub user_id: String,
    pub total_warns: u32,
    pub last_reason: Option<String>,
}

fn db() -> Result<&'static Mutex<Connection>> {
    if let Some(conn) = DB.get() {
        return Ok(conn);
    }
    let conn = Connection::open("warns.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS warns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            reason TEXT NOT NULL,
            admin_id TEXT NOT NULL,
            timestamp INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_group_user ON warns(group_id, user_id)",
        [],
    )?;
    let _ = DB.set(Mutex::new(conn));
    Ok(DB.get().expect("db initialized"))
}

pub fn add_warn(group_id: &str, user_id: &str, reason: &str, admin_id: &str) -> Result<u32> {
    let conn = db()?;
    let guard = conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    guard.execute(
        "INSERT INTO warns (group_id, user_id, reason, admin_id, timestamp) VALUES (?, ?, ?, ?, ?)",
        params![group_id, user_id, reason, admin_id, now],
    )?;
    drop(guard);
    get_warn_count(group_id, user_id)
}

pub fn get_warn_count(group_id: &str, user_id: &str) -> Result<u32> {
    let conn = db()?;
    let guard = conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut stmt =
        guard.prepare("SELECT COUNT(*) FROM warns WHERE group_id = ? AND user_id = ?")?;
    let count: u32 = stmt.query_row(params![group_id, user_id], |row| row.get(0))?;
    Ok(count)
}

pub fn remove_warn(group_id: &str, user_id: &str) -> Result<bool> {
    let conn = db()?;
    let guard = conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let row_id: Option<i64> = guard
        .prepare("SELECT id FROM warns WHERE group_id = ? AND user_id = ? ORDER BY timestamp DESC LIMIT 1")?
        .query_row(params![group_id, user_id], |r| r.get(0))
        .ok();
    if let Some(id) = row_id {
        let changed = guard.execute("DELETE FROM warns WHERE id = ?", params![id])?;
        Ok(changed > 0)
    } else {
        Ok(false)
    }
}

pub fn reset_warns(group_id: &str, user_id: &str) -> Result<()> {
    let conn = db()?;
    let guard = conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    guard.execute(
        "DELETE FROM warns WHERE group_id = ? AND user_id = ?",
        params![group_id, user_id],
    )?;
    Ok(())
}

pub fn get_all_warns_in_group(group_id: &str) -> Result<Vec<WarnEntry>> {
    let conn = db()?;
    let guard = conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut stmt = guard.prepare(
        "SELECT user_id, COUNT(*) as total,
                (SELECT reason FROM warns w2 WHERE w2.group_id = w1.group_id AND w2.user_id = w1.user_id ORDER BY timestamp DESC LIMIT 1) as last_reason
         FROM warns w1
         WHERE group_id = ?
         GROUP BY user_id
         ORDER BY total DESC",
    )?;
    let rows = stmt.query_map(params![group_id], |row| {
        Ok(WarnEntry {
            user_id: row.get(0)?,
            total_warns: row.get(1)?,
            last_reason: row.get(2)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
