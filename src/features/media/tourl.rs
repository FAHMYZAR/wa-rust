use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::Result;
use async_trait::async_trait;

pub struct TourlFeature;

#[async_trait]
impl Feature for TourlFeature {
    fn name(&self) -> &'static str {
        "tourl"
    }

    fn description(&self) -> &'static str {
        "Mengubah media menjadi URL"
    }

    fn usage(&self) -> &'static str {
        "tourl (reply ke pesan media)"
    }

    fn category(&self) -> Category {
        Category::Utility
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let _ = ctx.react("⏳").await;

        let downloaded = media::download_media(&ctx.msg.client, &ctx.msg.message).await;
        let media = match downloaded {
            Ok(m) => m,
            Err(_) => {
                ctx.reply("❌ Reply atau kirim gambar/video dengan caption .tourl")
                    .await?;
                return Ok(());
            }
        };

        let ext = match media.kind {
            media::MediaKind::Image => "jpg",
            media::MediaKind::Video => "mp4",
            media::MediaKind::Audio => "mp3",
            media::MediaKind::Sticker => "webp",
            media::MediaKind::Document => "bin",
        };
        let filename = format!("wa_{}_{}.{ext}", media.kind.label(), ctx.msg.info.id);

        let url =
            tokio::task::spawn_blocking(move || media::catbox_upload(&media.bytes, &filename))
                .await?;

        let _ = ctx.react("").await;

        match url {
            Ok(link) => {
                let text = format!("✅ *URL:*\n{link}");
                ctx.reply_quoting(text).await
            }
            Err(_) => ctx.reply("❌ Gagal mengupload ke Catbox!").await,
        }
    }
}
