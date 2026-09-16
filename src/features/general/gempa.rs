use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::http;
use anyhow::Result;
use async_trait::async_trait;

pub struct GempaFeature;

#[async_trait]
impl Feature for GempaFeature {
    fn name(&self) -> &'static str {
        "gempa"
    }

    fn description(&self) -> &'static str {
        "Info gempa terbaru BMKG"
    }

    fn category(&self) -> Category {
        Category::Utility
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let _ = ctx.react("🌍").await;

        let json = tokio::task::spawn_blocking(|| {
            http::get_json("https://data.bmkg.go.id/DataMKG/TEWS/autogempa.json")
        })
        .await
        .map_err(|e| anyhow::anyhow!("task join: {e}"))?;

        let _ = ctx.react("").await;

        let json = match json {
            Ok(j) => j,
            Err(e) => {
                ctx.reply(format!("gagal mengambil data gempa: {e}"))
                    .await?;
                return Ok(());
            }
        };

        let gempa = json
            .get("Infogempa")
            .and_then(|i| i.get("gempa"))
            .unwrap_or(&serde_json::Value::Null);

        let text = http::format_gempa(gempa);
        ctx.reply_quoting(text).await
    }
}
