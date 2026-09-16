use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct GrouplinkFeature;

#[async_trait]
impl Feature for GrouplinkFeature {
    fn name(&self) -> &'static str {
        "grouplink"
    }

    fn description(&self) -> &'static str {
        "Ambil link undangan grup"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["linkgroup", "linkgrup"]
    }

    fn category(&self) -> Category {
        Category::Group
    }

    fn group_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if !group::require_admins(ctx).await? {
            return Ok(());
        }
        let code = ctx
            .msg
            .client
            .groups()
            .get_invite_link(ctx.group_jid(), false)
            .await
            .map_err(|e| anyhow::anyhow!("gagal mengambil link: {e}"))?;
        let meta = group::get_metadata(&ctx.msg.client, ctx.group_jid()).await?;
        let msg = format!(
            "🔗 *LINK GRUP*\n\n*Nama:* {}\n*Link:* https://chat.whatsapp.com/{code}",
            meta.subject
        );
        ctx.reply_quoting(msg).await
    }
}
