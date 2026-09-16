use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::group;
use anyhow::Result;
use async_trait::async_trait;
use whatsapp_rust::prelude::*;

pub struct GroupInfoFeature;

#[async_trait]
impl Feature for GroupInfoFeature {
    fn name(&self) -> &'static str {
        "groupinfo"
    }

    fn description(&self) -> &'static str {
        "Lihat info lengkap grup"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["infogrup", "infogroup"]
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

        let meta = match group::get_metadata(&ctx.msg.client, ctx.group_jid()).await {
            Ok(m) => m,
            Err(_) => {
                ctx.reply("❌ Gagal menampilkan info grup!").await?;
                return Ok(());
            }
        };

        let admins: Vec<&whatsapp_rust::GroupParticipant> =
            meta.participants.iter().filter(|p| p.is_admin()).collect();
        let members: Vec<&whatsapp_rust::GroupParticipant> =
            meta.participants.iter().filter(|p| !p.is_admin()).collect();

        let formatted_date = meta
            .creation_time
            .map(format_creation_date)
            .unwrap_or_else(|| "-".to_string());

        let group_id = ctx.group_jid().user.clone();

        let mut message = String::from("📊 *INFO GRUP*\n\n");
        message.push_str(&format!("*Nama:* {}\n", meta.subject));
        message.push_str(&format!("*ID:* {}\n", group_id));
        message.push_str(&format!("*Dibuat:* {}\n", formatted_date));
        if let Some(owner) = &meta.creator {
            message.push_str(&format!("*Owner:* @{}\n\n", owner.user));
        } else {
            message.push('\n');
        }

        message.push_str(&format!(
            "*Deskripsi:*\n{}\n\n",
            meta.description.as_deref().unwrap_or("Tidak ada deskripsi")
        ));

        message.push_str("*Statistik:*\n");
        message.push_str(&format!("> Total Member: {}\n", meta.participants.len()));
        message.push_str(&format!("> Admin: {}\n", admins.len()));
        message.push_str(&format!("> Member: {}\n\n", members.len()));

        message.push_str("*Pengaturan:*\n");
        message.push_str(&format!(
            "> Kirim Pesan: {}\n",
            if meta.is_announcement {
                "Hanya Admin"
            } else {
                "Semua Member"
            }
        ));
        message.push_str(&format!(
            "> Edit Info: {}\n",
            if meta.is_locked {
                "Hanya Admin"
            } else {
                "Semua Member"
            }
        ));

        let mentions: Vec<Jid> = meta.creator.clone().into_iter().collect();
        ctx.send(group::text_with_mentions(&message, &mentions))
            .await
    }
}

fn format_creation_date(ts: u64) -> String {
    // Convert unix seconds to dd Month yyyy (id-ID) without chrono.
    const MONTHS: [&str; 12] = [
        "Januari",
        "Februari",
        "Maret",
        "April",
        "Mei",
        "Juni",
        "Juli",
        "Agustus",
        "September",
        "Oktober",
        "November",
        "Desember",
    ];
    let days = ts / 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    let month = MONTHS
        .get((m as usize).saturating_sub(1))
        .copied()
        .unwrap_or("");
    format!("{:02} {} {}", d, month, y)
}

/// Howard Hinnant's civil_from_days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
