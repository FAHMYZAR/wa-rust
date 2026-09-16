use anyhow::{Context as _, Result};
use std::io::Read as _;
use std::time::Duration;
use ureq::unversioned::multipart::{Form, Part};
use whatsapp_rust::download::Downloadable;
use whatsapp_rust::prelude::*;

/// A downloaded media blob plus a hint at how to re-send it.
pub struct Media {
    pub bytes: Vec<u8>,
    pub kind: MediaKind,
    pub caption: Option<String>,
    pub mimetype: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Sticker,
    Document,
}

impl MediaKind {
    pub fn label(self) -> &'static str {
        match self {
            MediaKind::Image => "image",
            MediaKind::Video => "video",
            MediaKind::Audio => "audio",
            MediaKind::Sticker => "sticker",
            MediaKind::Document => "document",
        }
    }
}

/// Pull the quoted message out of the incoming message, if any.
fn quoted_message(msg: &wa::Message) -> Option<&wa::Message> {
    let ci = crate::utils::group::context_info(msg)?;
    ci.quoted_message.as_option()
}

/// Resolve a downloadable media part, searching:
/// 1. the message itself (direct media or view-once wrapper),
/// 2. the quoted message (same shapes).
///
/// Returns the message, a reference to the underlying `Downloadable`, the kind,
/// the caption, and the mimetype.
pub fn locate_media(
    msg: &wa::Message,
) -> Option<(&dyn Downloadable, MediaKind, Option<String>, String)> {
    for candidate in [msg, quoted_message(msg).unwrap_or(msg)] {
        if let Some(found) = media_in(candidate) {
            return Some(found);
        }
    }
    None
}

fn media_in(msg: &wa::Message) -> Option<(&dyn Downloadable, MediaKind, Option<String>, String)> {
    // Plain media.
    if let Some(m) = msg.image_message.as_option() {
        return Some((
            m,
            MediaKind::Image,
            m.caption.clone(),
            mime_or(&m.mimetype, "image/jpeg"),
        ));
    }
    if let Some(m) = msg.video_message.as_option() {
        return Some((
            m,
            MediaKind::Video,
            m.caption.clone(),
            mime_or(&m.mimetype, "video/mp4"),
        ));
    }
    if let Some(m) = msg.sticker_message.as_option() {
        return Some((
            m,
            MediaKind::Sticker,
            None,
            mime_or(&m.mimetype, "image/webp"),
        ));
    }
    if let Some(m) = msg.audio_message.as_option() {
        return Some((
            m,
            MediaKind::Audio,
            None,
            mime_or(&m.mimetype, "audio/mpeg"),
        ));
    }
    if let Some(m) = msg.document_message.as_option() {
        return Some((
            m,
            MediaKind::Document,
            m.caption.clone(),
            mime_or(&m.mimetype, "application/octet-stream"),
        ));
    }
    // View-once wrappers (v1, v2, and the video-message extension).
    for wrapped in [
        msg.view_once_message
            .as_option()
            .and_then(|f| f.message.as_option()),
        msg.view_once_message_v2
            .as_option()
            .and_then(|f| f.message.as_option()),
        msg.view_once_message_v2_extension
            .as_option()
            .and_then(|f| f.message.as_option()),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(found) = media_in(wrapped) {
            return Some(found);
        }
    }
    None
}

fn mime_or(field: &Option<String>, fallback: &str) -> String {
    field
        .as_ref()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

/// Download the media referenced by `msg` (or its quote) into memory.
pub async fn download_media(client: &Client, msg: &wa::Message) -> Result<Media> {
    let (dl, kind, caption, mimetype) =
        locate_media(msg).context("tidak ada media pada pesan ini")?;
    let bytes = client
        .download(dl)
        .await
        .map_err(|e| anyhow::anyhow!("download media gagal: {e}"))?;
    Ok(Media {
        bytes,
        kind,
        caption,
        mimetype,
    })
}

/// Root of the project-local `disk/` folder. Everything the bot writes at
/// runtime (scratch media, the sweeper stamp) lives under here, so the whole
/// area can be cleaned without ever touching the system temp directory.
pub fn project_disk_dir() -> std::path::PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    dir.push("disk");
    dir
}

