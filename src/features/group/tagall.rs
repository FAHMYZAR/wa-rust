use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

pub struct TagallFeature;

#[async_trait]
impl Feature for TagallFeature {
    fn name(&self) -> &'static str {
        "tagall"
    }

    fn description(&self) -> &'static str {
        "Tag semua member grup"
    }

    fn usage(&self) -> &'static str {
        "tagall [pesan]"
    }

    fn category(&self) -> Category {
        Category::Group
    }

    fn group_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if !ctx.is_group {
            ctx.reply("❌ Perintah ini hanya bisa digunakan di grup!")
                .await?;
            return Ok(());
        }
        let meta = match group::get_metadata(&ctx.msg.client, ctx.group_jid()).await {
            Ok(m) => m,
            Err(_) => {
                ctx.reply("❌ Terjadi kesalahan saat tag all!").await?;
                return Ok(());
            }
        };
        let mentioned: Vec<Jid> = meta.participants.iter().map(|p| p.jid.clone()).collect();
        let header = if ctx.args.is_empty() {
            "📢 *TAG ALL*".to_string()
        } else {
            ctx.args.to_string()
        };
        let body = format!(
            "{header}\n\n{}",
            mentioned
                .iter()
                .map(|j| format!("@{}", j.user))
                .collect::<Vec<_>>()
                .join("\n")
        );
        ctx.send(group::text_with_mentions(&body, &mentioned)).await
    }
}
