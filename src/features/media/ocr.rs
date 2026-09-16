use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::Result;
use async_trait::async_trait;

pub struct OcrFeature;

#[async_trait]
impl Feature for OcrFeature {
    fn name(&self) -> &'static str {
        "ocr"
    }

    fn description(&self) -> &'static str {
        "Ekstrak teks dari gambar (OCR)"
    }

    fn usage(&self) -> &'static str {
        "ocr (reply ke pesan gambar)"
    }

    fn category(&self) -> Category {
        Category::Utility
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let _ = ctx.react("📄").await;

        let downloaded = media::download_media(&ctx.msg.client, &ctx.msg.message).await;
        let media = match downloaded {
            Ok(m) => m,
            Err(_) => {
                ctx.reply("❌ Reply atau kirim gambar dengan caption `.ocr`")
                    .await?;
                return Ok(());
            }
        };

        if matches!(
            media.kind,
            media::MediaKind::Audio | media::MediaKind::Video
        ) || media.mimetype.starts_with("video/")
        {
            let _ = ctx.react("❌").await;
            ctx.reply("OCR hanya mendukung gambar, bukan video/audio.")
                .await?;
            return Ok(());
        }

        if media.bytes.len() > 10 * 1024 * 1024 {
            let _ = ctx.react("❌").await;
            ctx.reply("❌ Gambar terlalu besar! Maksimal 10MB").await?;
            return Ok(());
        }

        let caption = media.caption.clone();
        let ext = "jpg";
        let filename = format!("ocr_{}_{}.{ext}", ctx.msg.info.id, caption.is_some());

        let url = match tokio::task::spawn_blocking(move || {
            media::catbox_upload(&media.bytes, &filename)
        })
        .await?
        {
            Ok(u) => u,
            Err(e) => {
                let _ = ctx.react("❌").await;
                ctx.reply(format!("gagal upload gambar: {e}")).await?;
                return Ok(());
            }
        };

        let extracted = tokio::task::spawn_blocking(move || extract_text(&url)).await?;
        let _ = ctx.react("").await;

        match extracted {
            Ok(text) => {
                ctx.reply(format!(
                    "📄 *OCR Result*\n\n{text}\n\n_✨ Extracted by EL-RUWET [BOT + AI]_"
                ))
                .await?;
            }
            Err(e) => {
                ctx.reply(format!("gagal mengekstrak teks: {e}")).await?;
            }
        }
        Ok(())
    }
}

/// OCR via ocr.space free endpoint. `OCR_API_KEY` env overrides the bundled
/// demo key.
pub fn extract_text(image_url: &str) -> Result<String> {
    let key = std::env::var("OCR_API_KEY").unwrap_or_else(|_| "helloworld".into());
    let url = format!(
        "https://api.ocr.space/parse/imageurl?apikey={}&url={}&OCREngine=2",
        urlencoding::encode(&key),
        urlencoding::encode(image_url),
    );
    let parsed = crate::utils::http::get_json(&url)?;
    let text = parsed
        .get("ParsedResults")
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
        .and_then(|r| r.get("ParsedText"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        let err = parsed
            .get("ErrorMessage")
            .and_then(|e| e.as_array())
            .and_then(|e| e.first())
            .and_then(|e| e.as_str())
            .unwrap_or("teks tidak ditemukan pada gambar");
        anyhow::bail!("{err}");
    }
    Ok(text.to_owned())
}
