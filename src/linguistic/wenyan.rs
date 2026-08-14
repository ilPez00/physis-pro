use std::collections::HashMap;

static WENYAN_MAP: once_cell::sync::Lazy<HashMap<&'static str, &'static str>> =
    once_cell::sync::Lazy::new(|| {
        let mut m = HashMap::new();
        m.insert("run", "跑");
        m.insert("walk", "行");
        m.insert("exercise", "練");
        m.insert("fitness", "健");
        m.insert("body", "身");
        m.insert("health", "康");
        m.insert("eat", "食");
        m.insert("food", "食");
        m.insert("sleep", "眠");
        m.insert("rest", "息");
        m.insert("work", "工");
        m.insert("create", "造");
        m.insert("build", "建");
        m.insert("code", "碼");
        m.insert("write", "書");
        m.insert("read", "讀");
        m.insert("learn", "學");
        m.insert("study", "研");
        m.insert("think", "思");
        m.insert("meditate", "禪");
        m.insert("mind", "心");
        m.insert("spirit", "神");
        m.insert("energy", "氣");
        m.insert("money", "財");
        m.insert("economic", "經");
        m.insert("finance", "金");
        m.insert("social", "交");
        m.insert("friend", "友");
        m.insert("family", "家");
        m.insert("love", "愛");
        m.insert("art", "藝");
        m.insert("music", "樂");
        m.insert("project", "項");
        m.insert("goal", "標");
        m.insert("progress", "進");
        m.insert("grade", "評");
        m.insert("score", "分");
        m.insert("time", "時");
        m.insert("day", "日");
        m.insert("week", "週");
        m.insert("month", "月");
        m.insert("year", "年");
        m.insert("morning", "晨");
        m.insert("night", "夜");
        m.insert("high", "高");
        m.insert("medium", "中");
        m.insert("low", "低");
        m.insert("start", "始");
        m.insert("end", "終");
        m.insert("success", "成");
        m.insert("fail", "敗");
        m.insert("test", "測");
        m.insert("fix", "修");
        m.insert("plan", "計");
        m.insert("do", "行");
        m.insert("check", "檢");
        m.insert("act", "為");
        m.insert("dream", "夢");
        m.insert("thought", "念");
        m.insert("idea", "想");
        m.insert("system", "系");
        m.insert("machine", "機");
        m.insert("human", "人");
        m.insert("structure", "構");
        m.insert("change", "變");
        m.insert("grow", "長");
        m.insert("reduce", "減");
        m.insert("increase", "增");
        m.insert("track", "追");
        m.insert("measure", "量");
        m.insert("analyze", "析");
        m
    });

#[derive(Debug, Clone)]
pub struct WenyanFilter {
    domain_prefixes: HashMap<String, &'static str>,
}

impl WenyanFilter {
    pub fn new() -> Self {
        let mut dp = HashMap::new();
        dp.insert("body & fitness".to_string(), "身健");
        dp.insert("body".to_string(), "身");
        dp.insert("fitness".to_string(), "健");
        dp.insert("health".to_string(), "康");
        dp.insert("intellectual".to_string(), "智");
        dp.insert("mind".to_string(), "心");
        dp.insert("economic".to_string(), "經");
        dp.insert("finance".to_string(), "財");
        dp.insert("psychological".to_string(), "情");
        dp.insert("social".to_string(), "交");
        dp.insert("spiritual".to_string(), "靈");
        dp.insert("operational".to_string(), "運");
        dp.insert("structural".to_string(), "構");
        dp.insert("informational".to_string(), "訊");
        dp.insert("energetic".to_string(), "能");
        Self { domain_prefixes: dp }
    }

    pub fn compress(&self, text: &str) -> String {
        let text = text.trim();
        if text.is_empty() {
            return String::new();
        }

        let mut parts: Vec<String> = Vec::new();

        for word in text.split_whitespace() {
            let cleaned = word.trim_matches(|c: char| ".,;:!?\"'()[]{}<>→".contains(c));
            if cleaned.is_empty() {
                continue;
            }
            let lower = cleaned.to_lowercase();
            if let Some(wen) = WENYAN_MAP.get(lower.as_str()) {
                parts.push(wen.to_string());
            } else {
                let transliterated = self.transliterate(cleaned);
                parts.push(transliterated);
            }
        }

        parts.join("/")
    }

    pub fn compress_domain(&self, text: &str) -> String {
        let lower = text.to_lowercase();
        let mut keys: Vec<&String> = self.domain_prefixes.keys().collect();
        keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
        for key in keys {
            if lower.contains(key.as_str()) {
                return self.domain_prefixes.get(key.as_str()).unwrap().to_string();
            }
        }
        if let Some(wen) = WENYAN_MAP.get(lower.as_str()) {
            return wen.to_string();
        }
        self.transliterate(text)
    }

    pub fn compress_goal(&self, name: &str, domain: &str, progress: f32) -> String {
        let name_c = self.compress(name);
        let domain_c = self.compress_domain(domain);
        let pct = (progress * 100.0) as u32;
        format!("{}/{}·{}%", domain_c, name_c, pct)
    }

    pub fn compress_experience(&self, action: &str, grade: f32, rationale: &str) -> String {
        let action_c = self.compress(action);
        let rationale_c = self.compress(rationale);
        let g = (grade * 100.0) as u32;
        if rationale_c.is_empty() {
            format!("行:{}·評{}", action_c, g)
        } else {
            format!("行:{}·評{}·由:{}", action_c, g, rationale_c)
        }
    }

    fn transliterate(&self, word: &str) -> String {
        let mut result = String::new();
        for ch in word.chars() {
            match ch {
                'a'..='z' | 'A'..='Z' => {
                    result.push(ch.to_ascii_lowercase());
                }
                '0'..='9' => result.push(ch),
                '_' | '-' => result.push('/'),
                _ => {}
            }
        }
        if result.len() > 4 {
            result[..4].to_string()
        } else {
            result
        }
    }
}

impl Default for WenyanFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wenyan_basic() {
        let f = WenyanFilter::new();
        let result = f.compress("run exercise body");
        assert_eq!(result, "跑/練/身");
    }

    #[test]
    fn test_wenyan_empty() {
        let f = WenyanFilter::new();
        assert_eq!(f.compress(""), "");
        assert_eq!(f.compress("   "), "");
    }

    #[test]
    fn test_wenyan_unknown_words() {
        let f = WenyanFilter::new();
        let result = f.compress("quantum flux");
        assert_eq!(result, "quan/flux");
    }

    #[test]
    fn test_wenyan_domain() {
        let f = WenyanFilter::new();
        assert_eq!(f.compress_domain("Body & Fitness"), "身健");
        assert_eq!(f.compress_domain("Operational"), "運");
        assert_eq!(f.compress_domain("Intellectual Work"), "智");
    }

    #[test]
    fn test_wenyan_goal() {
        let f = WenyanFilter::new();
        let result = f.compress_goal("morning run", "Body & Fitness", 0.75);
        assert!(result.contains("晨/跑·75%"));
    }

    #[test]
    fn test_wenyan_experience() {
        let f = WenyanFilter::new();
        let result = f.compress_experience("ran 5k", 0.8, "daily cardio");
        assert_eq!(result, "行:ran/5k·評80·由:dail/card");
    }

    #[test]
    fn test_wenyan_goal_no_progress() {
        let f = WenyanFilter::new();
        let result = f.compress_goal("write code", "Intellectual", 0.0);
        assert_eq!(result, "智/書/碼·0%");
    }
}
