//! Stateless administration-review primitives.
//!
//! These are deliberately decision-support types: they identify duplicate
//! documents and review status, but never approve payments or mutate an ERP.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDocument {
    pub id: String,
    pub path: String,
    pub kind: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub document_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub document_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueue {
    pub documents: usize,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub duplicate_documents: usize,
}

/// Group documents by a supplied content fingerprint. Empty fingerprints are
/// ignored because “unknown” must never create a false duplicate cluster.
pub fn duplicate_groups(documents: &[ReviewDocument]) -> Vec<DuplicateGroup> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for doc in documents {
        if let Some(fp) = doc.fingerprint.as_deref().filter(|fp| !fp.is_empty()) {
            grouped
                .entry(fp.to_string())
                .or_default()
                .push(doc.id.clone());
        }
    }
    grouped
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(fingerprint, document_ids)| DuplicateGroup {
            fingerprint,
            document_ids,
        })
        .collect()
}

pub fn review_queue(documents: &[ReviewDocument]) -> ReviewQueue {
    let duplicate_groups = duplicate_groups(documents);
    let duplicate_documents = duplicate_groups.iter().map(|g| g.document_ids.len()).sum();
    ReviewQueue {
        documents: documents.len(),
        duplicate_groups,
        duplicate_documents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_review_ignores_unknown_fingerprints() {
        let docs = vec![
            ReviewDocument {
                id: "a".into(),
                path: "a.pdf".into(),
                kind: "invoice".into(),
                fingerprint: Some("same".into()),
                vendor: None,
                document_number: None,
            },
            ReviewDocument {
                id: "b".into(),
                path: "b.pdf".into(),
                kind: "invoice".into(),
                fingerprint: Some("same".into()),
                vendor: None,
                document_number: None,
            },
            ReviewDocument {
                id: "c".into(),
                path: "c.pdf".into(),
                kind: "invoice".into(),
                fingerprint: None,
                vendor: None,
                document_number: None,
            },
        ];
        let queue = review_queue(&docs);
        assert_eq!(queue.documents, 3);
        assert_eq!(queue.duplicate_groups.len(), 1);
        assert_eq!(queue.duplicate_documents, 2);
    }
}
