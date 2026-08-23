//! Output formatters — Wiki-style reports, JSON graph, Mermaid mindmap/flowchart, domain reports,
//! semiotic triangle/category/heatmap/square renderers.
use std::collections::HashMap;

use crate::models::{DomainDef, Goal, HumanDomain, HumanMode, SemioticGrid};
use crate::trie::DynamicVectorTrie;

/// Formats goals and trie structure into a Wiki-style markdown report.
pub fn format_wiki(trie: &DynamicVectorTrie, goals: &[Goal], title: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", title));

    // Only goals with tracked progress are worth listing. A fresh scan maps
    // one goal per file, all at 0% — dumping tens of thousands of those UUID
    // stubs (a full-disk scan produced ~93k lines) drowns the report. Collapse
    // the untracked ones to a single count; keep real progress goals verbatim.
    out.push_str("## Goals\n\n");
    let tracked: Vec<&Goal> = goals.iter().filter(|g| g.progress > 0.0).collect();
    for goal in &tracked {
        out.push_str(&format!("- [[{}]] — progress: {:.0}%\n", goal.id, goal.progress * 100.0));
    }
    let untracked = goals.len() - tracked.len();
    if untracked > 0 {
        let sep = if tracked.is_empty() { "" } else { "\n" };
        out.push_str(&format!("{sep}_{untracked} entries mapped, no progress tracked yet._\n"));
    }

    out.push_str("\n## Ontology Tree\n\n");
    out.push_str("```mermaid\nmindmap\n  root((Physis))\n");
    // Bounded recursive walk: keep the wiki tree readable on huge tries.
    const MAX_DEPTH: usize = 4;
    const MAX_NODES: usize = 300;
    fn walk(
        pool: &[crate::trie::Node],
        dict: &[String],
        idx: usize,
        depth: usize,
        emitted: &mut usize,
        out: &mut String,
    ) {
        if depth > MAX_DEPTH || *emitted >= MAX_NODES {
            return;
        }
        let node = &pool[idx];
        if depth > 0 && !dict[node.token_id as usize].is_empty() {
            let indent = "  ".repeat(depth + 1);
            out.push_str(&format!("{}{}\n", indent, dict[node.token_id as usize]));
            *emitted += 1;
        }
        let cs = node.child_start;
        let cc = node.child_count as usize;
        if cs >= 0 {
            for i in 0..cc {
                if *emitted >= MAX_NODES {
                    break;
                }
                walk(pool, dict, cs as usize + i, depth + 1, emitted, out);
            }
        }
    }
    let mut emitted = 0usize;
    walk(&trie.pool, &trie.dictionary, 0, 0, &mut emitted, &mut out);
    out.push_str("```\n\n");

    let s = trie.stats();
    out.push_str("## Stats\n\n");
    out.push_str(&format!("- Nodes: {}\n", s.get("nodes").unwrap_or(&0)));
    out.push_str(&format!("- Tokens: {}\n", s.get("tokens").unwrap_or(&0)));
    out.push_str(&format!("- Terminal nodes: {}\n", s.get("terminal_nodes").unwrap_or(&0)));
    out.push_str(&format!("- Max depth: {}\n", s.get("max_depth").unwrap_or(&0)));

    out
}

/// Exports the trie as a JSON graph with entities and relations.
pub fn format_json_graph(trie: &DynamicVectorTrie) -> String {
    let json = trie.export_json();
    serde_json::to_string_pretty(&json).unwrap_or_default()
}

/// Renders the trie as a Mermaid mindmap diagram.
pub fn format_mermaid_mindmap(trie: &DynamicVectorTrie, title: &str) -> String {
    let mut out = String::new();
    out.push_str("mindmap\n");
    out.push_str(&format!("  root(({}))\n", title));

    fn add_children(
        pool: &[crate::trie::Node],
        dict: &[String],
        idx: usize,
        depth: usize,
        out: &mut String,
    ) {
        let node = &pool[idx];
        if depth > 0 && !dict[node.token_id as usize].is_empty() {
            let indent = "  ".repeat(depth + 1);
            out.push_str(&format!("{}{}\n", indent, dict[node.token_id as usize]));
        }
        let cs = node.child_start;
        let cc = node.child_count as usize;
        if cs >= 0 {
            for i in 0..cc {
                add_children(pool, dict, cs as usize + i, depth + 1, out);
            }
        }
    }

    add_children(&trie.pool, &trie.dictionary, 0, 0, &mut out);
    out
}

