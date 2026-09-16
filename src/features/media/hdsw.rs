use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media::{self as wa_media, VideoOptions};
use whatsapp_rust::upload::UploadOptions;

pub struct HdswFeature;

#[async_trait]
impl Feature for HdswFeature {
    fn name(&self) -> &'static str {
        "hdsw"
    }

    fn description(&self) -> &'static str {
        "Cv document video to media player HD (Max file 250Mb)"
    }

    fn usage(&self) -> &'static str {
        "hdsw [caption] (reply ke document video)"
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
                ctx.reply("❌ Reply document video dengan .hdsw [caption] untuk convert ke media player HD!")
                    .await?;
                return Ok(());
            }
        };

        let size_in_mb = media.bytes.len() as f64 / (1024.0 * 1024.0);
        if size_in_mb > 250.0 {
            let _ = ctx.react("❌").await;
            ctx.reply(format!(
                "❌ Video terlalu besar! ({size_in_mb:.2} MB)\nMaksimal 250 MB."
            ))
            .await?;
            return Ok(());
        }

        let custom_caption = ctx.args.trim().to_string();
        let in_bytes = media.bytes;

        let mp4_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let temp_in = media::temp_file_path_str("hd_in", "mp4");
            let temp_out = media::temp_file_path_str("hd_out", "mp4");
            std::fs::write(&temp_in, &in_bytes)?;

            // Re-encode: x264, preset fast, crf 23, aac audio, faststart
            let status = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i",
                    &temp_in,
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-crf",
                    "24",
                    "-vf",
                    "scale='min(1080,iw)':-2,format=yuv420p",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "128k",
                    "-movflags",
                    "+faststart",
                    &temp_out,
                ])
                .status()?;
            let _ = std::fs::remove_file(&temp_in);

            if !status.success() {
                let _ = std::fs::remove_file(&temp_out);
                anyhow::bail!("ffmpeg encode hd gagal");
            }

            let out = std::fs::read(&temp_out).context("baca hd video gagal")?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        })
        .await??;

        let final_size = mp4_bytes.len() as f64 / (1024.0 * 1024.0);

        let upload = ctx
            .msg
            .client
            .upload(mp4_bytes, MediaType::Video, UploadOptions::default())
            .await
            .context("upload video hd gagal")?;

        let caption = if custom_caption.is_empty() {
            format!("✅ Video HD selesai!\n\n📊 {size_in_mb:.2}MB → {final_size:.2}MB")
        } else {
            format!(
                "✅ Video HD selesai!\n\n📊 {size_in_mb:.2}MB → {final_size:.2}MB\n\n{custom_caption}"
            )
        };

        let msg = wa_media::video_message(
            upload,
            VideoOptions {
                caption: Some(caption),
                mimetype: Some("video/mp4".into()),
                ..Default::default()
            },
        );

        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}
