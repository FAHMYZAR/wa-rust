use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct CloseFeature;

#[async_trait]
impl Feature for CloseFeature {
    fn name(&self) -> &'static str {
        "close"
    }

    fn description(&self) -> &'static str {
        "Tutup grup (hanya admin bisa kirim)"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["mute"]
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
            ctx.reply("❌ Hanya admin yang bisa close grup!").await?;
            return Ok(());
        }

        if !group::is_bot_admin(&ctx.msg.client, ctx.group_jid()).await? {
            ctx.reply("❌ Bot harus jadi admin untuk close grup!")
                .await?;
            return Ok(());
        }

        match ctx
            .msg
            .client
            .groups()
            .set_announce(ctx.group_jid(), true)
            .await
        {
            Ok(()) => {
                ctx.reply("🔒 *GRUP DITUTUP*\n\nHanya admin yang bisa mengirim pesan!")
                    .await?;
            }
            Err(_) => {
                ctx.reply("❌ Gagal menutup grup!").await?;
            }
        }
        Ok(())
    }
}