/// Renders the trie as a Mermaid flowchart with parent-child edges.
pub fn format_mermaid_flowchart(trie: &DynamicVectorTrie) -> String {
    let mut out = String::new();
    out.push_str("flowchart TD\n");

    for (i, node) in trie.pool.iter().enumerate() {
        let label = trie.token_str(node.token_id);
        if label.is_empty() && i == 0 {
            continue;
        }
        let escaped = label.replace('\"', "'");
        let display = if label.is_empty() { "root" } else { &escaped };

        let cs = node.child_start as usize;
        let cc = node.child_count as usize;
        for j in 0..cc {
            let child_idx = cs + j;
            let child_label = trie.token_str(trie.pool[child_idx].token_id);
            let child_escaped = child_label.replace('\"', "'");
            out.push_str(&format!("    n{}[{}] --> n{}[{}]\n", i, display, child_idx, child_escaped));
        }
    }

    out
}

/// Produces a per-domain report with goal count, average grade, and category metadata.
pub fn format_domain_report(
    goals: &[Goal],
    domains: &HashMap<String, DomainDef>,
    domain_grades: &HashMap<String, Vec<f32>>,
) -> String {
    let mut out = String::new();
    out.push_str("# Domain Report\n\n");

    for (name, def) in domains {
        out.push_str(&format!("## {}\n\n", def.name));
        out.push_str(&format!("- Category: {}\n", def.category.as_deref().unwrap_or("none")));
        out.push_str(&format!("- Domain: {}\n", def.name));
        out.push_str(&format!("- Unit: {}\n", def.unit));

        if let Some(grades) = domain_grades.get(name) {
            let avg: f32 = grades.iter().sum::<f32>() / grades.len() as f32;
            out.push_str(&format!("- Avg grade: {:.2}\n", avg));
        }

        let domain_goals: Vec<&Goal> = goals.iter().filter(|g| g.id == *name).collect();
        if !domain_goals.is_empty() {
            out.push_str("\nGoals:\n");
            for g in &domain_goals {
                out.push_str(&format!("  - {} ({:.0}%)\n", g.id, g.progress * 100.0));
            }
        }
        out.push('\n');
    }

    out
}

// ── Semiotic Renderers ─────────────────────────────────────────────

/// Renders a Peircean semiotic triangle as a Mermaid flowchart.
pub fn format_semiotic_triangle(name: &str, representamen: &str, object: &str, interpretant: &str) -> String {
    format!(
        r#"flowchart TD
    subgraph "{} Semiotic Triangle"
        R["{}<br/>Representamen"]
        O["{}<br/>Object"]
        I["{}<br/>Interpretant"]
        R -->|"stands for"| O
        R -->|"grounds"| I
        O -->|"determines"| I
        I -->|"represents"| R
    end
"#,
        name, representamen, object, interpretant
    )
}

/// Renders a Greimas semiotic square (S / ~S / S' / ~S') as a Mermaid flowchart.
pub fn format_greimas_square(
    title: &str,
    s: &str,
    not_s: &str,
    s_prime: &str,
    not_s_prime: &str,
) -> String {
    format!(
        r#"flowchart LR
    subgraph "{} — Greimas Square"
        S["{}<br/>(S)"] --- N_S["{}<br/>(~S)"]
        SP["{}<br/>(S′)"] --- N_SP["{}<br/>(~S′)"]
        S ===|"contrariety"| N_S
        SP ===|"contrariety"| N_SP
        S -->|"implication"| SP
        N_S -->|"implication"| N_SP
        S -.->|"negation"| N_SP
        N_S -.->|"negation"| SP
    end
"#,
        title, s, not_s, s_prime, not_s_prime
    )
}

/// Renders a category-theory diagram (objects + morphisms) as a Mermaid flowchart.
pub fn format_category_diagram(objects: &[(&str, &str)], morphisms: &[(&str, &str, &str)]) -> String {
    let mut out = String::from("flowchart LR\n");
    for (id, label) in objects {
        out.push_str(&format!("    {}[\"{}\"]\n", id, label));
    }
    for (from, to, label) in morphisms {
        out.push_str(&format!("    {} -->|\"{}\"| {}\n", from, label, to));
    }
    out
}

