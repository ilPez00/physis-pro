use std::collections::HashSet;

static FUNCTION_WORDS: once_cell::sync::Lazy<HashSet<&'static str>> =
    once_cell::sync::Lazy::new(|| {
        let mut s = HashSet::new();
        s.extend(["a", "an", "the"]);
        s.extend([
            "in", "on", "at", "to", "for", "of", "with", "by", "from", "up",
            "about", "into", "over", "after", "before", "between", "under",
            "through", "during", "without", "within", "along", "across",
            "behind", "below", "beneath", "beside", "beyond", "around",
            "toward", "upon", "via",
        ]);
        s.extend([
            "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "having",
            "do", "does", "did", "doing",
            "will", "would", "shall", "should",
            "can", "could", "may", "might", "must",
            "am",
        ]);
        s.extend([
            "i", "you", "he", "she", "it", "we", "they",
            "me", "him", "her", "us", "them",
            "my", "your", "his", "its", "our", "their",
            "mine", "yours", "hers", "ours", "theirs",
            "this", "that", "these", "those",
            "who", "whom", "what", "which",
            "myself", "yourself", "himself", "herself", "itself",
            "ourselves", "themselves",
        ]);
        s.extend([
            "and", "or", "but", "nor", "yet", "so",
            "because", "although", "while", "if", "unless",
            "since", "as", "when", "where", "whether",
        ]);
        s.extend([
            "some", "any", "each", "every", "all", "both", "few", "many",
            "much", "several", "no", "not", "neither", "either",
        ]);
        s.extend([
            "then", "just", "very", "also", "too", "only", "quite",
            "here", "there", "now", "then", "still", "already",
            "yet", "again", "ever", "never", "always",
        ]);
        s.extend([
            "that", "which", "who", "whom", "whose",
            "where", "when", "why", "how",
        ]);
        s
    });

#[derive(Debug, Clone)]
pub struct PirahaFilter {
    stop_words: HashSet<String>,
}

impl PirahaFilter {
    pub fn new() -> Self {
        Self {
            stop_words: FUNCTION_WORDS
                .iter()
                .map(|w| w.to_string())
                .collect(),
        }
    }

    pub fn filter(&self, text: &str) -> String {
        let text = text.trim();
        if text.is_empty() {
            return String::new();
        }

        let mut lines: Vec<String> = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let compressed = self.compress_line(line);
            if !compressed.is_empty() {
                lines.push(compressed);
            }
        }

        if lines.len() == 1 {
            lines.into_iter().next().unwrap_or_default()
        } else {
            lines.join(" | ")
        }
    }

    fn compress_line(&self, line: &str) -> String {
        let mut tokens: Vec<String> = Vec::new();

        let mut parts: Vec<&str> = Vec::new();
        for token in line.split_whitespace() {
            if token.contains('→') {
                for sub in token.split("→") {
                    let s = sub.trim();
                    if !s.is_empty() {
                        parts.push(s);
                    }
                }
            } else if token.contains('/') {
                for sub in token.split('/') {
                    let s = sub.trim();
                    if !s.is_empty() {
                        parts.push(s);
                    }
                }
            } else {
                parts.push(token);
            }
        }

        for token in parts {
            let cleaned = token.trim_matches(|c: char| ".,;:!?\"'()[]{}<>".contains(c));
            if cleaned.is_empty() {
                continue;
            }

            let lower = cleaned.to_lowercase();
            if self.stop_words.contains(&lower) {
                continue;
            }

            let numeric = cleaned.parse::<f64>().ok();
            if let Some(n) = numeric {
                if n == 0.0 || n == 1.0 {
                    tokens.push(cleaned.to_string());
                } else if n.fract() == 0.0 {
                    tokens.push(format!("{}", n as i64));
                } else {
                    tokens.push(format!("{:.2}", n));
                }
                continue;
            }

            tokens.push(cleaned.to_string());
        }

        tokens.join(" ")
    }

    pub fn log_event(&self, event_type: &str, data: &str, grade: Option<f32>) -> String {
        let data_c = self.filter(data);
        match grade {
            Some(g) => format!("[{}] {} G{:.0}", event_type, data_c, g * 100.0),
            None => format!("[{}] {}", event_type, data_c),
        }
    }

    pub fn log_goal(&self, name: &str, progress: f32) -> String {
        let name_c = self.filter(name);
        format!("GOAL {} P{:.0}", name_c, progress * 100.0)
    }

    pub fn log_experience(&self, action: &str, grade: f32, rationale: &str) -> String {
        let action_c = self.filter(action);
        let rationale_c = self.filter(rationale);
        let g = (grade * 100.0) as u32;
        format!("ACT {} G{} REASON {}", action_c, g, rationale_c)
    }
}

impl Default for PirahaFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piraha_basic() {
        let f = PirahaFilter::new();
        let result = f.filter("I went for a run in the morning");
        assert_eq!(result, "went run morning");
    }

    #[test]
    fn test_piraha_empty() {
        let f = PirahaFilter::new();
        assert_eq!(f.filter(""), "");
        assert_eq!(f.filter("  "), "");
    }

    #[test]
    fn test_piraha_strips_all_function_words() {
        let f = PirahaFilter::new();
        let result = f.filter("the cat and the dog are on the table");
        assert_eq!(result, "cat dog table");
    }

    #[test]
    fn test_piraha_preserves_numerics() {
        let f = PirahaFilter::new();
        let result = f.filter("ran 5 kilometers with a grade of 0.8");
        assert_eq!(result, "ran 5 kilometers grade 0.80");
    }

    #[test]
    fn test_piraha_log_event() {
        let f = PirahaFilter::new();
        let result = f.log_event("SCAN", "found the file", None);
        assert_eq!(result, "[SCAN] found file");
    }

    #[test]
    fn test_piraha_log_event_with_grade() {
        let f = PirahaFilter::new();
        let result = f.log_event("DREAM", "new mutation", Some(0.75));
        assert_eq!(result, "[DREAM] new mutation G75");
    }

    #[test]
    fn test_piraha_log_goal() {
        let f = PirahaFilter::new();
        let result = f.log_goal("morning run exercise", 0.5);
        assert_eq!(result, "GOAL morning run exercise P50");
    }

    #[test]
    fn test_piraha_log_experience() {
        let f = PirahaFilter::new();
        let result = f.log_experience("ran 5k", 0.8, "it was a good run");
        assert_eq!(result, "ACT ran 5k G80 REASON good run");
    }

    #[test]
    fn test_piraha_path_separators() {
        let f = PirahaFilter::new();
        let result = f.filter("project → ai → memory");
        assert_eq!(result, "project ai memory");
    }

    #[test]
    fn test_piraha_multiline() {
        let f = PirahaFilter::new();
        let result = f.filter("hello world\nthe cat sat");
        assert_eq!(result, "hello world | cat sat");
    }
}
