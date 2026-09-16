use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;

pub struct TrigerFeature;

#[async_trait]
impl Feature for TrigerFeature {
    fn name(&self) -> &'static str {
        "triger"
    }

    fn description(&self) -> &'static str {
        "Buat efek \"triggered\" deep-fried"
    }

    fn usage(&self) -> &'static str {
        "triger (kirim gambar/sticker atau reply)"
    }

    fn category(&self) -> Category {
        Category::Fun
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let result: Result<()> = async {
            if media::locate_media(&ctx.msg.message).is_none() {
                ctx.reply("❌ Kirim gambar/sticker atau reply gambar/sticker!")
                    .await?;
                return Ok(());
            }
            let media = media::download_media(&ctx.msg.client, &ctx.msg.message).await?;
            ctx.react("⏳").await?;

            let (webp_bytes, is_animated) =
                tokio::task::spawn_blocking(move || render_triggered_sticker(&media)).await??;

            let upload = ctx
                .msg
                .client
                .upload(webp_bytes, MediaType::Sticker, UploadOptions::default())
                .await
                .context("upload sticker triger gagal")?;

            let msg = media::sticker_message(upload, is_animated);
            let send_res = ctx.send(msg).await;
            let _ = ctx.unreact().await;
            send_res
        }
        .await;
        if result.is_err() {
            let _ = ctx.unreact().await;
            ctx.reply("❌ Gagal membuat triggered effect!").await?;
        }
        Ok(())
    }
}

/// Complex brutal filter chain implementing:
/// 1. Auto-pad and scale to 512x512 transparent sticker canvas
/// 2. Fish-Eye / Bulge distortion (cembung melingkar pada area wajah/mata)
/// 3. Liquify / Warp ripple distortion (asymmetric warp off-center)
/// 4. Violent Screen Shake (getaran hebat per frame dengan osilasi sinusoidal)
/// 5. Deep-Fried / High Saturation (kontras & saturasi ekstrem, exposure tinggi)
/// 6. Toxic Neon Green / Glow (colorchannelmixer dominan hijau neon)
/// 7. Pixelated Crunch / Noise (noise video kasar ala meme deep-fried)
const BRUTAL_TRIGGER_VF: &str = "scale='if(gt(a,1),512,-1)':'if(gt(a,1),-1,512)',pad=512:512:(512-iw)/2:(512-ih)/2:color=0x00000000,lenscorrection=cx=0.45:cy=0.45:k1=-0.45:k2=-0.2,crop=in_w-24:in_h-24:12+12*sin(n*3):12+12*cos(n*4),scale=512:512,eq=contrast=2.6:saturation=3.8:brightness=0.12,colorchannelmixer=rr=1.3:gg=1.6:bb=0.6,noise=alls=15:allf=t+u";

/// Render a deep-fried triggered sticker for any media type (image, static sticker,
/// animated sticker, or video cut to 3 seconds).
/// Pure Rust + magick/ffmpeg, zero Node.js/sharp dependency.
fn render_triggered_sticker(media: &media::Media) -> Result<(Vec<u8>, bool)> {
    let is_anim = is_animated_media(media);
    if is_anim {
        if media.kind == media::MediaKind::Video || media.mimetype.starts_with("video/") {
            render_video_triger(&media.bytes).map(|b| (b, true))
        } else if is_animated_webp(&media.bytes) {
            render_animated_webp_triger(&media.bytes).map(|b| (b, true))
        } else {
            // GIF
            render_gif_triger(&media.bytes).map(|b| (b, true))
        }
    } else {
        // Static image or sticker is rendered as a 2-second violently shaking animated sticker
        render_static_triger(media).map(|b| (b, true))
    }
}

fn is_animated_media(media: &media::Media) -> bool {
    if media.kind == media::MediaKind::Video || media.mimetype.starts_with("video/") {
        return true;
    }
    if media.bytes.starts_with(b"GIF8") {
        return true;
    }
    is_animated_webp(&media.bytes)
}

fn is_animated_webp(bytes: &[u8]) -> bool {
    if bytes.len() < 16 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WEBP" {
        return false;
    }
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let tag = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        if tag == b"ANIM" {
            return true;
        }
        let data_end = pos + 8 + size;
        pos = data_end + (size % 2);
    }
    false
}

