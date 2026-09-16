use crate::core::feature::{Category, CommandContext, Feature};
use anyhow::Result;
use async_trait::async_trait;

pub struct GetJidFeature;

#[async_trait]
impl Feature for GetJidFeature {
    fn name(&self) -> &'static str {
        "getjid"
    }

    fn description(&self) -> &'static str {
        "Ambil JID user (untuk @lid format)"
    }

    fn category(&self) -> Category {
        Category::Owner
    }

    fn owner_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let target = match crate::utils::group::resolve_target_jid(ctx) {
            Ok(j) => j,
            Err(_) => {
                ctx.reply(
                    "❌ Tag atau reply pesan user untuk ambil JID!\n\nContoh:\n> `/getjid @user`\n> Reply pesan + `/getjid`",
                )
                .await?;
                return Ok(());
            }
        };

        let jid_str = target.to_string();
        let number = jid_str
            .replace("@s.whatsapp.net", "")
            .replace("@c.us", "")
            .replace("@lid", "")
            .split(':')
            .next()
            .unwrap_or("")
            .split('@')
            .next()
            .unwrap_or("")
            .to_string();

        let format_label = if jid_str.contains("@lid") {
            "@lid (Newsletter/Channel)"
        } else {
            "@s.whatsapp.net (Normal)"
        };

        let mut message = String::from("*📋 USER INFO*\n\n");
        message.push_str(&format!("*JID:* `{jid_str}`\n"));
        message.push_str(&format!("*Number:* {number}\n"));
        message.push_str(&format!("*Format:* {format_label}\n\n"));
        message.push_str("_Copy JID di atas untuk proteksi user_");

        let mentions = vec![target];
        let body = message;
        ctx.send(crate::utils::group::text_with_mentions(&body, &mentions))
            .await
    }
}
