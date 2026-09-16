use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;

pub struct SmemeFeature;

#[async_trait]
impl Feature for SmemeFeature {
    fn name(&self) -> &'static str {
        "smeme"
    }

    fn description(&self) -> &'static str {
        "Buat meme sticker dari gambar dengan teks atas|bawah"
    }

    fn usage(&self) -> &'static str {
        "smeme teks_atas|teks_bawah (reply gambar)"
    }

    fn category(&self) -> Category {
        Category::Media
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let text = if ctx.args.trim().is_empty() {
            ctx.reply(
                "❌ Masukkan teks!\n\nContoh: .smeme teks atas|teks bawah\nAtau: .smeme teks atas",
            )
            .await?;
            return Ok(());
        } else {
            ctx.args.to_string()
        };

        let _ = ctx.react("⏳").await;

        let downloaded = media::download_media(&ctx.msg.client, &ctx.msg.message).await;
        let media = match downloaded {
            Ok(m) => m,
            Err(_) => {
                ctx.reply(
                    "❌ Reply gambar/sticker dengan teks!\n\nContoh: .smeme teks atas|teks bawah",
                )
                .await?;
                return Ok(());
            }
        };

        let (top, bottom) = if let Some(idx) = text.find('|') {
            (
                text[..idx].trim().to_uppercase(),
                text[idx + 1..].trim().to_uppercase(),
            )
        } else {
            (text.trim().to_uppercase(), String::new())
        };

        let webp_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let temp_in = media::temp_file_path_str("smeme_in", "bin");
            let temp_mid = media::temp_file_path_str("smeme_mid", "png");
            let temp_out = media::temp_file_path_str("smeme_out", "webp");

            std::fs::write(&temp_in, &media.bytes)?;

            if media::require_program("magick").is_err() {
                return render_smeme_ffmpeg(&temp_in, &temp_out, &top, &bottom);
            }

            // Normalize input to a PNG sized 512x512 with transparent padding
            let status = Command::new("magick")
                .args([
                    &temp_in,
                    "-resize",
                    "512x512",
                    "-background",
                    "none",
                    "-gravity",
                    "center",
                    "-extent",
                    "512x512",
                    &temp_mid,
                ])
                .status()?;
            if !status.success() {
                anyhow::bail!("resize gagal");
            }

            // Draw white text with black outline (Impact-style). Font size ~50.
            let mut args: Vec<String> = vec![
                temp_mid.clone(),
                "-font".into(),
                "DejaVu-Sans-Bold".into(),
                "-fill".into(),
                "white".into(),
                "-stroke".into(),
                "black".into(),
                "-strokewidth".into(),
                "3".into(),
                "-pointsize".into(),
                "50".into(),
            ];
            if !top.is_empty() {
                args.extend([
                    "-gravity".into(),
                    "north".into(),
                    "-annotate".into(),
                    "+0+10".into(),
                    top.clone(),
                ]);
            }
            if !bottom.is_empty() {
                args.extend([
                    "-gravity".into(),
                    "south".into(),
                    "-annotate".into(),
                    "+0+10".into(),
                    bottom.clone(),
                ]);
            }
            args.push("-stroke".into());
            args.push("none".into());
            args.push(temp_out.clone());

            let status = Command::new("magick").args(&args).status()?;
            let _ = std::fs::remove_file(&temp_mid);
            if !status.success() {
                anyhow::bail!("draw text gagal");
            }

            let out = std::fs::read(&temp_out).context("gagal baca hasil webp")?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        })
        .await??;

        let upload = ctx
            .msg
            .client
            .upload(webp_bytes, MediaType::Sticker, UploadOptions::default())
            .await
            .context("upload meme stiker gagal")?;
        let msg = media::sticker_message(upload, false);
        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}

/// Fallback meme renderer using ffmpeg `drawtext` on the input image.
/// Used when ImageMagick is not installed (e.g. Windows dev box).
/// Runs with `current_dir` set to the temp dir so paths need no escaping.
fn render_smeme_ffmpeg(input: &str, out_path: &str, top: &str, bottom: &str) -> Result<Vec<u8>> {
    let inp = std::path::PathBuf::from(input);
    let out = std::path::PathBuf::from(out_path);
    let dir = inp
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path input smeme tak valid"))?
        .to_path_buf();
    let in_name = inp
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("nama input smeme tak valid"))?
        .to_string();
    let out_name = out
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("nama output smeme tak valid"))?
        .to_string();

    let mut filter = String::from(
        "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000",
    );
    if !top.is_empty() {
        let name = format!("smeme_top_{}.txt", std::process::id());
        std::fs::write(dir.join(&name), top)?;
        filter.push_str(&format!(
            ",drawtext=textfile={name}:fontcolor=white:fontsize=44:x=(w-text_w)/2:y=16:borderw=3:bordercolor=black"
        ));
    }
    if !bottom.is_empty() {
        let name = format!("smeme_bot_{}.txt", std::process::id());
        std::fs::write(dir.join(&name), bottom)?;
        filter.push_str(&format!(
            ",drawtext=textfile={name}:fontcolor=white:fontsize=44:x=(w-text_w)/2:y=h-text_h-16:borderw=3:bordercolor=black"
        ));
    }

    let status = Command::new("ffmpeg")
        .current_dir(&dir)
        .args([
            "-y",
            "-i",
            &in_name,
            "-vf",
            &filter,
            "-frames:v",
            "1",
            &out_name,
        ])
        .status()?;
    let _ = std::fs::remove_file(dir.join(format!("smeme_top_{}.txt", std::process::id())));
    let _ = std::fs::remove_file(dir.join(format!("smeme_bot_{}.txt", std::process::id())));
    if !status.success() {
        anyhow::bail!("ffmpeg smeme render gagal");
    }
    let out = std::fs::read(&out)?;
    Ok(out)
}
