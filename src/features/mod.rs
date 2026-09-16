pub mod general;
pub mod group;
pub mod media;

use crate::core::feature::Feature;
use crate::core::registry::FeatureRegistry;
use std::sync::Arc;

/// Builds the feature registry. Mirrors the JS auto-loader: new features are
/// added to the vec below. `help` is wired last so it can enumerate the rest.
pub fn build_registry() -> Arc<FeatureRegistry> {
    let owner_prefix = std::env::var("OWNER_PREFIX").unwrap_or_else(|_| "/".to_string());
    let user_prefix = std::env::var("USER_PREFIX").unwrap_or_else(|_| ".".to_string());

    let mut registry = FeatureRegistry::new();

    let features: Vec<Arc<dyn Feature>> = vec![
        Arc::new(general::ping::PingFeature),
        Arc::new(general::gempa::GempaFeature),
        Arc::new(general::tr::TranslateFeature),
        Arc::new(general::hug::HugFeature),
        Arc::new(general::logs::LogsFeature),
        Arc::new(group::tagall::TagallFeature),
        Arc::new(group::hidetag::HidetagFeature),
        Arc::new(group::grouplink::GrouplinkFeature),
        Arc::new(group::add::AddFeature),
        Arc::new(group::open::OpenFeature),
        Arc::new(group::close::CloseFeature),
        Arc::new(group::groupinfo::GroupInfoFeature),
        Arc::new(group::adminlist::AdminListFeature),
        Arc::new(group::resetlink::ResetLinkFeature),
        Arc::new(group::getjid::GetJidFeature),
        Arc::new(group::kick::KickFeature),
        Arc::new(group::promote::PromoteFeature),
        Arc::new(group::demote::DemoteFeature),
        Arc::new(group::warn::WarnFeature),
        Arc::new(group::unwarn::UnwarnFeature),
        Arc::new(group::warnlist::WarnlistFeature),
        Arc::new(media::tourl::TourlFeature),
        Arc::new(media::rvo::RvoFeature),
        Arc::new(media::ocr::OcrFeature),
        Arc::new(media::sticker::StickerFeature),
        Arc::new(media::toimg::ToimgFeature),
        Arc::new(media::smeme::SmemeFeature),
        Arc::new(media::brat::BratFeature),
        Arc::new(media::bratvid::BratvidFeature),
        Arc::new(media::q::QFeature),
        Arc::new(media::triger::TrigerFeature),
        Arc::new(media::hdsw::HdswFeature),
        Arc::new(media::remini::ReminiFeature::default()),
        Arc::new(media::rmbg::RmbgFeature),
    ];
    for feature in features {
        registry.register(feature);
    }

    let help = Arc::new(general::help::HelpFeature::new(owner_prefix, user_prefix));

    let mut with_help = FeatureRegistry::new();
    for feature in registry
        .categories()
        .into_iter()
        .flat_map(|c| registry.in_category(c))
    {
        with_help.register(feature);
    }
    with_help.register(Arc::clone(&help) as Arc<dyn Feature>);

    let with_help = Arc::new(with_help);
    help.set_registry(Arc::clone(&with_help));
    with_help
}
