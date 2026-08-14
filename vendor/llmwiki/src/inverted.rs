use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{NumericOptions, Schema, Value, STORED, TEXT};
use tantivy::{Index, IndexWriter, TantivyDocument};

use crate::indexer::FileMeta;
use crate::parser;

fn make_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_u64_field("file_id", NumericOptions::default().set_stored());
    builder.add_text_field("path", STORED);
    builder.add_text_field("title", TEXT | STORED);
    builder.add_text_field("description", TEXT | STORED);
    builder.add_text_field("body", TEXT);
    builder.add_text_field("tags", TEXT | STORED);
    builder.build()
}

pub fn build_tantivy_index(wiki_root: &Path, meta: &[FileMeta]) -> Result<()> {
    let tantivy_dir = wiki_root.join(".index").join("tantivy");

    if tantivy_dir.exists() {
        fs::remove_dir_all(&tantivy_dir)
            .with_context(|| format!("Failed to clear {}", tantivy_dir.display()))?;
    }
    fs::create_dir_all(&tantivy_dir)
        .with_context(|| format!("Failed to create {}", tantivy_dir.display()))?;

    let schema = make_schema();
    let index = Index::create_in_dir(&tantivy_dir, schema.clone())
        .with_context(|| "Failed to create Tantivy index")?;

    let file_id_field = schema.get_field("file_id").unwrap();
    let path_field = schema.get_field("path").unwrap();
    let title_field = schema.get_field("title").unwrap();
    let description_field = schema.get_field("description").unwrap();
    let body_field = schema.get_field("body").unwrap();
    let tags_field = schema.get_field("tags").unwrap();

    let mut writer: IndexWriter = index.writer(50_000_000)?;

    for fm in meta {
        let abs_path = wiki_root.join(&fm.path);
        let content = match fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: skipping {} for Tantivy — {}", fm.path, e);
                continue;
            }
        };
        let body = parser::strip_frontmatter(&content).to_owned();

        let mut doc = TantivyDocument::default();
        doc.add_u64(file_id_field, fm.id as u64);
        doc.add_text(path_field, &fm.path);
        doc.add_text(title_field, &fm.title);
        doc.add_text(description_field, &fm.description);
        doc.add_text(body_field, &body);
        for tag in &fm.tags {
            doc.add_text(tags_field, tag);
        }
        writer.add_document(doc)?;
    }

    writer.commit()?;
    Ok(())
}

pub fn keyword_search(wiki_root: &Path, query: &str, top_k: usize) -> Result<Vec<(String, f32)>> {
    let tantivy_dir = wiki_root.join(".index").join("tantivy");

    let index = Index::open_in_dir(&tantivy_dir)
        .with_context(|| "Tantivy index not found — run `llmwiki index` first")?;

    let schema = index.schema();
    let title_field = schema.get_field("title")?;
    let description_field = schema.get_field("description")?;
    let body_field = schema.get_field("body")?;
    let path_field = schema.get_field("path")?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let query_parser =
        QueryParser::for_index(&index, vec![title_field, description_field, body_field]);
    let (parsed_query, _errors) = query_parser.parse_query_lenient(query);

    let top_docs = searcher.search(&*parsed_query, &TopDocs::with_limit(top_k))?;

    let mut results = Vec::new();
    for (score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_address)?;
        let path = doc
            .get_first(path_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !path.is_empty() {
            results.push((path, score));
        }
    }

    Ok(results)
}

pub fn get_match_locations(wiki_root: &Path, path: &str, query: &str) -> Vec<(u32, String)> {
    let abs_path = wiki_root.join(path);
    let content = match fs::read_to_string(&abs_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .collect();

    if tokens.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();

    'outer: for (idx, line) in content.lines().enumerate() {
        let line_lower = line.to_lowercase();
        for token in &tokens {
            if line_lower.contains(token.as_str()) {
                matches.push(((idx + 1) as u32, token.clone()));
                if matches.len() >= 3 {
                    break 'outer;
                }
                break;
            }
        }
    }

    matches
}
