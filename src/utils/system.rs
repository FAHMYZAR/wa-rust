#![allow(dead_code)]

use std::sync::OnceLock;
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();

pub fn init_start_time() {
    let _ = START_TIME.set(Instant::now());
}

pub fn get_uptime() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_secs()
}

pub fn format_uptime(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{hours}h {minutes}m {secs}s")
}

pub struct SystemInfo {
    pub platform: &'static str,
    pub arch: &'static str,
    pub engine: String,
    pub uptime: String,
    pub memory_used: String,
    pub memory_total: String,
    pub cpu: String,
}

pub fn get_system_info() -> SystemInfo {
    let platform = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let engine = format!("Rust (wa-rust v{})", env!("CARGO_PKG_VERSION"));
    let uptime = format_uptime(get_uptime());
    let (memory_used, memory_total) = get_memory_info();
    let cpu = get_cpu_info();

    SystemInfo {
        platform,
        arch,
        engine,
        uptime,
        memory_used,
        memory_total,
        cpu,
    }
}

fn get_memory_info() -> (String, String) {
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        let mut total_kb: Option<u64> = None;
        let mut avail_kb: Option<u64> = None;
        let mut free_kb: Option<u64> = None;

        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                total_kb = parse_kb(rest);
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail_kb = parse_kb(rest);
            } else if let Some(rest) = line.strip_prefix("MemFree:") {
                free_kb = parse_kb(rest);
            }
        }

        if let Some(total) = total_kb {
            let avail = avail_kb.or(free_kb).unwrap_or(0);
            let used = total.saturating_sub(avail);
            let used_gb = (used as f64) / (1024.0 * 1024.0);
            let total_gb = (total as f64) / (1024.0 * 1024.0);
            return (format!("{used_gb:.2} GB"), format!("{total_gb:.2} GB"));
        }
    }

    ("N/A".to_string(), "N/A".to_string())
}

fn parse_kb(s: &str) -> Option<u64> {
    s.split_whitespace().next()?.parse().ok()
}

fn get_cpu_info() -> String {
    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim().to_lowercase();
                if key == "model name" || key == "hardware" || key == "processor" {
                    let val = val.trim();
                    if !val.is_empty() {
                        let first = val.split_whitespace().next().unwrap_or(val);
                        return first.to_string();
                    }
                }
            }
        }
    }
    if let Ok(id) = std::env::var("PROCESSOR_IDENTIFIER") {
        if let Some(first) = id.split_whitespace().next() {
            return first.to_string();
        }
    }
    std::env::consts::ARCH.to_string()
}
