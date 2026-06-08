//! llmwiki/uberwiki bridge — searches the project wiki for context.
//! Uses BM25 (Tantivy) + semantic (fastembed) with RRF fusion when available.
//! Falls back to simple filesystem grep when the wiki binary is not running.
//! Also searches the FileThemeGraph for matching themes and files.

use std::path::Path;
#[cfg(feature = "wiki-embedded")]
use std::path::PathBuf;
use std::process::Command;

use crate::graph::FileThemeGraph;

/// Seed the wiki with default documentation pages (ontology reference, tools, config).
/// Creates `.md` files with YAML frontmatter at `{wiki_path}/physis/`.
/// Safe to call repeatedly — overwrites existing files.
#[cfg(feature = "wiki-embedded")]
pub fn seed_default_wiki(wiki_path: &Path) -> std::io::Result<()> {
    let pages: Vec<(&str, &str, Vec<&str>, &str)> = vec![
        ("physis/overview.md", "Physis System Overview", vec!["physis", "architecture", "overview"],
         r#"Physis is an industrial backoffice intelligence engine that embeds business observations into a semiotic ontology grid and drives a PDCA (Plan-Do-Check-Act) improvement loop.

## Architecture

- **Ontology Grid** — 10 ontologies (human, machine, semiotic, category, agent, natural, social, abstract, engineering, office) with ~500+ domains, each mapped to a domain×mode coordinate in the semiotic square.
- **Embedding Engine** — BGE-base-en-v1.5 ONNX model (768-dim vectors) + RandomProjectionEmbedder fallback.
- **PDCA Actor** — Zero-shot LLM planning and execution loop for continuous improvement.
- **Dream Engine** — Generates and evaluates hypothetical future scenarios.
- **Coherence Core** — Graph-based belief maintenance and contradiction detection.
- **Wiki** — Tantivy BM25 search over markdown documentation with YAML frontmatter.
- **MCP Server** — Model Context Protocol via SSE on port 8080; 13+ tools for AI agent integration.
"#),
        ("physis/ontologies.md", "Ontology Reference", vec!["ontology", "domain", "semiotic"],
         r#"Physis uses 10 ontologies, each containing multiple domains. Every domain is mapped to a semiotic coordinate (domain×mode).

## Human Ontology
The core semiotic square: **HEAL**, **CONSTRUCT**, **FABRICATE**, **BOND**, **STUDY**.
Each domain has associated modes (CREATE, WORK, DESTROY, etc.) forming the activity grid.

## Machine Ontology (23 domains)
Thermal Zone, Pressure Regulation, Torque & Force, Speed Control, Flow Management, Level Management, Material Feed, Power Draw, Machine Mode, Startup Sequence, Changeover, Batch Tracking, Machine Health State, Remaining Useful Life, Safety Interlock, Environmental Condition, Alarm Categorization, Anomaly Detection, Production Throughput, OEE, Cycle Time, Yield & Scrap, SPC Quality.

## AI Agent Ontology (21 domains)
System Prompt, User Input, Assistant Response, Context Window, Prompt Template, Working Memory, Episodic Memory, Semantic Memory, Tool Invocation, Tool Result, Output Publishing, Error Recovery, Quality Check, Confidence Threshold, Human Escalation, PDCA Cycle, Workflow Chain, Agent Loop, Agent State, Agent Versioning, Telemetry & Log.

## Office Ontology (75 domains across 12 categories)
**Communication** — Email, Meeting, Phone Call, Presentation, Report, Newsletter, Internal Memo, Chat, Social Media, Survey.
**Finance** — Budget Planning, Invoice, Expense Report, Payroll, Tax Filing, Audit Trail, Revenue Report, Cost Analysis, Cash Flow, Investment.
**Procurement** — Purchase Order, Supplier Contract, Vendor Evaluation, RFQ, Receiving Report.
**People** — Employee Record, Performance Review, Training Log, Timesheet, Leave Request, Recruitment, Onboarding, Offboarding, Diversity Report.
**Projects** — Project Plan, Task List, Milestone, Risk Register, Status Report, Post-Mortem, Resource Allocation, Stakeholder Map.
**Customer** — Lead Record, CRM Entry, Customer Feedback, Support Ticket, SLA Report, NPS Score, Account Plan, Contract Renewal.
**Reporting** — KPI Dashboard, Quarterly Review, Board Deck, Compliance Report, Benchmarking.
**IT** — Incident Ticket, Change Request, Asset Inventory, Access Log, Backup Report, Security Audit, License Tracker.
**Facilities** — Maintenance Log, Floor Plan, Asset Tag, Safety Inspection, Visitor Log.
**Legal** — Contract Repository, NDA, Policy Document, IP Record, Dispute Log.
**Marketing** — Campaign Brief, Content Calendar, SEO Tracker, Ad Spend Report, Social Analytics.
**Travel** — Itinerary, Expense Claim, Travel Policy, Mileage Log.
**R&D** — Research Note, Experiment Log, Patent Disclosure, Lab Notebook.

## Semiotic Ontology
Greimas square, semiotic grid, triadic sign structure — qualitative analysis tools.

## Engineering Ontology
Design specs, CAD files, BOM, test protocols, validation reports.

## Other Ontologies
- **Natural** — environmental data, sensor readings
- **Social** — organizational structure, stakeholder relationships
- **Abstract** — conceptual categories, theoretical frameworks
- **Category** — classification systems, taxonomies
"#),
        ("physis/mcp_tools.md", "MCP Tools Reference", vec!["mcp", "tools", "api"],
         r#"Physis exposes tools via the Model Context Protocol (MCP) over SSE at `/mcp/sse`.

## Available Tools

### `physis_health`
- **Description**: Check system health and status
- **Arguments**: (none)
- **Returns**: `{"status":"ok","classifier_cells":N,"goals":N,"dreams":N}`

### `physis_classify`
- **Description**: Classify a text observation against the ontology grid
- **Arguments**: `text` (string) — the observation text
- **Returns**: Top matching domains with domain×mode and score

### `physis_embed`
- **Description**: Embed text into a vector
- **Arguments**: `text` (string)
- **Returns**: Embedding vector (array of floats)

### `physis_goals_list`
- **Description**: List all active goals
- **Arguments**: (none)
- **Returns**: List of goals with id, progress, domain

### `physis_goals_add`
- **Description**: Add a new goal
- **Arguments**: `name` (string), optional `domain` (string)
- **Returns**: Created goal

### `physis_goals_remove`
- **Description**: Remove a goal by id
- **Arguments**: `id` (string)
- **Returns**: Confirmation

### `physis_pdca_plan`
- **Description**: Generate a PDCA plan for a goal
- **Arguments**: `goal_id` (string), `context` (string, optional)
- **Returns**: Plan as structured text

### `physis_pdca_act`
- **Description**: Execute a PDCA step
- **Arguments**: `goal_id` (string), `step_id` (string), `result` (string)
- **Returns**: Action result

### `physis_pdca_stats`
- **Description**: Get PDCA statistics
- **Arguments**: (none)
- **Returns**: Stats summary

### `physis_dream_generate`
- **Description**: Generate hypothetical scenarios
- **Arguments**: `prompt` (string), `variations` (integer, optional, default 3)
- **Returns**: List of generated dreams

### `physis_dream_evaluate`
- **Description**: Evaluate a dream scenario
- **Arguments**: `dream_id` (string)
- **Returns**: Evaluation with scores

### `physis_query`
- **Description**: Search the in-memory scan trie
- **Arguments**: `query` (string), `max` (integer, optional, default 10)
- **Returns**: Matching trie paths (populated via `physis_scan`)

### `physis_stats`
- **Description**: Get comprehensive system statistics
- **Arguments**: (none)
- **Returns**: Full stats including classifier, goals, dreams, coherence

### `physis_wiki_search`
- **Description**: Search the physis wiki documentation
- **Arguments**: `query` (string)
- **Returns**: Wiki search results with titles and excerpts
"#),
        ("physis/configuration.md", "Configuration Guide", vec!["config", "environment", "setup"],
         r#"Physis configuration is loaded from `~/.physis/config.json` and environment variables.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PHYSIS_API_TOKEN` | Bearer token for API auth | (none, no auth) |
| `PHYSIS_LOG_DIR` | Log output directory | `~/.physis/logs` |
| `PHYSIS_DATA_DIR` | Data storage directory | `~/.physis` |
| `OPENAI_API_KEY` | OpenAI API key | (none) |
| `ANTHROPIC_API_KEY` | Anthropic API key | (none) |
| `SUPABASE_URL` | Supabase project URL | (none) |
| `SUPABASE_KEY` | Supabase service role key | (none) |

## Data Paths

| Path | Purpose |
|------|---------|
| `{data_dir}/wiki/` | Wiki markdown files + Tantivy index |
| `{data_dir}/theme_graph.json` | FileThemeGraph persistence |
| `{data_dir}/quality_failures.json` | Quality tracker failures |
| `{data_dir}/config.json` | Physis configuration file |

## Features (Cargo)

- `web` — HTTP API server (axum)
- `cli` — CLI binary (`physis-pro`)
- `mcp` — MCP protocol server
- `embed-onnx` — ONNX embedding model
- `wiki-embedded` — In-process Tantivy wiki indexer
- `industrial` — Machine adapters (MQTT, Modbus)
- `ingest` — Document ingest pipeline
- `full` — All of the above
"#),
    ];

    let wiki_physis_dir = wiki_path.join("physis");
    std::fs::create_dir_all(&wiki_physis_dir)?;

    for (rel_path, title, tags, body) in &pages {
        let file_path = wiki_path.join(rel_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tag_str = tags.iter().map(|t| format!("  - {t}")).collect::<Vec<_>>().join("\n");
        let content = format!(
            "---\ntitle: {title}\ntags:\n{tag_str}\n---\n\n{body}"
        );
        std::fs::write(&file_path, content)?;
    }

    Ok(())
}

/// Ensure the wiki index is built. Seeds default pages if none exist, then
/// runs the Tantivy BM25 indexer. Safe to call on every startup.
#[cfg(feature = "wiki-embedded")]
pub fn ensure_wiki_indexed(wiki_path: &Path) {
    if let Err(e) = seed_default_wiki(wiki_path) {
        log::warn!("[wiki] failed to seed default pages: {e}");
        return;
    }
    match llmwiki_physis::index(wiki_path, true) {
        Ok(stats) => log::info!("[wiki] indexed +{} ~{} -{} ({} skipped)",
            stats.added, stats.updated, stats.removed, stats.skipped),
        Err(e) => log::warn!("[wiki] index failed: {e}"),
    }
}

/// Combined wiki search — queries the llmwiki index AND the file-theme graph.
/// Returns formatted results from both sources.
pub fn search_all(query: &str, wiki_path: &Path, graph_path: &Path) -> Vec<String> {
    let mut results = Vec::new();

    // 1. Search the llmwiki
    let wiki_results = search_wiki(query, wiki_path);
    for r in wiki_results {
        results.push(format!("[wiki] {}", r));
    }

    // 2. Search the file-theme graph
    if graph_path.exists() {
        if let Ok(graph) = FileThemeGraph::load(graph_path) {
            let graph_results = search_graph(query, &graph);
            for r in graph_results {
                results.push(format!("[graph] {}", r));
            }
        }
    }

    results
}

/// Search the llmwiki index for a query.
/// Tries to call the `wiki` CLI binary first; falls back to grep.
pub fn search_wiki(query: &str, wiki_path: &Path) -> Vec<String> {
    // In-process Tantivy BM25 (no subprocess). Falls through to the binary/grep
    // on empty results or error. Gated behind `wiki-embedded`.
    #[cfg(feature = "wiki-embedded")]
    {
        match llmwiki_physis::search_bm25(wiki_path, query, 10) {
            Ok(results) if !results.is_empty() => {
                return llmwiki_physis::format_results(&results)
                    .lines()
                    .map(|l| l.to_string())
                    .collect();
            }
            Ok(_) => {}
            Err(e) => log::warn!("[wiki] in-process search failed: {e}; falling back"),
        }
    }

    let wiki_bin = which("wiki").or_else(|| which("llmwiki"));
    if let Some(bin) = wiki_bin {
        match Command::new(bin).arg("search").arg(query).arg("--path").arg(wiki_path).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
                if !lines.is_empty() {
                    return lines;
                }
            }
            Err(e) => {
                log::warn!("wiki binary failed: {e}, falling back to grep");
            }
        }
    }
    // Fallback: recursive grep
    let mut results = Vec::new();
    if wiki_path.exists() {
        let _ = walk_files(wiki_path, query, &mut results);
    }
    results
}

/// Search the FileThemeGraph for matching themes and files (uberwiki integration).
pub fn search_graph(query: &str, graph: &FileThemeGraph) -> Vec<String> {
    let mut results = Vec::new();
    let _q = query.to_lowercase();

    // Match themes
    let matching_themes = graph.search_themes_by_text(query);
    for theme in matching_themes {
        let file_count = graph.edges.iter().filter(|e| e.theme_id == theme.id).count();
        let cat = theme.category.as_deref().unwrap_or("uncategorized");
        results.push(format!("theme: {} [{}×{}] ({}, {} files)",
            theme.name, theme.domain, theme.mode, cat, file_count));
    }

    // Match files
    let matching_files = graph.search_files(query);
    for file in matching_files {
        let themes: Vec<&str> = graph.edges.iter()
            .filter(|e| e.file_id == file.id)
            .filter_map(|e| graph.get_theme(e.theme_id))
            .map(|t| t.name.as_str())
            .collect();
        results.push(format!("file: {} ({}) — themes: {}",
            file.title, file.path.display(), themes.join(", ")));
    }

    results
}

fn walk_files(dir: &Path, query: &str, results: &mut Vec<String>) -> std::io::Result<()> {
    let q = query.to_lowercase();
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, query, results)?;
        } else if let Some(ext) = path.extension() {
            if ext == "md" || ext == "txt" {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.to_lowercase().contains(&q) {
                        results.push(format!("{}: ...{}...", path.display(),
                            &content[content.to_lowercase().find(&q)
                                .map(|i| i.saturating_sub(20))
                                .unwrap_or(0)..]
                                .chars().take(100).collect::<String>()));
                    }
                }
            }
        }
    }
    Ok(())
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).filter_map(|dir| {
            let full = dir.join(name);
            if full.is_file() { Some(full) } else { None }
        }).next()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::FileThemeGraph;

    #[test]
    fn test_search_wiki_nonexistent_path() {
        let results = search_wiki("test", Path::new("/nonexistent/wiki/path"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_graph_themes() {
        let mut g = FileThemeGraph::new();
        g.add_theme("Budget Planning", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        g.add_theme("Email Management", Some("Communication"), "BOND", "WORK", "office", vec![0.9, 0.1]);
        let results = search_graph("budget", &g);
        assert!(results.iter().any(|r| r.contains("Budget Planning")));
        assert!(!results.iter().any(|r| r.contains("Email Management")));
    }

    #[test]
    fn test_search_graph_files() {
        let mut g = FileThemeGraph::new();
        g.add_theme("Budget", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        g.add_file(std::path::PathBuf::from("budget.xlsx"), "Q4 Budget", "Spreadsheet", vec![0.1, 0.9]);
        g.add_edge(0, 0, 0.95);
        let results = search_graph("Q4", &g);
        assert!(results.iter().any(|r| r.contains("Q4 Budget")));
    }

    #[test]
    fn test_search_graph_no_match() {
        let g = FileThemeGraph::new();
        let results = search_graph("nonexistent", &g);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_all_empty_graph() {
        let dir = tempfile::tempdir().unwrap();
        let graph_path = dir.path().join("graph.json");
        let wiki_path = dir.path().join("wiki");
        let results = search_all("test", &wiki_path, &graph_path);
        assert!(results.is_empty());
    }
}
