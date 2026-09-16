use crate::core::feature::{Category, CommandContext, Feature};
use crate::core::registry::FeatureRegistry;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media::{self, ImageOptions};
use whatsapp_rust::upload::UploadOptions;

const HELP_BANNER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/disk/welcome-banner.jpg"
));

#[derive(Default)]
struct Session {
    category: Option<Category>,
    page: usize,
    last_help: Option<Instant>,
}

pub struct HelpFeature {
    registry: OnceLock<Weak<FeatureRegistry>>,
    owner_prefix: String,
    user_prefix: String,
    sessions: Mutex<HashMap<(String, String), Session>>,
}

impl HelpFeature {
    pub fn new(owner_prefix: String, user_prefix: String) -> Self {
        Self {
            registry: OnceLock::new(),
            owner_prefix,
            user_prefix,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_registry(&self, registry: Arc<FeatureRegistry>) {
        let _ = self.registry.set(Arc::downgrade(&registry));
    }

    fn render(&self, ctx: &CommandContext<'_>, registry: &FeatureRegistry) -> Option<String> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let key = (ctx.sender_jid().to_string(), ctx.group_jid().to_string());
        let action = ctx.args.strip_prefix('!');
        if matches!(action, Some("next" | "prev" | "back")) && !sessions.contains_key(&key) {
            return None;
        }
        let session = sessions.entry(key).or_default();
        match action {
            None => {
                if let Some(last) = session.last_help {
                    let remaining = Duration::from_secs(3).saturating_sub(last.elapsed());
                    if !remaining.is_zero() {
                        return Some(format!(
                            "⏳ Tunggu {} detik lagi...",
                            remaining.as_millis().div_ceil(1000)
                        ));
                    }
                }
                session.last_help = Some(Instant::now());
                session.category = None;
                session.page = 0;
            }
            Some("back") => {
                session.category = None;
                session.page = 0;
            }
            Some("next" | "prev") => {
                let category = session.category?;
                let count = visible_commands(registry, category).len();
                if action == Some("next") {
                    if (session.page + 1) * 10 >= count {
                        return None;
                    }
                    session.page += 1;
                } else {
                    if session.page == 0 {
                        return None;
                    }
                    session.page -= 1;
                }
            }
            Some(label) => {
                session.category = Some(
                    visible_categories(registry)
                        .into_iter()
                        .find(|c| c.label() == label)?,
                );
                session.page = 0;
            }
        }
        if let Some(category) = session.category {
            let commands = visible_commands(registry, category);
            if commands.is_empty() {
                return None;
            }
            let prefix = if ctx.is_owner {
                &self.owner_prefix
            } else {
                &self.user_prefix
            };
            Some(category_menu(
                category.label(),
                &commands,
                session.page,
                prefix,
            ))
        } else {
            // V8 heap and CPU percentages have no equivalent in the current Rust metrics.
            let mut text =
                "*MENU UTAMA*\n\n*Status:* Active\n*Memory:* N/A/N/A MB\n*CPU:* N/A% (".to_string();
            text.push_str(
                &std::thread::available_parallelism()
                    .map_or(1, usize::from)
                    .to_string(),
            );
            text.push_str(" cores)\n\n*KATEGORI FITUR:*\n\n");
            for category in visible_categories(registry) {
                let label = category.label();
                let name = format!("{}{}", label[..1].to_uppercase(), &label[1..]);
                text.push_str(&format!(
                    "▸ `!{label}` - {name} ({})\n",
                    visible_commands(registry, category).len()
                ));
            }
            text.push_str("\n_💡 Ketik Misal : `!tools` untuk masuk salah satu menu_");
            Some(text)
        }
    }
}

fn visible_categories(registry: &FeatureRegistry) -> Vec<Category> {
    registry
        .categories()
        .into_iter()
        .filter(|category| {
            registry
                .in_category(*category)
                .iter()
                .any(|feature| !feature.hidden())
        })
        .collect()
}

fn visible_commands(registry: &FeatureRegistry, category: Category) -> Vec<Arc<dyn Feature>> {
    registry
        .in_category(category)
        .into_iter()
        .filter(|feature| !feature.hidden())
        .collect()
}

fn category_menu(
    category: &str,
    commands: &[Arc<dyn Feature>],
    page: usize,
    prefix: &str,
) -> String {
    let pages = commands.len().div_ceil(10);
    let mut text = format!(
        "*{}*\n\n*Halaman {}/{}* | *Total: {}*\n\n",
        category.to_uppercase(),
        page + 1,
        pages,
        commands.len()
    );
    for (index, command) in commands.iter().enumerate().skip(page * 10).take(10) {
        text.push_str(&format!(
            "*{}.* `{prefix}{}`\n   {}\n\n",
            index + 1,
            command.name(),
            command.description()
        ));
    }
    text.push_str("*🔄 Navigasi:*\n");
    if page > 0 {
        text.push_str("▸ `!prev` - Halaman sebelumnya\n");
    }
    if page + 1 < pages {
        text.push_str("▸ `!next` - Halaman selanjutnya\n");
    }
    text.push_str("▸ `!back` - Kembali ke menu utama");
    text
}

#[async_trait]
impl Feature for HelpFeature {
    fn name(&self) -> &'static str {
        "help"
    }
    fn description(&self) -> &'static str {
        "Tampilkan menu bantuan"
    }
    fn category(&self) -> Category {
        Category::General
    }

    fn hidden(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let Some(registry) = self.registry.get().and_then(Weak::upgrade) else {
            return Ok(());
        };
        if let Some(text) = self.render(ctx, &registry) {
            if ctx.args.is_empty() {
                let upload = ctx
                    .msg
                    .client
                    .upload(
                        HELP_BANNER.to_vec(),
                        MediaType::Image,
                        UploadOptions::default(),
                    )
                    .await?;
                ctx.send(media::image_message(
                    upload,
                    ImageOptions {
                        caption: Some(text),
                        mimetype: Some("image/jpeg".into()),
                        ..Default::default()
                    },
                ))
                .await?;
            } else {
                ctx.reply(text).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::media::sticker::StickerFeature;

    #[test]
    fn pagination_has_ten_items_and_correct_navigation() {
        let commands: Vec<Arc<dyn Feature>> = (0..11)
            .map(|_| Arc::new(StickerFeature) as Arc<dyn Feature>)
            .collect();
        let first = category_menu("media", &commands, 0, ".");
        assert_eq!(first.matches("`.sticker`").count(), 10);
        assert!(first.contains("*Halaman 1/2* | *Total: 11*"));
        assert!(first.contains("`!next`"));
        assert!(!first.contains("`!prev`"));
        let last = category_menu("media", &commands, 1, "/");
        assert!(last.contains("*11.* `/sticker`"));
        assert!(last.contains("`!prev`"));
        assert!(!last.contains("`!next`"));
    }

    #[test]
    fn registry_does_not_form_a_strong_reference_cycle() {
        let registry = crate::features::build_registry();
        let weak = Arc::downgrade(&registry);
        drop(registry);
        assert!(weak.upgrade().is_none());
    }
}
