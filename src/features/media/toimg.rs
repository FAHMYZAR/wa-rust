use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media::{self as wa_media, ImageOptions, VideoOptions};
use whatsapp_rust::upload::UploadOptions;

pub struct ToimgFeature;

#[async_trait]
impl Feature for ToimgFeature {
    fn name(&self) -> &'static str {
        "toimg"
    }

    fn description(&self) -> &'static str {
        "Konversi sticker menjadi gambar PNG (sticker animasi menjadi GIF video)"
    }

    fn usage(&self) -> &'static str {
        "toimg (reply ke sticker)"
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
                ctx.reply("⚠️ Kirim/Reply sticker atau video dengan .toimg!")
                    .await?;
                return Ok(());
            }
        };

        let in_bytes = media.bytes.clone();
        let is_sticker = media.kind == media::MediaKind::Sticker;
        let is_video =
            media.kind == media::MediaKind::Video || media.mimetype.starts_with("video/");

        if is_video {
            let _ = ctx.react("").await;
            let upload = ctx
                .msg
                .client
                .upload(in_bytes, MediaType::Video, UploadOptions::default())
                .await
                .context("upload video gagal")?;
            let msg = wa_media::video_message(
                upload,
                VideoOptions {
                    caption: Some("🎬 Video as GIF".into()),
                    gif_playback: Some(true),
                    mimetype: Some(media.mimetype),
                    ..Default::default()
                },
            );
            return ctx.send(msg).await;
        }

        if !is_sticker {
            let _ = ctx.react("❌").await;
            ctx.reply("⚠️ Kirim/Reply sticker atau video dengan .toimg!")
                .await?;
            return Ok(());
        }

        let caption = media.caption;
        let png_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let temp_in = media::temp_file_path_str("toimg_in", "webp");
            let temp_out = media::temp_file_path_str("toimg_out", "png");

            std::fs::write(&temp_in, &in_bytes)?;

            // dwebp handles animated webp poorly; try dwebp first, fallback to magick
            let mut last_err = None::<String>;
            for cmd in ["dwebp", "magick"] {
                let status = if cmd == "dwebp" {
                    Command::new("dwebp")
                        .args([&temp_in, "-o", &temp_out])
                        .status()
                } else {
                    Command::new("magick").args([&temp_in, &temp_out]).status()
                };
                match status {
                    Ok(s) if s.success() => {
                        let out = std::fs::read(&temp_out).context("gagal baca hasil png")?;
                        let _ = std::fs::remove_file(&temp_in);
                        let _ = std::fs::remove_file(&temp_out);
                        return Ok(out);
                    }
                    Ok(s) => last_err = Some(format!("{cmd} exit {s}")),
                    Err(e) => last_err = Some(format!("{cmd}: {e}")),
                }
            }

            let _ = std::fs::remove_file(&temp_in);
            let _ = std::fs::remove_file(&temp_out);
            anyhow::bail!(
                "konversi webp ke png gagal: {}",
                last_err.unwrap_or_default()
            )
        })
        .await??;

        let upload = ctx
            .msg
            .client
            .upload(png_bytes, MediaType::Image, UploadOptions::default())
            .await
            .context("upload gambar gagal")?;

        let msg = wa_media::image_message(
            upload,
            ImageOptions {
                caption: caption.or_else(|| Some("🎉 Sticker → Image!".into())),
                mimetype: Some("image/png".into()),
                ..Default::default()
            },
        );
        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}
