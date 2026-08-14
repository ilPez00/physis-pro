//! Linguistic lenses — Wenyan (kanji compression), Piraha (filler-stripping), Sanskrit (poetic expansion).

pub mod piraha;
pub mod sanskrit;
pub mod wenyan;

use std::collections::HashMap;

use crate::config::LinguisticConfig;

/// Which linguistic transformation to apply to text data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinguisticLense {
    /// Poetic expansion (Sanskrit mode).
    Sanskrit,
    /// Filler-stripping minimalism (Piraha mode).
    Piraha,
    /// Kanji-heavy compression (Wenyan mode).
    Wenyan,
}

impl LinguisticLense {
    /// Return the uppercase string label for this lense variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sanskrit => "SANSKRIT",
            Self::Piraha => "PIRAHA",
            Self::Wenyan => "WENYAN",
        }
    }
}

/// Routes text through one or all linguistic lenses.
#[derive(Debug, Clone)]
pub struct LinguisticRouter {
    /// Wenyan filter (kanji compression).
    pub wenyan: wenyan::WenyanFilter,
    /// Piraha filter (filler stripping).
    pub piraha: piraha::PirahaFilter,
    /// Sanskrit engine (poetic expansion).
    pub sanskrit: sanskrit::SanskritEngine,
    /// Which lenses are enabled at runtime.
    enabled: LinguisticConfig,
}

impl LinguisticRouter {
    /// Create a new router with default filters for all three lenses.
    pub fn new() -> Self {
        Self {
            wenyan: wenyan::WenyanFilter::new(),
            piraha: piraha::PirahaFilter::new(),
            sanskrit: sanskrit::SanskritEngine::new(),
            enabled: LinguisticConfig::default(),
        }
    }

    /// Create a router that honours the enabled/disabled flags in `config`.
    /// Disabled lenses return the input text unchanged.
    pub fn with_config(config: &LinguisticConfig) -> Self {
        Self {
            wenyan: wenyan::WenyanFilter::new(),
            piraha: piraha::PirahaFilter::new(),
            sanskrit: sanskrit::SanskritEngine::new(),
            enabled: config.clone(),
        }
    }

    /// Whether a given lense is active.
    pub fn is_enabled(&self, lense: LinguisticLense) -> bool {
        match lense {
            LinguisticLense::Wenyan => self.enabled.wenyan_enabled,
            LinguisticLense::Piraha => self.enabled.piraha_enabled,
            LinguisticLense::Sanskrit => self.enabled.sanskrit_enabled,
        }
    }

    /// Transform `raw_data` through the specified lense.
    /// Returns the input unchanged if the lense is disabled.
    pub fn route(&self, raw_data: &str, lense: LinguisticLense) -> String {
        if !self.is_enabled(lense) {
            return raw_data.to_string();
        }
        match lense {
            LinguisticLense::Piraha => self.piraha.filter(raw_data),
            LinguisticLense::Wenyan => self.wenyan.compress(raw_data),
            LinguisticLense::Sanskrit => self.sanskrit.dream(raw_data),
        }
    }

    /// Return the list of enabled lense variants.
    pub fn enabled_lenses(&self) -> Vec<LinguisticLense> {
        let mut v = Vec::new();
        if self.is_enabled(LinguisticLense::Wenyan) {
            v.push(LinguisticLense::Wenyan);
        }
        if self.is_enabled(LinguisticLense::Piraha) {
            v.push(LinguisticLense::Piraha);
        }
        if self.is_enabled(LinguisticLense::Sanskrit) {
            v.push(LinguisticLense::Sanskrit);
        }
        v
    }

    /// Apply all enabled lenses and return a map from lense to result.
    /// Disabled lenses are omitted from the map.
    pub fn route_all(&self, raw_data: &str) -> HashMap<LinguisticLense, String> {
        let mut results = HashMap::new();
        for lense in self.enabled_lenses() {
            results.insert(lense, self.route(raw_data, lense));
        }
        results
    }
}

impl Default for LinguisticRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_all_lenses() {
        let router = LinguisticRouter::new();
        let data = "ran 5k exercise";
        for lense in &[LinguisticLense::Wenyan, LinguisticLense::Piraha, LinguisticLense::Sanskrit] {
            let output = router.route(data, *lense);
            assert!(!output.is_empty(), "{} should produce output", lense.as_str());
        }
    }

    #[test]
    fn test_router_all_returns_map() {
        let router = LinguisticRouter::new();
        let data = "morning run code";
        let results = router.route_all(data);
        assert_eq!(results.len(), 3);
        assert!(results.contains_key(&LinguisticLense::Wenyan));
        assert!(results.contains_key(&LinguisticLense::Piraha));
        assert!(results.contains_key(&LinguisticLense::Sanskrit));
    }

    #[test]
    fn test_router_wenyan_produces_kanji() {
        let router = LinguisticRouter::new();
        let result = router.route("run body health", LinguisticLense::Wenyan);
        assert!(result.contains('跑') || result.contains('身') || result.contains('康'));
    }

    #[test]
    fn test_router_piraha_strips_fillers() {
        let router = LinguisticRouter::new();
        let result = router.route("I am running the code", LinguisticLense::Piraha);
        assert!(!result.contains("I"));
        assert!(!result.contains("the"));
        assert!(result.contains("running"));
        assert!(result.contains("code"));
    }

    #[test]
    fn test_router_sanskrit_expands() {
        let router = LinguisticRouter::new();
        let result = router.route("run code", LinguisticLense::Sanskrit);
        assert!(result.contains("blossoms into"));
    }

    #[test]
    fn test_lense_as_str() {
        assert_eq!(LinguisticLense::Wenyan.as_str(), "WENYAN");
        assert_eq!(LinguisticLense::Piraha.as_str(), "PIRAHA");
        assert_eq!(LinguisticLense::Sanskrit.as_str(), "SANSKRIT");
    }

    #[test]
    fn test_router_empty_input() {
        let router = LinguisticRouter::new();
        assert_eq!(router.route("", LinguisticLense::Wenyan), "");
        assert_eq!(router.route("", LinguisticLense::Piraha), "");
        assert_eq!(router.route("", LinguisticLense::Sanskrit), "");
    }

    #[test]
    fn test_router_disabled_lense_returns_input() {
        let cfg = LinguisticConfig {
            wenyan_enabled: false,
            ..LinguisticConfig::default()
        };
        let router = LinguisticRouter::with_config(&cfg);
        assert_eq!(router.route("hello world", LinguisticLense::Wenyan), "hello world");
    }

    #[test]
    fn test_router_all_skips_disabled() {
        let cfg = LinguisticConfig {
            piraha_enabled: false,
            sanskrit_enabled: false,
            ..LinguisticConfig::default()
        };
        let router = LinguisticRouter::with_config(&cfg);
        let results = router.route_all("some text");
        assert_eq!(results.len(), 1);
        assert!(results.contains_key(&LinguisticLense::Wenyan));
    }

    #[test]
    fn test_router_all_enabled_three_lenses() {
        let cfg = LinguisticConfig::default();
        let router = LinguisticRouter::with_config(&cfg);
        let results = router.route_all("test");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_router_all_disabled_empty_map() {
        let cfg = LinguisticConfig {
            wenyan_enabled: false,
            piraha_enabled: false,
            sanskrit_enabled: false,
        };
        let router = LinguisticRouter::with_config(&cfg);
        let results = router.route_all("test");
        assert!(results.is_empty());
    }
}
