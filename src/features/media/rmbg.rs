use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media::{self as wa_media, ImageOptions};
use whatsapp_rust::upload::UploadOptions;

pub struct RmbgFeature;

#[async_trait]
impl Feature for RmbgFeature {
    fn name(&self) -> &'static str {
        "rmbg"
    }

    fn description(&self) -> &'static str {
        "Hapus background gambar (flood-fill chroma lokal)"
    }

    fn usage(&self) -> &'static str {
        "rmbg (reply ke gambar)"
    }

    fn category(&self) -> Category {
        Category::Media
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let _ = ctx.react("⏳").await;

        let downloaded = media::download_media(&ctx.msg.client, &ctx.msg.message).await;
        let media = match downloaded {
            Ok(m) => m,
            Err(_) => {
                ctx.reply("❌ Reply atau kirim gambar dengan caption `.rmbg`")
                    .await?;
                return Ok(());
            }
        };

        let is_image = media.kind == media::MediaKind::Image
            || media.kind == media::MediaKind::Sticker
            || media.mimetype.starts_with("image/");
        if !is_image {
            let _ = ctx.react("❌").await;
            ctx.reply("❌ Reply atau kirim gambar dengan caption `.rmbg`")
                .await?;
            return Ok(());
        }

        if media.bytes.len() > 10 * 1024 * 1024 {
            let _ = ctx.react("❌").await;
            ctx.reply("❌ Gambar terlalu besar! Maksimal 10MB").await?;
            return Ok(());
        }

        let png_bytes = tokio::task::spawn_blocking(move || remove_bg(&media.bytes)).await??;

        let upload = ctx
            .msg
            .client
            .upload(png_bytes, MediaType::Image, UploadOptions::default())
            .await
            .context("upload hasil rmbg gagal")?;

        let msg = wa_media::image_message(
            upload,
            ImageOptions {
                caption: Some("✅ *Background berhasil dihapus!*".into()),
                mimetype: Some("image/png".into()),
                ..Default::default()
            },
        );

        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}

/// Local background removal: flood-fill the four corner colours to transparent.
/// Works well on solid / near-solid backgrounds. `Fuzz` env controls tolerance.
fn remove_bg(input: &[u8]) -> Result<Vec<u8>> {
    media::require_program("magick")?;
    let fuzz = std::env::var("RMBG_FUZZ").unwrap_or_else(|_| "25%".into());
    let temp_in = media::temp_file_path_str("rmbg_in", "bin");
    let temp_out = media::temp_file_path_str("rmbg_out", "png");
    std::fs::write(&temp_in, input)?;

    // Sample the corner pixel colour, then flood-fill it to transparent from each corner.
    let corner = Command::new("magick")
        .args([&temp_in, "-format", "%[pixel:p{0,0}]", "info:"])
        .output()?;
    let corner_color = String::from_utf8_lossy(&corner.stdout).trim().to_string();
    if corner_color.is_empty() {
        anyhow::bail!("gagal membaca warna pojok gambar");
    }

    let mut cmd = Command::new("magick");
    cmd.args([&temp_in, "-fuzz", &fuzz, "-fill", "none"]);
    for geo in ["0,0", "-0+0", "0,-0"] {
        cmd.args(["-draw", &format!("color {geo} floodfill")]);
    }
    // Top-right corner too
    cmd.args(["-draw", "color -0,0 floodfill"]);
    cmd.arg(&temp_out);

    let status = cmd.status()?;
    let _ = std::fs::remove_file(&temp_in);

    if !status.success() {
        let _ = std::fs::remove_file(&temp_out);
        anyhow::bail!("magick removebg gagal");
    }

    let out = std::fs::read(&temp_out)?;
    let _ = std::fs::remove_file(&temp_out);
    Ok(out)
}
