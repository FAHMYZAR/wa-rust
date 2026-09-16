use crate::core::feature::{Category, CommandContext, Feature};
use anyhow::Result;
use async_trait::async_trait;

pub struct PingFeature;

#[async_trait]
impl Feature for PingFeature {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn description(&self) -> &'static str {
        "Cek response time bot"
    }

    fn category(&self) -> Category {
        Category::Utility
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let start = std::time::Instant::now();
        let result = async {
            ctx.reply("🏓 Pong!").await?;
            ctx.reply(format!(
                "⚡ Response time: {}ms",
                start.elapsed().as_millis()
            ))
            .await
        }
        .await;
        if result.is_err() {
            ctx.reply("❌ Terjadi kesalahan!").await?;
        }
        Ok(())
    }
}
