use std::collections::HashMap;

static SEMANTIC_NET: once_cell::sync::Lazy<HashMap<&'static str, Vec<&'static str>>> =
    once_cell::sync::Lazy::new(|| {
        let mut m: HashMap<&str, Vec<&str>> = HashMap::new();
        m.insert("run", vec!["sprint", "jog", "move", "flow", "circulate"]);
        m.insert("walk", vec!["stroll", "wander", "journey", "pace"]);
        m.insert("exercise", vec!["train", "strengthen", "endure", "discipline"]);
        m.insert("fitness", vec!["vitality", "stamina", "vigor", "capacity"]);
        m.insert("body", vec!["form", "vessel", "temple", "physique"]);
        m.insert("health", vec!["wholeness", "balance", "harmony", "wellbeing"]);
        m.insert("eat", vec!["nourish", "consume", "sustain", "fuel"]);
        m.insert("sleep", vec!["restore", "recharge", "dream", "deep_rest"]);
        m.insert("rest", vec!["pause", "recover", "stillness", "restoration"]);
        m.insert("work", vec!["labor", "create", "produce", "contribute"]);
        m.insert("create", vec!["manifest", "generate", "originate", "craft"]);
        m.insert("build", vec!["construct", "assemble", "erect", "forge"]);
        m.insert("code", vec!["logic", "structure", "language", "instruction"]);
        m.insert("write", vec!["express", "compose", "inscribe", "articulate"]);
        m.insert("read", vec!["absorb", "interpret", "comprehend", "study"]);
        m.insert("learn", vec!["acquire", "understand", "master", "integrate"]);
        m.insert("think", vec!["contemplate", "reflect", "ponder", "reason"]);
        m.insert("meditate", vec!["stillness", "awareness", "presence", "contemplation"]);
        m.insert("mind", vec!["consciousness", "awareness", "intellect", "psyche"]);
        m.insert("energy", vec!["force", "vitality", "power", "dynamism"]);
        m.insert("money", vec!["resource", "capital", "value", "exchange"]);
        m.insert("friend", vec!["ally", "companion", "connection", "bond"]);
        m.insert("family", vec!["lineage", "kin", "ancestry", "heritage"]);
        m.insert("love", vec!["devotion", "affection", "connection", "care"]);
        m.insert("art", vec!["expression", "beauty", "creation", "aesthetics"]);
        m.insert("music", vec!["rhythm", "harmony", "melody", "sound"]);
        m.insert("project", vec!["endeavor", "initiative", "undertaking", "quest"]);
        m.insert("goal", vec!["aim", "objective", "aspiration", "milestone"]);
        m.insert("progress", vec!["advance", "growth", "development", "evolution"]);
        m.insert("dream", vec!["vision", "aspiration", "imagination", "possibility"]);
        m.insert("change", vec!["transform", "evolve", "shift", "metamorphosis"]);
        m.insert("grow", vec!["expand", "develop", "bloom", "flourish"]);
        m.insert("system", vec!["framework", "structure", "order", "pattern"]);
        m.insert("time", vec!["duration", "cycle", "moment", "eternity"]);
        m.insert("morning", vec!["dawn", "beginning", "awakening", "freshness"]);
        m.insert("night", vec!["darkness", "stillness", "mystery", "rest"]);
        m
    });

#[allow(dead_code)]
static ASSOCIATION_WORDS: &[&str] = &[
    "thus", "therefore", "moreover", "likewise", "furthermore",
    "indeed", "surely", "verily", "hence", "thereby",
];

#[derive(Debug, Clone)]
pub struct SanskritEngine {
    dream_log: Vec<String>,
}

impl SanskritEngine {
    pub fn new() -> Self {
        Self {
            dream_log: Vec::new(),
        }
    }

