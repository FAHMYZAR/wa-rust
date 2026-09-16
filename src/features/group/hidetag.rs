use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

pub struct HidetagFeature;

#[async_trait]
impl Feature for HidetagFeature {
    fn name(&self) -> &'static str {
        "hidetag"
    }

    fn description(&self) -> &'static str {
        "Tag semua member tanpa menampilkan daftar nama"
    }

    fn usage(&self) -> &'static str {
        "hidetag <pesan>"
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
        if ctx.args.is_empty() {
            ctx.reply("❌ Format: `.hidetag pesan`").await?;
            return Ok(());
        }
        let meta = match group::get_metadata(&ctx.msg.client, ctx.group_jid()).await {
            Ok(m) => m,
            Err(_) => {
                ctx.reply("❌ Terjadi kesalahan!").await?;
                return Ok(());
            }
        };
        let mentioned: Vec<Jid> = meta.participants.iter().map(|p| p.jid.clone()).collect();
        ctx.send(group::hidetag_message(ctx.args, &mentioned)).await
    }
}
