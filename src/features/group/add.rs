use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

pub struct AddFeature;

#[async_trait]
impl Feature for AddFeature {
    fn name(&self) -> &'static str {
        "add"
    }

    fn description(&self) -> &'static str {
        "Tambah member ke grup (nomor)"
    }

    fn usage(&self) -> &'static str {
        "add <nomor>"
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
        let digits: String = ctx.args.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 8 {
            ctx.reply("format: add 628123456789 atau reply target")
                .await?;
            return Ok(());
        }
        let normalized = if let Some(rest) = digits.strip_prefix('0') {
            format!("62{rest}")
        } else {
            digits
        };
        let target = Jid::pn(normalized);
        let client = &ctx.msg.client;
        let results = client
            .groups()
            .add_participants(ctx.group_jid(), std::slice::from_ref(&target))
            .await
            .map_err(|e| anyhow::anyhow!("gagal menambah member: {e}"))?;
        if results.iter().any(|r| r.is_ok()) {
            ctx.send(group::text_with_mentions(
                "✅ Member berhasil ditambahkan!",
                std::slice::from_ref(&target),
            ))
            .await
        } else {
            ctx.reply("❌ Gagal menambahkan member (mungkin nomor privasi atau sudah ada).")
                .await
        }
    }
}
