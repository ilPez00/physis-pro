use crate::capture::RunRecord;
use crate::embed::{combine, cosine, hash_embed};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PhysisSignals {
    pub semantic: f32,
    pub structural: f32,
    pub topical: f32,
}

pub fn score(record: &RunRecord) -> PhysisSignals {
    let in_files: Vec<(String, String)> = record
        .input
        .rag_files
        .iter()
        .map(|f| (f.path.clone(), f.body.clone()))
        .collect();
    let out_files: Vec<(String, String)> = record
        .output
        .edited_files
        .iter()
        .map(|f| (f.path.clone(), f.body.clone()))
        .collect();

    let in_text = combine(&record.input.prompt, &in_files);
    let out_text = combine(&record.output.text, &out_files);

    let in_vec = hash_embed(&in_text);
    let out_vec = hash_embed(&out_text);

    let sim = cosine(&in_vec, &out_vec).clamp(-1.0, 1.0);
    let semantic = ((1.0 - sim) * 0.5).clamp(0.0, 1.0);

    let structural = struct_distance(&in_files, &out_files);

    let topical = topic_jaccard(&in_text, &out_text);

    PhysisSignals { semantic, structural, topical }
}

fn struct_distance(a: &[(String, String)], b: &[(String, String)]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let a_keys: std::collections::HashSet<&str> =
        a.iter().map(|(p, _)| p.as_str()).collect();
    let b_keys: std::collections::HashSet<&str> =
        b.iter().map(|(p, _)| p.as_str()).collect();
    let union: usize = a_keys.union(&b_keys).count();
    if union == 0 {
        return 0.0;
    }
    let inter: usize = a_keys.intersection(&b_keys).count();
    let jaccard = inter as f32 / union as f32;
    (1.0 - jaccard).clamp(0.0, 1.0)
}

fn topic_jaccard(a: &str, b: &str) -> f32 {
    let ha = top_hash(a);
    let hb = top_hash(b);
    if ha.is_empty() && hb.is_empty() {
        return 0.0;
    }
    let inter = ha.intersection(&hb).count() as f32;
    let union = ha.union(&hb).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        (1.0 - inter / union).clamp(0.0, 1.0)
    }
}

fn top_hash(text: &str) -> std::collections::HashSet<String> {
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for w in text.split_whitespace() {
        let w = w
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if w.len() >= 4 {
            *counts.entry(w).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    let _ = Sha256::digest(b"unused");
    sorted.into_iter().take(20).map(|(w, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::*;

    fn rec(prompt: &str, files_in: &[(&str, &str)], text: &str, files_out: &[(&str, &str)]) -> RunRecord {
        RunRecord {
            run_id: None,
            model: "test".into(),
            task: "t".into(),
            created_at: None,
            input: InputSide {
                prompt: prompt.into(),
                rag_files: files_in
                    .iter()
                    .map(|(p, b)| RagFile { path: (*p).into(), body: (*b).into() })
                    .collect(),
            },
            output: OutputSide {
                text: text.into(),
                edited_files: files_out
                    .iter()
                    .map(|(p, b)| EditedFile { path: (*p).into(), body: (*b).into() })
                    .collect(),
                deleted_files: vec![],
            },
        }
    }

    #[test]
    fn identical_runs_have_low_risk() {
        let r = rec(
            "explain photosynthesis",
            &[("notes.md", "plants use sunlight water co2")],
            "photosynthesis uses sunlight water and co2 to make glucose",
            &[("notes.md", "plants use sunlight water co2")],
        );
        let s = score(&r);
        assert!(s.semantic < 0.5, "semantic={}", s.semantic);
        assert!(s.structural < 0.1, "structural={}", s.structural);
    }

    #[test]
    fn unrelated_runs_have_higher_semantic_risk() {
        let r = rec(
            "explain photosynthesis",
            &[("notes.md", "plants use sunlight water co2")],
            "the capital of france is paris and its population is large",
            &[],
        );
        let s = score(&r);
        let clean = score(&rec(
            "explain photosynthesis",
            &[("notes.md", "plants use sunlight water co2")],
            "photosynthesis uses sunlight water and co2 to make glucose",
            &[("notes.md", "plants use sunlight water co2")],
        ));
        assert!(s.semantic > clean.semantic, "unrelated={} clean={}", s.semantic, clean.semantic);
    }
}

