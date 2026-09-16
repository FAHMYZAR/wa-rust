use super::feature::{Category, CommandContext};
use super::registry::FeatureRegistry;
use crate::config::Config;
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

            // Only non-owners may never use the owner prefix, even for shared commands.
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

        if feature.owner_only() && !is_owner {
            return Ok(());
        }
        // Every command in the `group` category is reserved for the owner / the
        // bot account itself. Regular members may not run any of them — not
        // even the passive ones like `tagall` or `hidetag`.
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
    /// Returns (prefix_used, text_after_prefix). Owner prefix wins when the
    /// sender is the owner and the text starts with it; otherwise user prefix.
    fn split_prefix<'a>(&'a self, text: &'a str, is_owner: bool) -> Option<(&'a str, &'a str)> {
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
