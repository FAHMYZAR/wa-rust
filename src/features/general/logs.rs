use crate::core::feature::{Category, CommandContext, Feature};
use anyhow::Result;
use async_trait::async_trait;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct LogsFeature;

#[async_trait]
impl Feature for LogsFeature {
    fn name(&self) -> &'static str {
        "logs"
    }

    fn description(&self) -> &'static str {
        "Lihat baris log terminal bot terakhir"
    }

    fn usage(&self) -> &'static str {
        "logs [jumlah_baris]"
    }

    fn category(&self) -> Category {
        Category::Owner
    }

    fn owner_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let count: usize = ctx.args.parse().unwrap_or(20).clamp(1, 100);

        let paths = ["bot.log", "bot_logs.txt", "output.log"];
        let mut target_path = None;
        for p in paths {
            if std::path::Path::new(p).exists() {
                target_path = Some(p);
                break;
            }
        }

        let Some(path) = target_path else {
            ctx.reply("ℹ️ Belum ada file log yang tersimpan di disk. Bot mencetak langsung ke stdout/stderr.")
                .await?;
            return Ok(());
        };

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
        let tail = lines
            .iter()
            .rev()
            .take(count)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        if tail.is_empty() {
            ctx.reply("Log kosong.").await?;
        } else {
            ctx.reply(format!(
                "📋 *BOT LOGS (terakhir {count} baris):*\n\n```\n{tail}\n```"
            ))
            .await?;
        }
        Ok(())
    }
}