/// Scratch directory for every temporary artefact the bot creates.
///
/// It is `disk/tmp`, i.e. inside the project itself — never the system temp
/// dir, never Termux's shared tmp. The sweeper only ever touches this
/// subdirectory, so `disk/welcome-banner.jpg` and the sweeper stamp survive.
pub fn temp_dir() -> std::path::PathBuf {
    let dir = project_disk_dir().join("tmp");
    match std::fs::create_dir_all(&dir) {
        Ok(()) => dir,
        Err(_) => std::env::temp_dir(),
    }
}

/// How often `disk/tmp` is emptied.
pub const CLEANUP_INTERVAL_SECS: u64 = 300;

/// Cached copy of the Brat font. It is re-created on demand and therefore
/// exempt from the sweep, so a cleanup can never delete it mid-render.
pub const FONT_CACHE_NAME: &str = "wa_rust_font.ttf";

/// Seconds since the Unix epoch.
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Where the sweeper records the time of its last run. Kept in `disk/` rather
/// than `disk/tmp/` so a sweep can never delete it.
fn cleanup_stamp_path() -> std::path::PathBuf {
    project_disk_dir().join(".cleanup-stamp")
}

/// Age of the last recorded sweep, or `None` when this machine has never run a
/// sweep yet.
fn last_cleanup_age() -> Option<u64> {
    let raw = std::fs::read_to_string(cleanup_stamp_path()).ok()?;
    let stamp: u64 = raw.trim().parse().ok()?;
    Some(now_unix().saturating_sub(stamp))
}

/// Persist the current time so the five-minute cycle can resume after a crash.
fn write_cleanup_stamp() {
    let _ = std::fs::create_dir_all(project_disk_dir());
    let _ = std::fs::write(cleanup_stamp_path(), now_unix().to_string());
}

/// A unique, filesystem-safe token for a single command invocation.
///
/// Build **every** scratch path of one invocation from the same token so two
/// concurrent commands can never collide on a shared filename.
pub fn temp_token(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{}_{}", std::process::id(), nanos)
}

/// Portable temporary file path inside [`temp_dir`].
pub fn temp_file_path(prefix: &str, ext: &str) -> std::path::PathBuf {
    temp_dir().join(format!("{}.{ext}", temp_token(prefix)))
}

/// String form of [`temp_file_path`], for code that builds `Command::args`
/// arrays mixing `&str` literals with the path.
pub fn temp_file_path_str(prefix: &str, ext: &str) -> String {
    temp_file_path(prefix, ext).to_string_lossy().into_owned()
}

/// Delete everything inside [`temp_dir`], except the staged font cache.
pub fn cleanup_temp_dir() {
    let dir = temp_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some(FONT_CACHE_NAME) {
            continue;
        }
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// One sweep: wipe `disk/tmp` and record the time in `disk/.cleanup-stamp`.
fn sweep_temp_dir() {
    cleanup_temp_dir();
    write_cleanup_stamp();
}

/// Background sweeper: empties `disk/tmp` every five minutes for the whole
/// lifetime of the process.
///
/// The schedule is anchored to the timestamp in `disk/.cleanup-stamp`, not to
/// process start, so a restart resumes the cycle where the previous run left
/// it: if the bot swept four minutes ago and then died, the next sweep happens
/// one minute after it comes back rather than five.
pub fn spawn_temp_cleanup_task() {
    tokio::spawn(async {
        let remaining = match last_cleanup_age() {
            Some(age) if age < CLEANUP_INTERVAL_SECS => CLEANUP_INTERVAL_SECS - age,
            _ => 0,
        };
        if remaining > 0 {
            tokio::time::sleep(Duration::from_secs(remaining)).await;
        }
        let mut ticker = tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            sweep_temp_dir();
        }
    });
}

