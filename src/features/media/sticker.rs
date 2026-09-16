use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;

pub struct StickerFeature;

#[async_trait]
impl Feature for StickerFeature {
    fn name(&self) -> &'static str {
        "sticker"
    }

    fn description(&self) -> &'static str {
        "Ubah gambar atau video pendek menjadi stiker WhatsApp"
    }

    fn usage(&self) -> &'static str {
        "sticker atau s (kirim gambar/video dengan caption atau reply)"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["s"]
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
                ctx.reply(
                    "❌ Kirim/Reply gambar/video dengan .sticker atau .s\n\nTips: Video akan jadi sticker animasi!",
                )
                .await?;
                return Ok(());
            }
        };

        let is_video =
            media.kind == media::MediaKind::Video || media.mimetype.starts_with("video/");
        let in_bytes = media.bytes;

        let webp_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let temp_in = media::temp_file_path_str("stk_in", "bin");
            let temp_out = media::temp_file_path_str("stk_out", "webp");

            std::fs::write(&temp_in, &in_bytes)?;

            let status = if is_video {
                // Limit to 6 seconds, 15 fps, 512x512 square fit
                Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-t",
                        "6",
                        "-i",
                        &temp_in,
                        "-vf",
                        "scale='if(gt(a,1),512,-1)':'if(gt(a,1),-1,512)',pad=512:512:(512-iw)/2:(512-ih)/2:color=0x00000000,fps=15",
                        "-c:v",
                        "libwebp",
                        "-lossless",
                        "0",
                        "-q:v",
                        "50",
                        "-loop",
                        "0",
                        "-an",
                        &temp_out,
                    ])
                    .status()
            } else if media::require_program("magick").is_ok() {
                Command::new("magick")
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
                        &temp_out,
                    ])
                    .status()
            } else {
                // Fallback: ffmpeg handles PNG/JPEG -> WebP without ImageMagick.
                Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-i",
                        &temp_in,
                        "-vf",
                        "scale='if(gt(a,1),512,-1)':'if(gt(a,1),-1,512)',pad=512:512:(512-iw)/2:(512-ih)/2:color=0x00000000",
                        "-c:v",
                        "libwebp",
                        "-lossless",
                        "0",
                        "-q:v",
                        "80",
                        "-an",
                        &temp_out,
                    ])
                    .status()
            };

            let _ = std::fs::remove_file(&temp_in);

            match status {
                Ok(s) if s.success() => {
                    let out = std::fs::read(&temp_out)?;
                    let _ = std::fs::remove_file(&temp_out);
                    Ok(out)
                }
                _ => {
                    let _ = std::fs::remove_file(&temp_out);
                    anyhow::bail!("konversi media ke webp stiker gagal");
                }
            }
        })
        .await??;

        let upload = ctx
            .msg
            .client
            .upload(webp_bytes, MediaType::Sticker, UploadOptions::default())
            .await
            .context("upload stiker ke WhatsApp gagal")?;

        let msg = media::sticker_message(upload, is_video);
        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}
