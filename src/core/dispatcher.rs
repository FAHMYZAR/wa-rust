use super::feature::{Category, CommandContext};
use super::registry::FeatureRegistry;
use crate::config::Config;
use crate::utils::mode;
use anyhow::Result;
use log::warn;
use std::sync::Arc;
use whatsapp_rust::prelude::*;

/// Parses incoming messages into commands and dispatches them to features.
/// Mirrors the JS `CommandHandler`: owner vs user prefix, permission checks.
pub struct Dispatcher {
    pub registry: Arc<FeatureRegistry>,
    pub config: Arc<Config>,
}

impl Dispatcher {
    /// Entry point for every incoming text message.
    pub async fn handle(&self, ctx: &MessageContext) -> Result<()> {
        let Some(text) = ctx.message.text_content() else {
            return Ok(());
        };

        // WhatsApp identifies self messages across phone-number and LID addresses.
        let is_owner = ctx.info.source.is_from_me;
        let is_group = ctx.info.source.is_group;

        // Global bot access mode.
        //
        // public:
        //   semua user bisa memakai bot di private maupun grup.
        //
        // private:
        //   hanya owner / akun bot sendiri yang boleh menggunakan command.
        //
        // chat:
        //   command hanya diproses dari private chat.
        //   Semua command dari grup diabaikan, termasuk command owner.
        let bot_mode = mode::get_mode()?;

        if !bot_mode.allows(is_owner, is_group) {
            return Ok(());
        }

        let (name, args) = if matches!(text, "!next" | "!prev" | "!back") {
            ("help", text)
        } else if let Some(category) = text.strip_prefix('!') {
            if Category::from_label(category).is_none() {
                return Ok(());
            }

            ("help", text)
        } else {
            let Some((prefix, rest)) = self.split_prefix(text.as_ref(), is_owner) else {
                return Ok(());
            };

            // Non-owner tidak boleh menggunakan owner prefix.
            if prefix == self.config.owner_prefix && !is_owner {
                return Ok(());
            }

            let Some(command) = split_command(rest) else {
                return Ok(());
            };

            command
        };

        let Some(feature) = self.registry.get(name) else {
            return Ok(());
        };

        // Feature yang memang owner-only tetap tidak pernah dibuka ke user
        // meskipun bot sedang dalam mode public.
        if feature.owner_only() && !is_owner {
            return Ok(());
        }

        // Pertahankan behaviour lama:
        // semua command category Group hanya dapat dijalankan owner.
        //
        // Guard ini sengaja tetap dipertahankan karena beberapa command
        // moderasi grup belum melakukan pengecekan admin sendiri.
        if feature.category() == Category::Group && !is_owner {
            return Ok(());
        }

        if feature.group_only() && !is_group {
            return Ok(());
        }

        let cmd_ctx = CommandContext {
            msg: ctx,
            args,
            is_owner,
            is_group,
        };

        if let Err(e) = feature.execute(&cmd_ctx).await {
            warn!("feature '{}' failed: {e:#}", feature.name());

            let _ = cmd_ctx.unreact().await;
        }

        Ok(())
    }
}

impl Dispatcher {
    /// Returns (prefix_used, text_after_prefix).
    ///
    /// Owner prefix wins when sender is owner and message starts with it.
    /// Otherwise user prefix is checked.
    fn split_prefix<'a>(
        &'a self,
        text: &'a str,
        is_owner: bool,
    ) -> Option<(&'a str, &'a str)> {
        if is_owner {
            if let Some(rest) = text.strip_prefix(self.config.owner_prefix.as_str()) {
                return Some((self.config.owner_prefix.as_str(), rest));
            }
        }

        text.strip_prefix(self.config.user_prefix.as_str())
            .map(|rest| (self.config.user_prefix.as_str(), rest))
    }
}

/// Splits `name args...` (first whitespace wins).
fn split_command(rest: &str) -> Option<(&str, &str)> {
    let rest = rest.trim_start();

    if rest.is_empty() {
        return None;
    }

    rest.split_once(char::is_whitespace)
        .map(|(name, args)| (name, args.trim()))
        .or(Some((rest, "")))
}