    pub fn dream(&self, text: &str) -> String {
        let text = text.trim();
        if text.is_empty() {
            return String::new();
        }

        let words: Vec<&str> = text
            .split_whitespace()
            .flat_map(|w| w.split(|c: char| ".,;:!?\"'()[]{}→".contains(c)))
            .filter(|w| !w.is_empty())
            .collect();

        let mut expansions: Vec<String> = Vec::new();
        let mut seen_core = Vec::new();

        for word in &words {
            let lower = word.to_lowercase();
            if let Some(associations) = SEMANTIC_NET.get(lower.as_str()) {
                if !seen_core.contains(&lower) {
                    seen_core.push(lower.clone());
                    let assoc_str = associations.join(", ");
                    expansions.push(format!(
                        "{} blossoms into {}",
                        word,
                        assoc_str
                    ));
                }
            }
        }

        if expansions.is_empty() {
            let mut echoed = words.to_vec();
            echoed.retain(|w| w.len() > 2);
            if echoed.is_empty() {
                return format!("⋮ {} ⋮", text);
            }
            let mut line = String::from("⋮ ");
            line.push_str(&echoed.join(" · "));
            line.push_str(" ⋮");
            return line;
        }

        let body = expansions.join("; ");
        let seed = seen_core.join(" → ");

        format!(
            "~ dreaming of {} ~\n  {}\n  ~ thus the seed '{}' yearns for form ~",
            words.iter().take(3).cloned().collect::<Vec<_>>().join(" "),
            body,
            seed,
        )
    }

    pub fn dream_on_domain(&self, domain: &str, progress: f32) -> String {
        let domain_dream = self.dream(domain);
        let pct = (progress * 100.0) as u32;
        format!(
            "{}\n  ~ progress {}% calls for deeper inquiry ~",
            domain_dream, pct
        )
    }

    pub fn dream_on_experience(&self, action: &str, grade: f32, rationale: &str) -> String {
        let action_dream = self.dream(action);
        let rationale_dream = self.dream(rationale);
        let g = (grade * 100.0) as u32;
        format!(
            "{}\n  ~ rated {}% ~\n  ~ rationale echoes: {} ~",
            action_dream, g, rationale_dream
        )
    }

    pub fn record_dream(&mut self, dream_text: String) {
        self.dream_log.push(dream_text);
    }

    pub fn dream_log(&self) -> &[String] {
        &self.dream_log
    }

    pub fn synthesize(&self, entries: &[(&str, f32)]) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push("~ synthesis of all dreaming ~".to_string());

        for (domain, progress) in entries {
            let dream = self.dream_on_domain(domain, *progress);
            lines.push(dream);
        }

        lines.push("~ all seeds tend toward their flowering ~".to_string());
        lines.join("\n\n")
    }
}

impl Default for SanskritEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanskrit_basic_expansion() {
        let e = SanskritEngine::new();
        let result = e.dream("run");
        assert!(result.contains("run blossoms into"));
        assert!(result.contains("sprint"));
        assert!(result.contains("yearns for form"));
    }

    #[test]
    fn test_sanskrit_empty() {
        let e = SanskritEngine::new();
        assert_eq!(e.dream(""), "");
        assert_eq!(e.dream("  "), "");
    }

    #[test]
    fn test_sanskrit_unknown_words() {
        let e = SanskritEngine::new();
        let result = e.dream("xyz quantum");
        assert!(result.starts_with("⋮"));
        assert!(result.contains("quantum"));
    }

    #[test]
    fn test_sanskrit_multiple_expansions() {
        let e = SanskritEngine::new();
        let result = e.dream("run code");
        assert!(result.contains("run blossoms into"));
        assert!(result.contains("code blossoms into"));
    }

    #[test]
    fn test_sanskrit_no_duplicate_seeds() {
        let e = SanskritEngine::new();
        let result = e.dream("run run run");
        let count = result.matches("run blossoms").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_sanskrit_domain_dream() {
        let e = SanskritEngine::new();
        let result = e.dream_on_domain("fitness", 0.5);
        assert!(result.contains("fitness blossoms into"));
        assert!(result.contains("progress 50%"));
    }

    #[test]
    fn test_sanskrit_experience_dream() {
        let e = SanskritEngine::new();
        let result = e.dream_on_experience("ran 5k", 0.8, "cardio training");
        assert!(result.contains("rated 80%"));
    }

    #[test]
    fn test_sanskrit_synthesize() {
        let e = SanskritEngine::new();
        let result = e.synthesize(&[("run", 0.3), ("code", 0.9)]);
        assert!(result.contains("synthesis of all dreaming"));
        assert!(result.contains("run"));
        assert!(result.contains("code"));
        assert!(result.contains("progress 30%"));
        assert!(result.contains("progress 90%"));
    }

    #[test]
    fn test_sanskrit_dream_log() {
        let mut e = SanskritEngine::new();
        e.record_dream("first dream".to_string());
        e.record_dream("second dream".to_string());
        assert_eq!(e.dream_log().len(), 2);
    }

    #[test]
    fn test_sanskrit_short_input_echoed() {
        let e = SanskritEngine::new();
        let result = e.dream("on");
        assert_eq!(result, "⋮ on ⋮");
    }
}
