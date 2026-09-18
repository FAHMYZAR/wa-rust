use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::mode::{self, BotMode};
use anyhow::Result;
use async_trait::async_trait;

pub struct SetModeFeature;

#[async_trait]
impl Feature for SetModeFeature {
    fn name(&self) -> &'static str {
        "setmode"
    }

    fn description(&self) -> &'static str {
        "Atur mode akses bot: public, private, atau chat"
    }

    fn usage(&self) -> &'static str {
        "setmode <public|private|chat>"
    }

    fn category(&self) -> Category {
        Category::Owner
    }

    fn owner_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let args = ctx.args.trim();

        // /setmode tanpa argumen -> lihat mode aktif.
        if args.is_empty() {
            let current = mode::get_mode()?;

            ctx.reply(format!(
                "*BOT MODE*\n\n\
                 Mode aktif: *{}*\n\
                 {}\n\n\
                 *Pilihan mode:*\n\
                 • `public` — semua orang, private + grup\n\
                 • `private` — hanya owner\n\
                 • `chat` — hanya private chat",
                current.as_str(),
                current.description()
            ))
            .await?;

            return Ok(());
        }

        let Some(new_mode) = BotMode::parse(args) else {
            ctx.reply(
                "*Mode tidak valid.*\n\n\
                 Gunakan salah satu:\n\
                 • `/setmode public`\n\
                 • `/setmode private`\n\
                 • `/setmode chat`",
            )
            .await?;

            return Ok(());
        };

        let current = mode::get_mode()?;

        if current == new_mode {
            ctx.reply(format!(
                "Mode bot sudah berada pada *{}*.\n\n{}",
                new_mode.as_str(),
                new_mode.description()
            ))
            .await?;

            return Ok(());
        }

        mode::set_mode(new_mode)?;

        ctx.reply(format!(
            "✅ Mode bot berhasil diubah.\n\n\
             *{}* → *{}*\n\n\
             {}",
            current.as_str(),
            new_mode.as_str(),
            new_mode.description()
        ))
        .await?;

        Ok(())
    }
}