use super::feature::{Category, Feature};
use std::collections::HashMap;
use std::sync::Arc;

/// Holds every registered feature, keyed by name and aliases.
/// Mirrors the JS `FeatureRegistry` auto-loader.
#[derive(Default)]
pub struct FeatureRegistry {
    by_name: HashMap<&'static str, Arc<dyn Feature>>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, feature: Arc<dyn Feature>) {
        self.by_name.insert(feature.name(), Arc::clone(&feature));
        for alias in feature.aliases() {
            self.by_name.insert(alias, Arc::clone(&feature));
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Feature>> {
        self.by_name.get(name).map(Arc::clone)
    }

    /// Unique features (deduplicated across aliases) in a category.
    pub fn in_category(&self, category: Category) -> Vec<Arc<dyn Feature>> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for feature in self.by_name.values() {
            if feature.category() == category && seen.insert(feature.name()) {
                out.push(Arc::clone(feature));
            }
        }
        out.sort_by_key(|f| f.name());
        out
    }

    pub fn categories(&self) -> Vec<Category> {
        let mut cats = std::collections::HashSet::new();
        for feature in self.by_name.values() {
            cats.insert(feature.category());
        }
        let mut cats: Vec<Category> = cats.into_iter().collect();
        cats.sort_by_key(|c| c.label());
        cats
    }
}
