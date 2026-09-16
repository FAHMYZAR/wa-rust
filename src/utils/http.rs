use anyhow::{anyhow, Result};
use std::io::Read as _;
use std::time::Duration;

const UA: &str = "wa-rust-bot/0.1.0";

fn client(timeout_secs: u64) -> ureq::Agent {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// Minimal JSON GET helper over `ureq` (blocking). Ran inside
/// `tokio::task::spawn_blocking` by callers so the async runtime never stalls.
pub fn get_json(url: &str) -> Result<serde_json::Value> {
    let body = client(15)
        .get(url)
        .header("User-Agent", UA)
        .call()
        .map_err(|e| anyhow!("http get failed: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow!("read body failed: {e}"))?;
    serde_json::from_str(&body).map_err(|e| anyhow!("json parse failed: {e}"))
}

/// Blocking bytes GET (images etc).
pub fn get_bytes(url: &str, timeout_secs: u64) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    client(timeout_secs)
        .get(url)
        .header("User-Agent", UA)
        .call()
        .map_err(|e| anyhow!("http get failed: {e}"))?
        .body_mut()
        .as_reader()
        .read_to_end(&mut buf)
        .map_err(|e| anyhow!("read body failed: {e}"))?;
    Ok(buf)
}

/// Build the message text for `/gempa` from BMKG's `autogempa.json` object.
pub fn format_gempa(value: &serde_json::Value) -> String {
    let get = |k: &str| value.get(k).and_then(|v| v.as_str()).unwrap_or("-");
    format!(
        "🌍 *INFO GEMPA TERBARU - BMKG*\n\n📅 *Waktu:* {}, {}\n📍 *Wilayah:* {}\n\n📊 *Magnitudo:* {} SR\n📏 *Kedalaman:* {}\n🌐 *Koordinat:* {}, {}\n\n📢 {}\n\n_Data dari BMKG_",
        get("Tanggal"),
        get("Jam"),
        get("Wilayah"),
        get("Magnitude"),
        get("Kedalaman"),
        get("Lintang"),
        get("Bujur"),
        get("Potensi"),
    )
}
