use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct ResetLinkFeature;

#[async_trait]
impl Feature for ResetLinkFeature {
    fn name(&self) -> &'static str {
        "resetlink"
    }

    fn description(&self) -> &'static str {
        "Reset link invite grup"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["revoke"]
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
            ctx.reply("❌ Hanya admin yang bisa reset link grup!")
                .await?;
            return Ok(());
        }

        if !group::is_bot_admin(&ctx.msg.client, ctx.group_jid()).await? {
            ctx.reply("❌ Bot harus jadi admin untuk reset link grup!")
                .await?;
            return Ok(());
        }

        let reset = ctx
            .msg
            .client
            .groups()
            .get_invite_link(ctx.group_jid(), true)
            .await;
        match reset {
            Ok(_) => {}
            Err(_) => {
                ctx.reply("❌ Gagal reset link grup!").await?;
                return Ok(());
            }
        }

        let code = match ctx
            .msg
            .client
            .groups()
            .get_invite_link(ctx.group_jid(), false)
            .await
        {
            Ok(c) => c,
            Err(_) => {
                ctx.reply("❌ Gagal reset link grup!").await?;
                return Ok(());
            }
        };

        let new_link = format!("https://chat.whatsapp.com/{code}");
        let message = format!(
            "✅ *LINK GRUP BERHASIL DIRESET*\n\n*Link Baru:* {new_link}\n\n_Link lama sudah tidak bisa digunakan!_"
        );
        ctx.reply(message).await
    }
}