/// Renders the semiotic grid activation heatmap as text table.
pub fn format_heatmap_table(grid: &SemioticGrid) -> String {
    // `{:<W}` sets a MINIMUM width, so anything longer than the column simply
    // overflowed and pushed the rest of the line right: "BRAINSTORM" (10) ran
    // into "MAINTAIN" as "BRAINSTORMMAINTAIN", and a full-length bar did the
    // same to the data rows, leaving the header and the grid misaligned.
    // Every cell is now clipped to the column as well as padded to it.
    const LABEL_W: usize = 14;
    // 9 leaves 8 usable characters, which fits every mode name except
    // BRAINSTORM (10). That one clips to "BRAINSTO", still unique among the
    // fourteen — the header only has to identify its column, not spell it.
    const COL_W: usize = 9;
    // Every cell reserves one trailing space, so neither a maximal bar nor a
    // clipped header can touch its neighbour — "BRAINSTORM" truncated to the
    // full 8 sat flush against "MAINTAIN" and read as one word.
    const BAR_W: usize = COL_W - 1;

    fn fit(s: &str, w: usize) -> String {
        let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
        let pad = w.saturating_sub(t.chars().count());
        t.extend(std::iter::repeat_n(' ', pad));
        t
    }

    // Trailing padding on the right edge is invisible noise in a terminal and
    // shows up as whitespace churn in any file this gets written to.
    fn trim_end(out: &mut String) {
        while out.ends_with(' ') {
            out.pop();
        }
    }

    let mut out = String::new();
    out.push_str("## Semiotic Grid Activation\n\n");
    out.push_str("```\n");
    out.push_str(&fit("", LABEL_W));
    for m in HumanMode::all() {
        out.push_str(&fit(m.as_str(), COL_W));
    }
    trim_end(&mut out);
    out.push('\n');
    for d in HumanDomain::all() {
        out.push_str(&fit(d.as_str(), LABEL_W));
        for m in HumanMode::all() {
            let bar = match grid.get_cell(d, m) {
                // Scale to the column that draws it: the old constant of 20 was
                // unrelated to the 8-wide column and guaranteed overflow.
                Some(cell) => "█".repeat(
                    ((cell.activation.clamp(0.0, 1.0) * BAR_W as f32).round() as usize).min(BAR_W),
                ),
                None => String::new(),
            };
            out.push_str(&fit(&bar, COL_W));
        }
        trim_end(&mut out);
        out.push('\n');
    }
    out.push_str("```\n");
    out
}

/// Serialize the entire semiotic grid as JSON.
pub fn format_semiotic_grid_json(grid: &SemioticGrid) -> String {
    serde_json::to_string_pretty(grid).unwrap_or_else(|_| "{}".to_string())
}

/// Renders the 36-cell grid as Mermaid mindmap.
pub fn format_semiotic_mindmap(grid: &SemioticGrid) -> String {
    let mut out = String::new();
    out.push_str("mindmap\n  root((Semiotic Grid))\n");
    for d in HumanDomain::all() {
        out.push_str(&format!("    {}_cell\n", d.as_str()));
        for m in HumanMode::all() {
            if let Some(cell) = grid.get_cell(d, m) {
                let activation_bar = "█".repeat(((cell.activation * 10.0) as usize).min(10));
                out.push_str(&format!("      {} {} {}\n", m.as_str(), cell.entries.len(), activation_bar));
            }
        }
    }
    out
}

