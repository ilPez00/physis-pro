use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub rank: usize,
    pub path: String,
    pub score: f32,
    pub description: String,
    pub match_locations: Vec<(u32, String)>,
}

pub fn format_results(results: &[SearchResult]) -> String {
    let mut output = String::new();

    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            output.push('\n');
        }

        output.push_str(&format!(
            "[{}] {}  (score: {:.2})\n",
            result.rank, result.path, result.score
        ));

        let wrapped = wrap_text(&result.description, 72);
        for line in wrapped {
            output.push_str(&format!("    {}\n", line));
        }

        if !result.match_locations.is_empty() {
            let locations: Vec<String> = result
                .match_locations
                .iter()
                .map(|(line_num, token)| format!("L{} \"{}\"", line_num, token))
                .collect();
            output.push_str(&format!("    \u{2192} match at: {}\n", locations.join(" \u{00b7} ")));
        }
    }

    output.trim_end().to_string()
}

pub fn rrf_merge(
    keyword: &[(String, f32)],
    semantic: &[(String, f32)],
    k: f32,
) -> Vec<(String, f32)> {
    use std::collections::{HashMap, HashSet};

    let absent_rank = f32::MAX / 2.0;

    let kw_ranks: HashMap<&str, f32> = keyword
        .iter()
        .enumerate()
        .map(|(i, (path, _))| (path.as_str(), (i + 1) as f32))
        .collect();
    let sem_ranks: HashMap<&str, f32> = semantic
        .iter()
        .enumerate()
        .map(|(i, (path, _))| (path.as_str(), (i + 1) as f32))
        .collect();

    let all_paths: HashSet<&str> = keyword
        .iter()
        .map(|(p, _)| p.as_str())
        .chain(semantic.iter().map(|(p, _)| p.as_str()))
        .collect();

    let mut results: Vec<(String, f32)> = all_paths
        .into_iter()
        .map(|path| {
            let rank_kw = kw_ranks.get(path).copied().unwrap_or(absent_rank);
            let rank_sem = sem_ranks.get(path).copied().unwrap_or(absent_rank);
            let score = 1.0 / (k + rank_kw) + 1.0 / (k + rank_sem);
            (path.to_string(), score)
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
}

#[allow(dead_code)]
pub fn results_to_json(results: &[SearchResult]) -> serde_json::Value {
    serde_json::to_value(results).unwrap_or(serde_json::Value::Array(vec![]))
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in words {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_results_basic() {
        let results = vec![SearchResult {
            rank: 1,
            path: "bio/photosynthesis.md".to_string(),
            score: 0.94,
            description: "How plants convert light to glucose.".to_string(),
            match_locations: vec![(67, "light".to_string())],
        }];
        let out = format_results(&results);
        assert!(out.contains("[1] bio/photosynthesis.md  (score: 0.94)"));
        assert!(out.contains("How plants convert light to glucose."));
        assert!(out.contains("L67 \"light\""));
    }

    #[test]
    fn test_format_results_no_matches() {
        let results = vec![SearchResult {
            rank: 1,
            path: "page.md".to_string(),
            score: 0.5,
            description: "A page.".to_string(),
            match_locations: vec![],
        }];
        let out = format_results(&results);
        assert!(!out.contains("match at:"));
    }

    #[test]
    fn test_wrap_text() {
        let long = "word ".repeat(20).trim().to_string();
        let lines = wrap_text(&long, 20);
        for line in &lines {
            assert!(line.len() <= 20, "line too long: {:?}", line);
        }
    }

    #[test]
    fn test_results_to_json() {
        let results = vec![SearchResult {
            rank: 1,
            path: "p.md".to_string(),
            score: 1.0,
            description: "desc".to_string(),
            match_locations: vec![],
        }];
        let json = results_to_json(&results);
        assert!(json.is_array());
        assert_eq!(json[0]["rank"], 1);
        assert_eq!(json[0]["path"], "p.md");
    }

    #[test]
    fn test_rrf_merge_combined() {
        let kw = vec![
            ("a.md".to_string(), 0.9_f32),
            ("b.md".to_string(), 0.7),
        ];
        let sem = vec![
            ("b.md".to_string(), 0.95_f32),
            ("c.md".to_string(), 0.8),
        ];
        let merged = rrf_merge(&kw, &sem, 60.0);
        // b.md appears in both lists so should score highest
        assert_eq!(merged[0].0, "b.md");
        // all three paths should be present
        let paths: Vec<&str> = merged.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"a.md"));
        assert!(paths.contains(&"b.md"));
        assert!(paths.contains(&"c.md"));
    }

    #[test]
    fn test_rrf_merge_keyword_only() {
        let kw = vec![("x.md".to_string(), 1.0_f32)];
        let sem: Vec<(String, f32)> = vec![];
        let merged = rrf_merge(&kw, &sem, 60.0);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].0, "x.md");
    }

    #[test]
    fn test_rrf_merge_empty() {
        let merged = rrf_merge(&[], &[], 60.0);
        assert!(merged.is_empty());
    }
}
