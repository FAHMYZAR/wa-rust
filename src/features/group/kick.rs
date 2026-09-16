use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct KickFeature;

#[async_trait]
impl Feature for KickFeature {
    fn name(&self) -> &'static str {
        "kick"
    }

    fn description(&self) -> &'static str {
        "Keluarkan member dari grup (reply/tag/nomor)"
    }

    fn usage(&self) -> &'static str {
        "kick <reply/tag/nomor>"
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
        if let Some(own) = client.pn() {
            if own.user == target.user {
                ctx.reply("tidak bisa mengeluarkan bot itu sendiri.")
                    .await?;
                return Ok(());
            }
        }
        if group::is_admin(client, ctx.group_jid(), &target).await? {
            ctx.reply("tidak bisa mengeluarkan admin.").await?;
            return Ok(());
        }
        let results = client
            .groups()
            .remove_participants(ctx.group_jid(), std::slice::from_ref(&target))
            .await
            .map_err(|e| anyhow::anyhow!("gagal mengeluarkan member: {e}"))?;
        if results.iter().any(|r| r.is_ok()) {
            ctx.send(group::text_with_mentions(
                "✅ Member berhasil dikeluarkan!",
                std::slice::from_ref(&target),
            ))
            .await
        } else {
            ctx.reply("gagal mengeluarkan member.").await
        }
    }
}