/// Renders domain counts per cell as structured data.
pub fn format_domain_matrix(domains: &HashMap<String, DomainDef>) -> String {
    let mut out = String::new();
    out.push_str("## Domain Distribution Matrix\n\n");
    let mut counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for def in domains.values() {
        let d = def.domain.as_deref().unwrap_or("unknown").to_string();
        let m = def.mode.as_deref().unwrap_or("unknown").to_string();
        *counts.entry(d).or_default().entry(m).or_default() += 1;
    }
    out.push_str(&format!("{:<14}", ""));
    for m in HumanMode::all() {
        out.push_str(&format!("{:<8}", m.as_str()));
    }
    out.push('\n');
    for d in HumanDomain::all() {
        out.push_str(&format!("{:<14}", d.as_str()));
        for m in HumanMode::all() {
            let count = counts.get(d.as_str()).and_then(|c| c.get(m.as_str())).copied().unwrap_or(0);
            out.push_str(&format!("{:<8}", count));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PhysisConfig;
    use crate::config::OntologyLoader;

    /// `{:<8}` is a minimum width, not a maximum, so any value wider than the
    /// column overflowed and shifted the rest of the line: the header read
    /// "BRAINSTORMMAINTAINMOVE" and a full-length activation bar knocked its
    /// row out of alignment with the header above it.
    #[test]
    fn heatmap_columns_stay_aligned_for_long_headers_and_full_bars() {
        let mut grid = SemioticGrid::new();
        // Drive one cell to full activation; `classify` adds 0.1 each call.
        for _ in 0..15 {
            grid.classify("x", HumanDomain::Fabricate, HumanMode::Brainstorm);
        }
        let table = format_heatmap_table(&grid);
        let body: Vec<&str> = table
            .lines()
            .skip_while(|l| !l.starts_with("```"))
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect();
        let header = body[0];

        // The widest mode name is 10 chars in an 8-wide column, so it must be
        // clipped rather than allowed to run into its neighbour.
        assert!(
            !header.contains("BRAINSTORMMAINTAIN"),
            "headers must not collide: {header:?}",
        );
        assert!(header.contains(" MAINTAIN"), "later headers survive: {header:?}");

        // A header only has to identify its column, so clipping is fine as long
        // as no two columns end up spelled the same.
        let names: Vec<&str> = header.split_whitespace().collect();
        assert_eq!(names.len(), HumanMode::all().len(), "one header per mode: {names:?}");
        let mut uniq = names.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), names.len(), "clipped headers collide: {names:?}");

        // Every row occupies the same grid, so a column index means one thing.
        let expected = 14 + 9 * HumanMode::all().len();
        assert!(header.chars().count() <= expected, "header width");
        for row in &body[1..] {
            assert!(
                row.chars().count() <= expected,
                "row overflows the grid ({} > {expected}): {row:?}",
                row.chars().count(),
            );
            assert!(!row.ends_with(' '), "rows must not carry trailing padding");
        }

        // And the saturated bar is still drawn, just inside its column.
        assert!(body[1..].iter().any(|r| r.contains('\u{2588}')), "no bar drawn");
    }

    #[test]
    fn test_format_wiki() {
        let trie = DynamicVectorTrie::new();
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let goals = vec![Goal::new_vec(embedding)];
        let wiki = format_wiki(&trie, &goals, "Test Wiki");
        assert!(wiki.contains("Test Wiki"));
    }

    #[test]
    fn test_format_wiki_goals_collapse() {
        // Untracked (0%) goals collapse to one count line; tracked ones stay.
        let mut goals = vec![
            Goal::new_vec(vec![0.0]),
            Goal::new_vec(vec![0.0]),
            Goal::new_vec(vec![0.0]),
        ];
        goals[2].progress = 0.5;
        let tracked_id = goals[2].id.clone();
        let wiki = format_wiki(&DynamicVectorTrie::new(), &goals, "T");
        let section = wiki.split("## Goals").nth(1).unwrap()
            .split("## Ontology Tree").next().unwrap();
        assert!(section.contains(&format!("[[{tracked_id}]]")), "tracked goal missing");
        assert!(section.contains("2 entries mapped"), "untracked not collapsed: {section}");
        // The two 0% goals must NOT be listed individually.
        assert_eq!(section.matches("progress:").count(), 1, "0% goals not collapsed");
    }

    #[test]
    fn test_format_wiki_ontology_tree() {
        // Ontology Tree must be a real recursive walk, not a flat 3-node slice.
        let mut trie = DynamicVectorTrie::new();
        trie.insert_path("alpha/beta/gamma");
        trie.insert_path("alpha/delta");
        let wiki = format_wiki(&trie, &[], "T");
        let tree = wiki
            .split("## Ontology Tree").nth(1).unwrap()
            .split("## Stats").next().unwrap();
        // All four path tokens should appear, at increasing indentation depth.
        for tok in ["alpha", "beta", "gamma", "delta"] {
            assert!(tree.contains(tok), "tree missing token {tok}: {tree}");
        }
        // Nested token must be indented deeper than its ancestor (real tree, not flat).
        let a = tree.lines().find(|l| l.trim() == "alpha").unwrap();
        let g = tree.lines().find(|l| l.trim() == "gamma").unwrap();
        let indent = |l: &str| l.len() - l.trim_start().len();
        assert!(indent(g) > indent(a), "gamma not nested under alpha");
    }

    #[test]
    fn test_format_mermaid() {
        let mut trie = DynamicVectorTrie::new();
        trie.insert_path("a/b/c");
        let mm = format_mermaid_mindmap(&trie, "Test");
        assert!(mm.contains("mindmap"));
    }

    #[test]
    fn test_domain_report() {
        let config = PhysisConfig::default();
        let loader = OntologyLoader::load_all(&config);
        let goals = vec![];
        let grades = HashMap::new();
        let report = format_domain_report(&goals, &loader.human_domains, &grades);
        assert!(report.contains("Domain Report"));
    }

    #[test]
    fn test_format_json_graph() {
        let mut trie = DynamicVectorTrie::new();
        trie.insert_path("a/b/c");
        let json_str = format_json_graph(&trie);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let entities = parsed["entities"].as_array().unwrap();
        let _relations = parsed["relations"].as_array().unwrap();
        assert!(!entities.is_empty(), "should have at least one entity");
    }

    #[test]
    fn test_format_json_graph_empty() {
        let trie = DynamicVectorTrie::new();
        let json_str = format_json_graph(&trie);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed["entities"].as_array().unwrap().is_empty());
        assert!(parsed["relations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_format_mermaid_flowchart() {
        let mut trie = DynamicVectorTrie::new();
        trie.insert_path("a/b/c");
        let fc = format_mermaid_flowchart(&trie);
        assert!(fc.contains("flowchart"));
        assert!(fc.contains("-->"));
    }
}
