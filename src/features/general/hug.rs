use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;

pub struct HugFeature;

#[async_trait]
impl Feature for HugFeature {
    fn name(&self) -> &'static str {
        "hug"
    }

    fn description(&self) -> &'static str {
        "Random hug anime sticker"
    }

    fn category(&self) -> Category {
        Category::Fun
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let result: Result<()> = async {
            ctx.react("❤️").await?;
            let bytes = tokio::task::spawn_blocking(fetch_hug_sticker).await??;
            let webp =
                tokio::task::spawn_blocking(move || convert_to_sticker_webp(&bytes)).await??;
            let upload = ctx
                .msg
                .client
                .upload(webp, MediaType::Sticker, UploadOptions::default())
                .await
                .map_err(|e| anyhow::anyhow!("upload gagal: {e}"))?;
            let send_res = ctx.send(media::sticker_message(upload, true)).await;
            let _ = ctx.unreact().await;
            send_res
        }
        .await;
        if result.is_err() {
            let _ = ctx.unreact().await;
            ctx.reply("❌ Gagal mengirim pelukan!").await?;
        }
        Ok(())
    }
}

fn fetch_hug_sticker() -> anyhow::Result<Vec<u8>> {
    let key = std::env::var("LOLHUMAN_API_KEY").unwrap_or_default();
    let url = format!(
        "https://api.lolhuman.xyz/api/random/sfw/hug?apikey={}",
        urlencoding::encode(&key)
    );
    crate::utils::http::get_bytes(&url, 15)
}

/// Convert API response (image/gif) to animated WebP sticker via ffmpeg.
/// Pure Rust + ffmpeg, no Node.js/sharp dependency.
fn convert_to_sticker_webp(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use anyhow::Context as _;
    use std::process::Command;

    let temp_in = media::temp_file_path("hug_in", "bin");
    let temp_out = media::temp_file_path("hug_out", "webp");
    std::fs::write(&temp_in, bytes)?;

    let status = Command::new("ffmpeg")
        .args([
            std::ffi::OsStr::new("-y"),
            std::ffi::OsStr::new("-i"),
            temp_in.as_os_str(),
            std::ffi::OsStr::new("-vf"),
            std::ffi::OsStr::new("scale='if(gt(a,1),512,-1)':'if(gt(a,1),-1,512)',pad=512:512:(512-iw)/2:(512-ih)/2:color=0x00000000,fps=15"),
            std::ffi::OsStr::new("-c:v"),
            std::ffi::OsStr::new("libwebp"),
            std::ffi::OsStr::new("-lossless"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-q:v"),
            std::ffi::OsStr::new("95"),
            std::ffi::OsStr::new("-loop"),
            std::ffi::OsStr::new("0"),
            std::ffi::OsStr::new("-an"),
            temp_out.as_os_str(),
        ])
        .status()
        .context("ffmpeg hug convert gagal")?;

    let _ = std::fs::remove_file(&temp_in);
    if status.success() {
        let out = std::fs::read(&temp_out)?;
        let _ = std::fs::remove_file(&temp_out);
        Ok(out)
    } else {
        let _ = std::fs::remove_file(&temp_out);
        anyhow::bail!("konversi hug ke webp stiker gagal");
    }
}
