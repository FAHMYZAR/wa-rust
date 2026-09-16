use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct DemoteFeature;

#[async_trait]
impl Feature for DemoteFeature {
    fn name(&self) -> &'static str {
        "demote"
    }

    fn description(&self) -> &'static str {
        "Cabut admin dari member"
    }

    fn usage(&self) -> &'static str {
        "demote <reply/tag/nomor>"
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
        let target = match group::resolve_target_jid(ctx) {
            Ok(j) => j,
            Err(e) => {
                ctx.reply(format!("{e}")).await?;
                return Ok(());
            }
        };
        let client = &ctx.msg.client;
        let meta = group::get_metadata(client, ctx.group_jid()).await?;
        if !group::is_admin_from_meta(&meta, &target) {
            ctx.reply("member ini bukan admin.").await?;
            return Ok(());
        }
        client
            .groups()
            .demote_participants(ctx.group_jid(), std::slice::from_ref(&target))
            .await
            .map_err(|e| anyhow::anyhow!("gagal demote: {e}"))?;
        ctx.send(group::text_with_mentions(
            "✅ Admin berhasil dicabut haknya!",
            std::slice::from_ref(&target),
        ))
        .await
    }
}
