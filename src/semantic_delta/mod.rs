use serde::{Deserialize, Serialize};

/// Represents a lexical-level diff between two text spans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LexicalDiff {
    pub char_change_count: usize,
    pub word_change_count: usize,
    pub is_formatting_only: bool,
    pub new_words: Vec<String>,
    pub removed_words: Vec<String>,
}

impl LexicalDiff {
    pub fn no_change() -> Self {
        Self {
            char_change_count: 0,
            word_change_count: 0,
            is_formatting_only: true,
            new_words: vec![],
            removed_words: vec![],
        }
    }
}

/// Delta variants for different semantic change types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PolarityDelta {
    Unchanged,
    Negated,
    Flipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScopeDelta {
    Unchanged,
    Narrowed,
    Widened,
    Shifted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModalityDelta {
    Unchanged,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityDelta {
    pub old_value: f32,
    pub new_value: f32,
    pub direction: QuantityDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QuantityDirection {
    Increase,
    Decrease,
    New,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityDelta {
    pub entity_type: String,
    pub old_value: String,
    pub new_value: String,
    pub change_type: EntityChangeType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityChangeType {
    Substitution,
    Addition,
    Removal,
    Negation,
}

/// Full semantic delta representing what meaning changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDelta {
    pub source_node: String,
    pub lexical: LexicalDiff,
    pub proposition_changed: bool,
    pub scope_change: Option<ScopeDelta>,
    pub polarity_change: Option<PolarityDelta>,
    pub quantity_change: Option<QuantityDelta>,
    pub modality_change: Option<ModalityDelta>,
    pub entity_changes: Vec<EntityDelta>,
    pub relation_changes: Vec<RelationDelta>,
    pub old_claims: Vec<String>,
    pub new_claims: Vec<String>,
    pub removed_constraints: Vec<String>,
    pub added_constraints: Vec<String>,
    pub semantic_distance: f32,
    pub confidence: f32,
}

impl SemanticDelta {
    pub fn no_semantic_change(source_node: String, old_text: &str, new_text: &str) -> Self {
        let lexical = if old_text.trim() == new_text.trim() {
            LexicalDiff::no_change()
        } else {
            LexicalDiff {
                char_change_count: new_text.len().max(old_text.len()),
                word_change_count: new_text
                    .split_whitespace()
                    .count()
                    .max(old_text.split_whitespace().count()),
                is_formatting_only: Self::is_formatting_only_change(old_text, new_text),
                new_words: vec![],
                removed_words: vec![],
            }
        };

        Self {
            source_node,
            lexical,
            proposition_changed: false,
            scope_change: None,
            polarity_change: None,
            quantity_change: None,
            modality_change: None,
            entity_changes: vec![],
            relation_changes: vec![],
            old_claims: vec![],
            new_claims: vec![],
            removed_constraints: vec![],
            added_constraints: vec![],
            semantic_distance: 1.0,
            confidence: 1.0,
        }
    }

    fn is_formatting_only_change(old_text: &str, new_text: &str) -> bool {
        let old_words: Vec<&str> = old_text.split_whitespace().collect();
        let new_words: Vec<&str> = new_text.split_whitespace().collect();

        let same_words: bool = old_words.iter().all(|w| {
            new_words
                .iter()
                .any(|nw| nw.trim().to_lowercase() == w.trim().to_lowercase())
        }) && new_words.iter().all(|w| {
            old_words
                .iter()
                .any(|ow| ow.trim().to_lowercase() == w.trim().to_lowercase())
        });

        same_words && old_text.chars().count() == new_text.chars().count()
    }

    pub fn compute_semantic_distance<E: Embedder>(
        embedder: &E,
        old_text: &str,
        new_text: &str,
    ) -> f32 {
        let old_emb = embedder.embed(old_text);
        let new_emb = embedder.embed(new_text);
        1.0 - cosine_sim(&old_emb, &new_emb).clamp(0.0, 1.0)
    }

    pub fn escalate_with_model(
        &self,
        _old_repr: &SemanticRepresentation,
        _new_repr: &SemanticRepresentation,
        initial_distance: f32,
    ) -> Self {
        let mut result = self.clone();
        result.semantic_distance = initial_distance.min(0.95);
        result.confidence = 0.8;
        result
    }
}

/// Semantic representation extracted from text for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticRepresentation {
    pub propositions: Vec<String>,
    pub entities: Vec<String>,
    pub quantities: Vec<QuantityInfo>,
    pub relations: Vec<String>,
    pub modality: String,
    pub polarity: String,
    pub scope: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityInfo {
    pub value: f32,
    pub unit: Option<String>,
    pub context: String,
}

/// Trait for text embedders used by the semantic delta detector.
pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
    fn dimension(&self) -> usize;
}

/// Default embedder using deterministic hash-based embedding.
#[derive(Debug)]
pub struct RandomProjectionEmbedder {
    dim: usize,
}

impl RandomProjectionEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl Embedder for RandomProjectionEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut h = 0u64;
        for ch in text.chars() {
            h = h.wrapping_mul(0x100000001).wrapping_add(ch as u64);
        }
        let h = h;
        let dim = self.dim;
        (0..dim)
            .map(|i| {
                let bit_shift = (i * 7) % 64;
                let bit = ((h >> bit_shift) & 1_u64) as f32;
                (bit - 0.5) * 2.0
            })
            .collect()
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// Compute cosine similarity between two vectors.
pub fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    (dot / (na * nb)).clamp(-1.0, 1.0)
}

/// Extract a semantic representation from text.
pub fn extract_semantic_representation(
    text: &str,
    embedder: &dyn Embedder,
) -> SemanticRepresentation {
    let embedding = embedder.embed(text);

    let propositions: Vec<String> = text
        .split('.')
        .filter(|s| !s.trim().is_empty() && s.split_whitespace().count() > 3)
        .map(|s| s.trim().to_string())
        .collect();

    let entities: Vec<String> = text
        .split(|c: char| {
            c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '-' | '_')
        })
        .filter(|s| !s.is_empty())
        .filter(|s| {
            s.chars()
                .next()
                .map(|c: char| c.is_uppercase())
                .unwrap_or(false)
        })
        .map(|s| s.trim().to_string())
        .collect();

    let quantities = extract_quantities(text);

    let relations: Vec<String> = vec![
        "because".to_string(),
        "therefore".to_string(),
        "however".to_string(),
        "although".to_string(),
    ]
    .into_iter()
    .filter(|r| text.contains(r))
    .collect();

    let modality = if text.to_lowercase().contains("guarantee")
        || text.to_lowercase().contains("always")
        || text.to_lowercase().contains("ensures")
    {
        "certain".to_string()
    } else if text.to_lowercase().contains("may")
        || text.to_lowercase().contains("might")
        || text.to_lowercase().contains("possibly")
    {
        "possible".to_string()
    } else {
        "contingent".to_string()
    };

    let polarity = if text.to_lowercase().contains("not")
        || text.to_lowercase().contains("no")
        || text.to_lowercase().contains("never")
    {
        "negative".to_string()
    } else {
        "positive".to_string()
    };

    let scope = if text.to_lowercase().contains("only")
        || text.to_lowercase().contains("all")
        || text.to_lowercase().contains("every")
    {
        "global".to_string()
    } else if text.to_lowercase().contains("some")
        || text.to_lowercase().contains("part")
        || text.to_lowercase().contains("portion")
    {
        "local".to_string()
    } else {
        "unspecified".to_string()
    };

    SemanticRepresentation {
        propositions,
        entities,
        quantities,
        relations,
        modality,
        polarity,
        scope,
        embedding,
    }
}

fn extract_quantities(text: &str) -> Vec<QuantityInfo> {
    let mut results = vec![];
    for word in text.split_whitespace() {
        let num_str: String = word
            .chars()
            .take_while(|c| *c == '.' || c.is_ascii_digit())
            .collect();
        if !num_str.is_empty() {
            if let Ok(num) = num_str.parse::<f32>() {
                let unit = if word.contains('%') {
                    Some("percentage".to_string())
                } else {
                    let after_num = &word[num_str.len()..];
                    if !after_num.is_empty()
                        && after_num.chars().all(|c| c.is_alphabetic() || c == '_')
                    {
                        Some(after_num.to_string())
                    } else {
                        None
                    }
                };
                results.push(QuantityInfo {
                    value: num,
                    unit,
                    context: word.to_string(),
                });
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut deduped = vec![];
    for q in &results {
        // Use context as key for deduplication (f32 doesn't implement Hash)
        let key = q.context.clone();
        if !seen.contains(&key) {
            seen.insert(key);
            deduped.push(q.clone());
        }
    }
    deduped
}

/// Relation delta representing changed relations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationDelta {
    pub relation_type: String,
    pub old_subject: String,
    pub new_subject: String,
    pub old_object: String,
    pub new_object: String,
}
