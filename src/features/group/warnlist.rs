use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::{group, warn};
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

pub struct WarnlistFeature;

#[async_trait]
impl Feature for WarnlistFeature {
    fn name(&self) -> &'static str {
        "warnlist"
    }

    fn description(&self) -> &'static str {
        "Lihat daftar warning di grup"
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
            ctx.reply("hanya admin yang bisa melihat daftar warning.")
                .await?;
            return Ok(());
        }
        let entries = warn::get_all_warns_in_group(&ctx.group_jid().to_string())?;
        if entries.is_empty() {
            ctx.reply("✅ tidak ada member yang punya warning.").await?;
            return Ok(());
        }
        let mut message = String::from("*DAFTAR WARNING*\n\n");
        let mut mentions: Vec<Jid> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            let user = e.user_id.split('@').next().unwrap_or(&e.user_id);
            if let Ok(jid) = e.user_id.parse::<Jid>() {
                mentions.push(jid);
            }
            message.push_str(&format!(
                "*{}.* @{user}\nWarn: {}/{}\n",
                i + 1,
                e.total_warns,
                warn::MAX_WARNS
            ));
            if let Some(last) = &e.last_reason {
                message.push_str(&format!("Terakhir: {last}\n"));
            }
            message.push('\n');
        }
        ctx.send(group::text_with_mentions(&message, &mentions))
            .await
    }
}