/// Extract ANMF frame subchunks from an animated WebP file and wrap each as a valid standalone WebP.
fn extract_webp_frames(
    bytes: &[u8],
    temp_dir: &std::path::Path,
    max_frames: usize,
) -> Vec<std::path::PathBuf> {
    let mut frames = Vec::new();
    if bytes.len() < 16 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WEBP" {
        return frames;
    }
    let mut pos = 12;
    let mut frame_idx = 0;
    while pos + 8 <= bytes.len() && frame_idx < max_frames {
        let tag = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let data_start = pos + 8;
        let data_end = data_start.saturating_add(size);
        if data_end > bytes.len() {
            break;
        }
        if tag == b"ANMF" && size > 16 {
            let sub = &bytes[data_start + 16..data_end];
            let riff_len = (sub.len() as u32 + 4).to_le_bytes();
            let mut frame_data = Vec::with_capacity(12 + sub.len());
            frame_data.extend_from_slice(b"RIFF");
            frame_data.extend_from_slice(&riff_len);
            frame_data.extend_from_slice(b"WEBP");
            frame_data.extend_from_slice(sub);

            frame_idx += 1;
            let frame_path = temp_dir.join(format!("anmf_frame_{:03}.webp", frame_idx));
            if std::fs::write(&frame_path, &frame_data).is_ok() {
                frames.push(frame_path);
            }
        }
        pos = data_end + (size % 2);
    }
    frames
}

