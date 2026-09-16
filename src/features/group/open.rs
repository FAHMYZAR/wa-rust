use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct OpenFeature;

#[async_trait]
impl Feature for OpenFeature {
    fn name(&self) -> &'static str {
        "open"
    }

    fn description(&self) -> &'static str {
        "Buka grup (semua bisa kirim)"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["unmute"]
    }

    fn category(&self) -> Category {
        Category::Group
    }

    fn group_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if !ctx.is_group {
            ctx.reply("❌ Perintah ini hanya untuk grup!").await?;
            return Ok(());
        }

        if !group::is_admin(&ctx.msg.client, ctx.group_jid(), ctx.sender_jid()).await? {
            ctx.reply("❌ Hanya admin yang bisa open grup!").await?;
            return Ok(());
        }

        if !group::is_bot_admin(&ctx.msg.client, ctx.group_jid()).await? {
            ctx.reply("❌ Bot harus jadi admin untuk open grup!")
                .await?;
            return Ok(());
        }

        match ctx
            .msg
            .client
            .groups()
            .set_announce(ctx.group_jid(), false)
            .await
        {
            Ok(()) => {
                ctx.reply("🔓 *GRUP DIBUKA*\n\nSemua member bisa mengirim pesan!")
                    .await?;
            }
            Err(_) => {
                ctx.reply("❌ Gagal membuka grup!").await?;
            }
        }
        Ok(())
    }
}