/// Best-effort lookup for an Arial-like face usable by ffmpeg `drawtext`.
/// Checked in order: Windows, Android/Termux, then common Linux paths.
pub fn resolve_font() -> Option<std::path::PathBuf> {
    // Narrow / condensed faces come first: the Brat wordmark is a condensed
    // grotesque, so the closer the face is to Arial Narrow the closer the
    // sticker is to the reference. Plain Arial is only a last resort.
    const CANDIDATES: &[&str] = &[
        "C:/Windows/Fonts/Arial Narrow Bold.ttf",
        "C:/Windows/Fonts/ARIALNB.TTF",
        "C:/Windows/Fonts/Arial Narrow.ttf",
        "C:/Windows/Fonts/ARIALN.TTF",
        "C:/Windows/Fonts/arialbd.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "/system/fonts/RobotoCondensed-Bold.ttf",
        "/system/fonts/RobotoCondensed-Regular.ttf",
        "/system/fonts/Roboto-Bold.ttf",
        "/system/fonts/Roboto-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSansNarrow-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSansNarrow-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    for candidate in CANDIDATES {
        let path = std::path::Path::new(candidate);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }
    None
}

/// Copy the resolved font into `dir` under a plain ASCII name and return that
/// name, so ffmpeg filter strings never need to escape a Windows drive colon.
pub fn stage_font(dir: &std::path::Path) -> Option<String> {
    let src = resolve_font()?;
    let dst = dir.join(FONT_CACHE_NAME);
    // Always re-copy: the cache name is fixed, so keeping an older file would
    // silently pin the bot to whatever face happened to be resolved first.
    std::fs::copy(&src, &dst).ok()?;
    Some(FONT_CACHE_NAME.to_string())
}

/// Returns `Ok(())` when `program` can be spawned, otherwise a message telling
/// the user which external CLI is missing.
pub fn require_program(program: &str) -> Result<()> {
    match std::process::Command::new(program)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => anyhow::bail!(
            "program `{program}` tidak ditemukan di PATH. Install ImageMagick (perintah `magick`) lalu jalankan ulang bot."
        ),
        Err(e) => Err(anyhow::anyhow!("gagal menjalankan `{program}`: {e}")),
    }
}

/// Upload bytes to Catbox and return the public URL. Blocking — call via
/// `spawn_blocking`.
pub fn catbox_upload(bytes: &[u8], filename: &str) -> Result<String> {
    let part = Part::bytes(bytes)
        .file_name(filename)
        .mime_str("application/octet-stream")
        .map_err(|e| anyhow::anyhow!("catbox part: {e}"))?;
    let form = Form::new()
        .text("reqtype", "fileupload")
        .part("fileToUpload", part);

    let config = ureq::config::Config::builder()
        .user_agent("wa-rust-bot/0.1.0")
        .timeout_global(Some(Duration::from_secs(30)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut resp = agent
        .post("https://catbox.moe/user/api.php")
        .send(form)
        .map_err(|e| anyhow::anyhow!("catbox post failed: {e}"))?;
    let mut body = String::new();
    resp.body_mut()
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|e| anyhow::anyhow!("catbox read failed: {e}"))?;

    let url = body.trim();
    if url.starts_with("https://") {
        Ok(url.to_owned())
    } else {
        anyhow::bail!("catbox respons tak terduga: {url}")
    }
}

/// Helper to build a sticker `wa::Message` from an UploadResponse.
pub fn sticker_message(
    upload: whatsapp_rust::upload::UploadResponse,
    is_animated: bool,
) -> wa::Message {
    wa::Message {
        sticker_message: whatsapp_rust::buffa::MessageField::some(wa::message::StickerMessage {
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key.to_vec()),
            file_sha256: Some(upload.file_sha256.to_vec()),
            file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
            file_length: Some(upload.file_length),
            media_key_timestamp: Some(upload.media_key_timestamp),
            mimetype: Some("image/webp".into()),
            is_animated: Some(is_animated),
            ..Default::default()
        }),
        ..Default::default()
    }
}
