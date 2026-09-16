use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Mutex;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media::{self as wa_media, ImageOptions};
use whatsapp_rust::upload::UploadOptions;

#[derive(Default)]
pub struct ReminiFeature {
    processing: Mutex<HashSet<String>>,
}

#[async_trait]
impl Feature for ReminiFeature {
    fn name(&self) -> &'static str {
        "remini"
    }

    fn description(&self) -> &'static str {
        "Enhance kualitas gambar menjadi HD"
    }

    fn usage(&self) -> &'static str {
        "remini (reply ke gambar)"
    }

    fn category(&self) -> Category {
        Category::Media
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let room = ctx.group_jid().to_string();
        let inserted = self
            .processing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(room.clone());
        if !inserted {
            return ctx
                .reply("⏳ Masih ada proses yang belum selesai, tunggu sebentar ya!")
                .await;
        }
        let result = self.execute_inner(ctx).await;
        self.processing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&room);
        if let Err(error) = result {
            let reply = if error.to_string().contains("API remini gagal") {
                "❌ API Remini sedang bermasalah! Coba lagi nanti"
            } else {
                "❌ Gagal enhance gambar!"
            };
            ctx.reply(reply).await?;
        }
        Ok(())
    }
}

impl ReminiFeature {
    async fn execute_inner(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if media::locate_media(&ctx.msg.message).is_none() {
            return ctx
                .reply("❌ Reply atau kirim gambar dengan caption `.remini`")
                .await;
        }
        let downloaded = media::download_media(&ctx.msg.client, &ctx.msg.message).await;
        let media = match downloaded {
            Ok(media) => media,
            Err(_) => {
                ctx.reply("❌ Reply atau kirim gambar dengan caption `.remini`")
                    .await?;
                return Ok(());
            }
        };

        let is_image =
            media.kind == media::MediaKind::Image || media.mimetype.starts_with("image/");
        if !is_image {
            ctx.reply("❌ Reply atau kirim gambar dengan caption `.remini`")
                .await?;
            return Ok(());
        }

        if media.bytes.len() > 10 * 1024 * 1024 {
            ctx.reply("❌ Gambar terlalu besar! Maksimal 10MB").await?;
            return Ok(());
        }

        let _ = ctx.react("⏳").await;

        let enhance_res: Result<()> = async {
            let png_bytes =
                tokio::task::spawn_blocking(move || enhance_image(&media.bytes)).await??;

            let upload = ctx
                .msg
                .client
                .upload(png_bytes, MediaType::Image, UploadOptions::default())
                .await
                .context("upload hasil enhance gagal")?;

            let text = "✅ Gambar berhasil di-enhance!\n\n✨ Enhanced by Remini AI".to_string();

            let msg = wa_media::image_message(
                upload,
                ImageOptions {
                    caption: Some(text),
                    mimetype: Some("image/png".into()),
                    ..Default::default()
                },
            );

            ctx.send(msg).await
        }
        .await;

        let _ = ctx.unreact().await;
        enhance_res
    }
}

fn enhance_image(input: &[u8]) -> Result<Vec<u8>> {
    let key = std::env::var("RESITA_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| anyhow::anyhow!("RESITA_API_KEY belum dikonfigurasi"))?;
    let image_url = tokio_block_on_upload(input)?;
    let enhanced_url = tokio_block_on_remini(&image_url, &key)?;
    crate::utils::http::get_bytes(&enhanced_url, 60)
}

fn tokio_block_on_upload(bytes: &[u8]) -> Result<String> {
    let filename = format!("remini_{}.jpg", std::process::id());
    media::catbox_upload(bytes, &filename)
}

fn tokio_block_on_remini(image_url: &str, api_key: &str) -> Result<String> {
    let url = format!(
        "https://api.ferdev.my.id/tools/remini?link={}&apikey={}",
        urlencoding::encode(image_url),
        urlencoding::encode(api_key),
    );
    let parsed = crate::utils::http::get_json(&url)?;
    if !parsed
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        anyhow::bail!("api remini gagal");
    }
    let data = parsed
        .get("data")
        .and_then(|d| d.as_str())
        .ok_or_else(|| anyhow::anyhow!("api remini gagal"))?;
    Ok(data.to_owned())
}
