use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

/// Category shown by `help` and used for grouping commands.
/// Only `General` is used so far; the rest are wired as features are ported.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    General,
    Owner,
    Group,
    Media,
    Download,
    Ai,
    Utility,
    Fun,
    Religious,
    Academic,
}

impl Category {
    pub fn from_label(label: &str) -> Option<Self> {
        [
            Category::General,
            Category::Owner,
            Category::Group,
            Category::Media,
            Category::Download,
            Category::Ai,
            Category::Utility,
            Category::Fun,
            Category::Religious,
            Category::Academic,
        ]
        .into_iter()
        .find(|category| category.label() == label)
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::General => "info",
            Category::Owner => "owner",
            Category::Group => "group",
            Category::Media => "media",
            Category::Download => "download",
            Category::Ai => "ai",
            Category::Utility => "tools",
            Category::Fun => "fun",
            Category::Religious => "religious",
            Category::Academic => "akademik",
        }
    }
}

/// One bot command. Mirrors the JS `BaseFeature` (name/description/ownerOnly/execute)
/// as a trait: each feature is a struct implementing this.
#[async_trait]
pub trait Feature: Send + Sync {
    /// Command trigger without prefix, e.g. `ping`.
    fn name(&self) -> &'static str;
    /// One-line description for the help menu.
    fn description(&self) -> &'static str;
    /// Usage hint, e.g. `ping`. Shown in help.
    #[allow(dead_code)]
    fn usage(&self) -> &'static str {
        self.name()
    }
    fn category(&self) -> Category;
    /// Extra triggers mapping to this feature.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
    /// Only the owner may run this command.
    fn owner_only(&self) -> bool {
        false
    }
    /// Whether this command only makes sense inside a group.
    fn group_only(&self) -> bool {
        false
    }
    /// Hidden features stay dispatchable but are never listed in the help menu.
    fn hidden(&self) -> bool {
        false
    }
    /// Handler. `args` is the text after the command name.
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()>;
}

/// Everything a feature handler needs: the message, parsed args, and sender info.
/// `args`/`is_owner`/`is_group` are consumed by ported features (not yet all used).
#[allow(dead_code)]
pub struct CommandContext<'a> {
    /// Full message context from whatsapp-rust.
    pub msg: &'a MessageContext,
    /// Text after the command name, trimmed.
    pub args: &'a str,
    /// Whether the sender is the bot owner.
    pub is_owner: bool,
    /// Whether the chat is a group.
    pub is_group: bool,
}

#[allow(dead_code)]
impl<'a> CommandContext<'a> {
    /// Sender's user part (phone number for DMs), e.g. `628123...`.
    pub fn sender_user(&self) -> &str {
        &self.msg.info.source.sender.user
    }

    pub async fn reply(&self, text: impl Into<String>) -> Result<()> {
        self.msg.reply(text).await?;
        Ok(())
    }

    pub async fn reply_quoting(&self, text: impl Into<String>) -> Result<()> {
        self.msg.reply_quoting(text).await?;
        Ok(())
    }

    pub async fn react(&self, emoji: &str) -> Result<()> {
        self.msg.react(emoji).await?;
        Ok(())
    }

    /// Clear/remove the reaction emoji on the incoming message.
    pub async fn unreact(&self) -> Result<()> {
        self.msg.react("").await?;
        Ok(())
    }

    pub async fn send(&self, message: wa::Message) -> Result<()> {
        self.msg.send_message(message).await?;
        Ok(())
    }

    /// Group JID of the current chat.
    pub fn group_jid(&self) -> &Jid {
        &self.msg.info.source.chat
    }

    /// Sender's full JID.
    pub fn sender_jid(&self) -> &Jid {
        &self.msg.info.source.sender
    }
}
