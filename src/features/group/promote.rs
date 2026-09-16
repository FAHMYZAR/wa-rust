use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct PromoteFeature;

#[async_trait]
impl Feature for PromoteFeature {
    fn name(&self) -> &'static str {
        "promote"
    }

    fn description(&self) -> &'static str {
        "Jadikan member sebagai admin"
    }

    fn usage(&self) -> &'static str {
        "promote <reply/tag/nomor>"
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
        let target_is_member = meta.participants.iter().any(|p| p.jid.user == target.user);
        if !target_is_member {
            ctx.reply("target bukan member grup ini.").await?;
            return Ok(());
        }
        if group::is_admin_from_meta(&meta, &target) {
            ctx.reply("member ini sudah admin.").await?;
            return Ok(());
        }
        client
            .groups()
            .promote_participants(ctx.group_jid(), std::slice::from_ref(&target))
            .await
            .map_err(|e| anyhow::anyhow!("gagal promote: {e}"))?;
        ctx.send(group::text_with_mentions(
            "✅ Member berhasil dijadikan admin!",
            std::slice::from_ref(&target),
        ))
        .await
    }
}
