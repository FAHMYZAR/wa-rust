use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

pub struct AdminListFeature;

#[async_trait]
impl Feature for AdminListFeature {
    fn name(&self) -> &'static str {
        "adminlist"
    }

    fn description(&self) -> &'static str {
        "Lihat daftar admin grup"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["listadmin"]
    }

    fn category(&self) -> Category {
        Category::Group
    }

    fn group_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if !ctx.is_group {
            ctx.reply("❌ Perintah ini hanya untuk grup!").await?;
            return Ok(());
        }

        let meta = group::get_metadata(&ctx.msg.client, ctx.group_jid()).await?;
        let admins: Vec<&whatsapp_rust::GroupParticipant> =
            meta.participants.iter().filter(|p| p.is_admin()).collect();

        if admins.is_empty() {
            ctx.reply("❌ Tidak ada admin di grup ini!").await?;
            return Ok(());
        }

        let mut message = String::from("*DAFTAR ADMIN*\n\n");
        message.push_str(&format!("*Grup:* {}\n", meta.subject));
        message.push_str(&format!("*Total Admin:* {}\n\n", admins.len()));

        for (index, admin) in admins.iter().enumerate() {
            let role = if admin.is_super_admin() {
                "Owner"
            } else {
                "Admin"
            };
            message.push_str(&format!("*{}.* @{}\n", index + 1, admin.jid.user));
            message.push_str(&format!("Role: {}\n\n", role));
        }

        let mentions: Vec<Jid> = admins.iter().map(|a| a.jid.clone()).collect();
        ctx.send(group::text_with_mentions(&message, &mentions))
            .await
    }
}
