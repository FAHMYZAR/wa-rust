use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::{group, warn};
use anyhow::Result;
use async_trait::async_trait;

pub struct WarnFeature;

#[async_trait]
impl Feature for WarnFeature {
    fn name(&self) -> &'static str {
        "warn"
    }

    fn description(&self) -> &'static str {
        "Beri warning ke member (3x = kick otomatis)"
    }

    fn usage(&self) -> &'static str {
        "warn <reply/tag> [alasan]"
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
            ctx.reply("hanya admin yang bisa memberi warning.").await?;
            return Ok(());
        }
        let target = match group::resolve_target_jid(ctx) {
            Ok(j) => j,
            Err(e) => {
                ctx.reply(format!("{e}")).await?;
                return Ok(());
            }
        };
        if let Some(own) = client.pn() {
            if own.user == target.user {
                ctx.reply("tidak bisa memberi warning ke bot.").await?;
                return Ok(());
            }
        }
        if group::is_admin(client, ctx.group_jid(), &target).await? {
            ctx.reply("tidak bisa memberi warning ke admin.").await?;
            return Ok(());
        }

        let reason = if ctx.args.is_empty() {
            "Tidak ada alasan"
        } else {
            ctx.args
        };
        let total = warn::add_warn(
            &ctx.group_jid().to_string(),
            &target.to_string(),
            reason,
            &ctx.sender_jid().to_string(),
        )?;

        let mut msg = format!(
            "⚠️ *WARNING*\n\nMember: @{}\nAlasan: {reason}\nTotal Warn: {total}/{}\n\n",
            target.user,
            warn::MAX_WARNS
        );

        if total >= warn::MAX_WARNS {
            let is_bot_admin = group::is_bot_admin(client, ctx.group_jid()).await?;
            if is_bot_admin {
                let _ = client
                    .groups()
                    .remove_participants(ctx.group_jid(), std::slice::from_ref(&target))
                    .await;
                warn::reset_warns(&ctx.group_jid().to_string(), &target.to_string())?;
                msg.push_str("❌ Member telah di-kick karena mencapai batas warning!");
            } else {
                msg.push_str("⚠️ Member mencapai batas warning, tapi bot bukan admin untuk kick!");
            }
        } else {
            msg.push_str(&format!(
                "⚠️ {} warning lagi akan di-kick!",
                warn::MAX_WARNS - total
            ));
        }

        ctx.send(group::text_with_mentions(
            &msg,
            std::slice::from_ref(&target),
        ))
        .await
    }
}
