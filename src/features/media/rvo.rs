use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media as wa_media;
use whatsapp_rust::media::{AudioOptions, ImageOptions, VideoOptions};
use whatsapp_rust::upload::UploadOptions;

pub struct RvoFeature;

#[async_trait]
impl Feature for RvoFeature {
    fn name(&self) -> &'static str {
        "rvo"
    }

    fn description(&self) -> &'static str {
        "Ekstrak media view once"
    }

    fn usage(&self) -> &'static str {
        "rvo (reply ke pesan view-once)"
    }

    fn category(&self) -> Category {
        Category::Owner
    }

    fn owner_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let media = match media::download_media(&ctx.msg.client, &ctx.msg.message).await {
            Ok(media) => media,
            Err(_) => {
                ctx.reply("❌ Balas ke media (View Once atau biasa) yang mau diambil!")
                    .await?;
                return Ok(());
            }
        };

        ctx.reply("⏳ Mengekstrak media...").await?;

        let upload_type = match media.kind {
            media::MediaKind::Image => MediaType::Image,
            media::MediaKind::Video => MediaType::Video,
            media::MediaKind::Audio => MediaType::Audio,
            media::MediaKind::Document | media::MediaKind::Sticker => {
                ctx.reply("❌ Media tidak didukung untuk diekstrak!")
                    .await?;
                return Ok(());
            }
        };
        let message_caption = media
            .caption
            .as_deref()
            .filter(|caption| !caption.is_empty())
            .map_or_else(
                || "✅ Media berhasil diambil!".to_owned(),
                |caption| format!("*Pesan:* {caption}"),
            );

        let upload = match ctx
            .msg
            .client
            .upload(media.bytes, upload_type, UploadOptions::default())
            .await
        {
            Ok(upload) => upload,
            Err(error) => {
                ctx.reply(format!("❌ Gagal memproses media: {error}"))
                    .await?;
                return Ok(());
            }
        };

        let outgoing = match media.kind {
            media::MediaKind::Image => wa_media::image_message(
                upload,
                ImageOptions {
                    caption: Some(message_caption),
                    mimetype: Some(media.mimetype),
                    ..Default::default()
                },
            ),
            media::MediaKind::Video => wa_media::video_message(
                upload,
                VideoOptions {
                    caption: Some(message_caption),
                    mimetype: Some(media.mimetype),
                    ..Default::default()
                },
            ),
            media::MediaKind::Audio => wa_media::audio_message(
                upload,
                AudioOptions {
                    mimetype: Some(media.mimetype),
                    ..Default::default()
                },
            ),
            media::MediaKind::Document | media::MediaKind::Sticker => unreachable!(),
        };

        if let Err(error) = ctx.send(outgoing).await {
            ctx.reply(format!("❌ Gagal memproses media: {error}"))
                .await?;
        }
        Ok(())
    }
}
