use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::{group, warn};
use anyhow::Result;
use async_trait::async_trait;

pub struct UnwarnFeature;

#[async_trait]
impl Feature for UnwarnFeature {
    fn name(&self) -> &'static str {
        "unwarn"
    }

    fn description(&self) -> &'static str {
        "Hapus 1 warning dari member"
    }

    fn usage(&self) -> &'static str {
        "unwarn <reply/tag>"
    }

    fn category(&self) -> Category {
        Category::Group
    }

    fn group_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if !ctx.is_group {
            ctx.reply("perintah ini hanya bisa dipakai di dalam group.")
                .await?;
            return Ok(());
        }
        let client = &ctx.msg.client;
        if !group::is_admin(client, ctx.group_jid(), ctx.sender_jid()).await? {
            ctx.reply("hanya admin yang bisa menghapus warning.")
                .await?;
            return Ok(());
        }
        let target = match group::resolve_target_jid(ctx) {
            Ok(j) => j,
            Err(e) => {
                ctx.reply(format!("{e}")).await?;
                return Ok(());
            }
        };

        let removed = warn::remove_warn(&ctx.group_jid().to_string(), &target.to_string())?;
        if !removed {
            ctx.reply("member ini tidak punya warning.").await?;
            return Ok(());
        }
        let total = warn::get_warn_count(&ctx.group_jid().to_string(), &target.to_string())?;
        let msg = format!(
            "✅ 1 warning dihapus!\n\nMember: @{}\nSisa Warn: {total}/{}\n",
            target.user,
            warn::MAX_WARNS
        );
        ctx.send(group::text_with_mentions(
            &msg,
            std::slice::from_ref(&target),
        ))
        .await
    }
}