/// Trim video to first 3 seconds, apply triggered deep-fried filter, convert to animated WebP sticker.
fn render_video_triger(bytes: &[u8]) -> Result<Vec<u8>> {
    use std::process::Command;

    let temp_in = media::temp_file_path("trig_in", "mp4");
    let temp_out = media::temp_file_path("trig_out", "webp");
    std::fs::write(&temp_in, bytes)?;

    let status = Command::new("ffmpeg")
        .args([
            std::ffi::OsStr::new("-y"),
            std::ffi::OsStr::new("-t"),
            std::ffi::OsStr::new("3"),
            std::ffi::OsStr::new("-i"),
            temp_in.as_os_str(),
            std::ffi::OsStr::new("-vf"),
            std::ffi::OsStr::new(BRUTAL_TRIGGER_VF),
            std::ffi::OsStr::new("-c:v"),
            std::ffi::OsStr::new("libwebp"),
            std::ffi::OsStr::new("-lossless"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-q:v"),
            std::ffi::OsStr::new("55"),
            std::ffi::OsStr::new("-loop"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-an"),
            temp_out.as_os_str(),
        ])
        .status();

    let _ = std::fs::remove_file(&temp_in);

    match status {
        Ok(s) if s.success() => {
            let out = std::fs::read(&temp_out)?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        }
        _ => {
            let _ = std::fs::remove_file(&temp_out);
            anyhow::bail!("render video triggered gagal");
        }
    }
}

/// Process animated WebP: decode via magick if present, or extract ANMF frames + FFmpeg.
fn render_animated_webp_triger(bytes: &[u8]) -> Result<Vec<u8>> {
    use std::process::Command;

    let temp_in = media::temp_file_path("trig_in", "webp");
    let temp_out = media::temp_file_path("trig_out", "webp");
    std::fs::write(&temp_in, bytes)?;

    // 1. Try ImageMagick if available (handles animated WebP on Termux / Linux)
    if let Ok(status) = Command::new("magick")
        .args([
            temp_in.as_os_str(),
            std::ffi::OsStr::new("-coalesce"),
            std::ffi::OsStr::new("-resize"),
            std::ffi::OsStr::new("512x512"),
            std::ffi::OsStr::new("-background"),
            std::ffi::OsStr::new("none"),
            std::ffi::OsStr::new("-gravity"),
            std::ffi::OsStr::new("center"),
            std::ffi::OsStr::new("-extent"),
            std::ffi::OsStr::new("512x512"),
            std::ffi::OsStr::new("-modulate"),
            std::ffi::OsStr::new("100,250"),
            std::ffi::OsStr::new("-swirl"),
            std::ffi::OsStr::new("120"),
            std::ffi::OsStr::new("-quality"),
            std::ffi::OsStr::new("65"),
            temp_out.as_os_str(),
        ])
        .status()
    {
        if status.success() {
            let _ = std::fs::remove_file(&temp_in);
            let out = std::fs::read(&temp_out)?;
            let _ = std::fs::remove_file(&temp_out);
            return Ok(out);
        }
    }
    let _ = std::fs::remove_file(&temp_in);
    let _ = std::fs::remove_file(&temp_out);

    // 2. Fallback: extract ANMF frames in pure Rust, then feed into FFmpeg
    let frames_dir = media::temp_file_path("trig_anmf", "dir");
    std::fs::create_dir_all(&frames_dir)?;

    let frames = extract_webp_frames(bytes, &frames_dir, 35);
    if frames.is_empty() {
        let _ = std::fs::remove_dir_all(&frames_dir);
        anyhow::bail!("tidak ada frame animasi webp yang dapat diekstrak");
    }

    let input_pattern = frames_dir.join("anmf_frame_%03d.webp");
    let temp_out = media::temp_file_path("trig_out", "webp");

    let status = Command::new("ffmpeg")
        .args([
            std::ffi::OsStr::new("-y"),
            std::ffi::OsStr::new("-start_number"),
            std::ffi::OsStr::new("1"),
            std::ffi::OsStr::new("-framerate"),
            std::ffi::OsStr::new("14"),
            std::ffi::OsStr::new("-i"),
            input_pattern.as_os_str(),
            std::ffi::OsStr::new("-t"),
            std::ffi::OsStr::new("3"),
            std::ffi::OsStr::new("-vf"),
            std::ffi::OsStr::new(BRUTAL_TRIGGER_VF),
            std::ffi::OsStr::new("-c:v"),
            std::ffi::OsStr::new("libwebp"),
            std::ffi::OsStr::new("-lossless"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-q:v"),
            std::ffi::OsStr::new("55"),
            std::ffi::OsStr::new("-loop"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-an"),
            temp_out.as_os_str(),
        ])
        .status();

    let _ = std::fs::remove_dir_all(&frames_dir);

    match status {
        Ok(s) if s.success() => {
            let out = std::fs::read(&temp_out)?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        }
        _ => {
            let _ = std::fs::remove_file(&temp_out);
            anyhow::bail!("render animated webp triggered gagal");
        }
    }
}

/// Convert GIF to animated WebP sticker with deep-fried filter.
fn render_gif_triger(bytes: &[u8]) -> Result<Vec<u8>> {
    use std::process::Command;

    let temp_in = media::temp_file_path("trig_in", "gif");
    let temp_out = media::temp_file_path("trig_out", "webp");
    std::fs::write(&temp_in, bytes)?;

    let status = Command::new("ffmpeg")
        .args([
            std::ffi::OsStr::new("-y"),
            std::ffi::OsStr::new("-t"),
            std::ffi::OsStr::new("3"),
            std::ffi::OsStr::new("-i"),
            temp_in.as_os_str(),
            std::ffi::OsStr::new("-vf"),
            std::ffi::OsStr::new(BRUTAL_TRIGGER_VF),
            std::ffi::OsStr::new("-c:v"),
            std::ffi::OsStr::new("libwebp"),
            std::ffi::OsStr::new("-lossless"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-q:v"),
            std::ffi::OsStr::new("55"),
            std::ffi::OsStr::new("-loop"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-an"),
            temp_out.as_os_str(),
        ])
        .status();

    let _ = std::fs::remove_file(&temp_in);

    match status {
        Ok(s) if s.success() => {
            let out = std::fs::read(&temp_out)?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        }
        _ => {
            let _ = std::fs::remove_file(&temp_out);
            anyhow::bail!("render gif triggered gagal");
        }
    }
}

/// Render a static image or sticker as a 2-second violently shaking, bulging, deep-fried animated sticker.
fn render_static_triger(media: &media::Media) -> Result<Vec<u8>> {
    use std::process::Command;

    let ext = if media.bytes.starts_with(b"\x89PNG") {
        "png"
    } else if media.bytes.starts_with(b"\xFF\xD8") {
        "jpg"
    } else if media.bytes.starts_with(b"RIFF") {
        "webp"
    } else {
        "bin"
    };

    let temp_in = media::temp_file_path("trig_in", ext);
    let temp_out = media::temp_file_path("trig_out", "webp");
    std::fs::write(&temp_in, &media.bytes)?;

    // Animate static image for 2s at 14fps with brutal shake, bulge, warp, and deep-fried effect
    let status = Command::new("ffmpeg")
        .args([
            std::ffi::OsStr::new("-y"),
            std::ffi::OsStr::new("-loop"),
            std::ffi::OsStr::new("1"),
            std::ffi::OsStr::new("-t"),
            std::ffi::OsStr::new("2"),
            std::ffi::OsStr::new("-framerate"),
            std::ffi::OsStr::new("14"),
            std::ffi::OsStr::new("-i"),
            temp_in.as_os_str(),
            std::ffi::OsStr::new("-vf"),
            std::ffi::OsStr::new(BRUTAL_TRIGGER_VF),
            std::ffi::OsStr::new("-c:v"),
            std::ffi::OsStr::new("libwebp"),
            std::ffi::OsStr::new("-lossless"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-q:v"),
            std::ffi::OsStr::new("55"),
            std::ffi::OsStr::new("-loop"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-an"),
            temp_out.as_os_str(),
        ])
        .status();

    let _ = std::fs::remove_file(&temp_in);

    match status {
        Ok(s) if s.success() => {
            let out = std::fs::read(&temp_out)?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        }
        _ => {
            let _ = std::fs::remove_file(&temp_out);
            anyhow::bail!("render static triggered effect gagal");
        }
    }
}
