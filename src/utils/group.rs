use anyhow::{bail, Context as _, Result};
use whatsapp_rust::prelude::*;
use whatsapp_rust::GroupMetadata;

/// Fetch group metadata with participants.
pub async fn get_metadata(client: &Client, group_jid: &Jid) -> Result<GroupMetadata> {
    client
        .groups()
        .get_metadata(group_jid)
        .await
        .map_err(|e| anyhow::anyhow!("failed to get group metadata: {e}"))
}

/// Returns true if the given user is an admin or superadmin in the group.
/// Matches both the participant JID and the backfilled phone number so LID
/// groups resolve correctly.
pub async fn is_admin(client: &Client, group_jid: &Jid, user_jid: &Jid) -> Result<bool> {
    let meta = get_metadata(client, group_jid).await?;
    Ok(is_admin_from_meta(&meta, user_jid))
}

/// Check admin role from already-fetched metadata (avoids a second query).
pub fn is_admin_from_meta(meta: &GroupMetadata, user_jid: &Jid) -> bool {
    let target = &user_jid.user;
    meta.participants.iter().any(|p| {
        p.is_admin()
            && (p.jid.user == *target
                || p.phone_number.as_ref().is_some_and(|pn| pn.user == *target))
    })
}

/// Returns true if the bot itself is an admin in the group.
pub async fn is_bot_admin(client: &Client, group_jid: &Jid) -> Result<bool> {
    let own = client.pn().context("bot not paired yet")?;
    is_admin(client, group_jid, &own).await
}

/// Require sender AND bot to be admins, replying with a reason otherwise.
/// Returns true when the caller may proceed.
pub async fn require_admins(ctx: &crate::core::feature::CommandContext<'_>) -> Result<bool> {
    if !ctx.is_group {
        ctx.reply("perintah ini hanya bisa dipakai di dalam group.")
            .await?;
        return Ok(false);
    }
    if !is_admin(&ctx.msg.client, ctx.group_jid(), ctx.sender_jid()).await? {
        ctx.reply("kamu harus admin group untuk memakai perintah ini.")
            .await?;
        return Ok(false);
    }
    if !is_bot_admin(&ctx.msg.client, ctx.group_jid()).await? {
        ctx.reply("bot harus menjadi admin group untuk menjalankan perintah ini.")
            .await?;
        return Ok(false);
    }
    Ok(true)
}

/// Extract ContextInfo from any message type that carries it.
pub fn context_info(msg: &wa::Message) -> Option<&wa::ContextInfo> {
    if let Some(m) = msg.extended_text_message.as_option() {
        if let Some(ci) = m.context_info.as_option() {
            return Some(ci);
        }
    }
    if let Some(m) = msg.image_message.as_option() {
        if let Some(ci) = m.context_info.as_option() {
            return Some(ci);
        }
    }
    if let Some(m) = msg.video_message.as_option() {
        if let Some(ci) = m.context_info.as_option() {
            return Some(ci);
        }
    }
    if let Some(m) = msg.document_message.as_option() {
        if let Some(ci) = m.context_info.as_option() {
            return Some(ci);
        }
    }
    if let Some(m) = msg.sticker_message.as_option() {
        if let Some(ci) = m.context_info.as_option() {
            return Some(ci);
        }
    }
    None
}

/// Build a message that @-mentions the given JIDs.
pub fn text_with_mentions(text: &str, mentioned: &[Jid]) -> wa::Message {
    wa::Message::text_with_context(
        text,
        wa::ContextInfo {
            mentioned_jid: mentioned.iter().map(|j| j.to_string()).collect(),
            ..Default::default()
        },
    )
}

/// Build a hidden-tag message: mentions everyone but shows no visible @.
pub fn hidetag_message(text: &str, mentioned: &[Jid]) -> wa::Message {
    // Zero-width separator repeated to hide visible @ tokens
    let hidden = "\u{200b}".repeat(mentioned.len());
    let body = if text.is_empty() {
        hidden
    } else {
        format!("{hidden} {text}")
    };
    text_with_mentions(&body, mentioned)
}

/// Resolve a target user JID from (1) the quoted message's sender,
/// (2) an explicit phone number argument, or (3) the first mention.
pub fn resolve_target_jid(ctx: &crate::core::feature::CommandContext<'_>) -> Result<Jid> {
    if let Some(ci) = context_info(&ctx.msg.message) {
        if let Some(participant) = ci.participant.as_deref() {
            if !participant.trim().is_empty() {
                if let Ok(jid) = participant.parse::<Jid>() {
                    return Ok(jid);
                }
            }
        }
        if let Some(first) = ci.mentioned_jid.first() {
            if let Ok(jid) = first.parse::<Jid>() {
                return Ok(jid);
            }
        }
    }

    let digits: String = ctx.args.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 8 {
        return Ok(Jid::pn(digits));
    }

    bail!("target tidak ditemukan — reply pesan seseorang atau tulis nomornya (contoh: 62812...)")
}
