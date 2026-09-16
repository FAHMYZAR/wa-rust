use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;

pub struct TranslateFeature;

#[async_trait]
impl Feature for TranslateFeature {
    fn name(&self) -> &'static str {
        "tr"
    }

    fn description(&self) -> &'static str {
        "Terjemahkan teks ke bahasa lain"
    }

    fn usage(&self) -> &'static str {
        "tr <kode_bahasa> [teks]"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["translate"]
    }

    fn category(&self) -> Category {
        Category::Utility
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let (target_lang, text_arg) = match ctx.args.split_once(char::is_whitespace) {
            Some((lang, rest)) => (lang.trim(), rest.trim()),
            None => (ctx.args.trim(), ""),
        };

        let quoted_text = group::context_info(&ctx.msg.message)
            .and_then(|ci| ci.quoted_message.as_option())
            .and_then(|qm| {
                qm.extended_text_message
                    .as_option()
                    .and_then(|et| et.text.clone())
            })
            .unwrap_or_default();

        let source_text = if !text_arg.is_empty() {
            text_arg.to_string()
        } else if !quoted_text.is_empty() {
            quoted_text
        } else {
            ctx.reply("format: tr <kode_bahasa> <teks> atau reply pesan dengan: tr <kode_bahasa>")
                .await?;
            return Ok(());
        };

        if target_lang.is_empty() {
            ctx.reply("sebutkan kode bahasa tujuan, contoh: tr id hello world")
                .await?;
            return Ok(());
        }

        let _ = ctx.react("🔄").await;
        let lang = target_lang.to_string();
        let query = source_text.clone();

        let translated = tokio::task::spawn_blocking(move || translate_text(&lang, &query)).await?;
        let _ = ctx.react("").await;

        match translated {
            Ok(res) => {
                let flag = match target_lang {
                    "id" => "🇮🇩",
                    "en" => "🇬🇧",
                    "ja" => "🇯🇵",
                    "ko" => "🇰🇷",
                    "ar" => "🇸🇦",
                    "zh" => "🇨🇳",
                    "es" => "🇪🇸",
                    "fr" => "🇫🇷",
                    "de" => "🇩🇪",
                    "ru" => "🇷🇺",
                    _ => "🌐",
                };
                ctx.reply_quoting(format!("{flag}\n`{res}`")).await
            }
            Err(e) => ctx.reply(format!("gagal menerjemahkan: {e}")).await,
        }
    }
}

fn translate_text(target: &str, text: &str) -> Result<String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&q={}",
        urlencoding::encode(target),
        urlencoding::encode(text)
    );
    let parsed: serde_json::Value = crate::utils::http::get_json(&url)?;
    let mut out = String::new();
    if let Some(sentences) = parsed.get(0).and_then(|v| v.as_array()) {
        for s in sentences {
            if let Some(txt) = s.get(0).and_then(|v| v.as_str()) {
                out.push_str(txt);
            }
        }
    }
    if out.is_empty() {
        anyhow::bail!("hasil terjemahan kosong");
    }
    Ok(out)
}
