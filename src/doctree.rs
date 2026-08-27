//! Phase 11 — hierarchical document index (PageIndex-shaped, deterministic).
//!
//! Flat chunking loses the thing that makes a chunk mean anything: where it
//! sits in the document. "Torque to 45 Nm" is unusable without the heading
//! chain "Spindle Assembly > Reassembly > Bearing preload" above it, and a
//! flat chunker throws that chain away. In a manual, a standard or a
//! procedure — the documents the industrial wedge actually ingests — that
//! chain *is* the meaning.
//!
//! This module parses a document into its own heading tree and makes each
//! section carry its ancestors. The retrieval unit becomes
//! `breadcrumb + body`, so the embedding sees the context the author implied
//! but never repeated, and a citation can name the section instead of an
//! opaque chunk offset.
//!
//! Deterministic and dependency-free: no model is consulted to find structure,
//! the document's own headings are the structure. Documents with no headings
//! degrade to a single whole-document section rather than failing.

use serde::{Deserialize, Serialize};

/// One section of a document: a heading, its own text, and its place in the tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Index into the returned `Vec<Section>`. Stable for a given input.
    pub id: usize,
    /// Enclosing section, `None` for top level.
    pub parent: Option<usize>,
    /// Heading depth 1..=6. Level 0 is the synthetic document-preamble section.
    pub level: u8,
    /// Heading text, or the document title for the preamble.
    pub heading: String,
    /// Text belonging to this heading, excluding descendants' text.
    pub body: String,
    /// Ancestor headings, outermost first, ending with this section's heading.
    pub breadcrumb: Vec<String>,
}

impl Section {
    /// The unit that gets embedded and cited: the heading chain, then the body.
    ///
    /// The chain is repeated into the text on purpose — the embedder only ever
    /// sees this string, so context that is not in it cannot influence the
    /// match.
    pub fn retrieval_text(&self) -> String {
        let path = self.breadcrumb.join(" > ");
        if self.body.trim().is_empty() {
            path
        } else {
            format!("{path}\n\n{}", self.body.trim())
        }
    }

    /// Human-readable location, for citations.
    pub fn path(&self) -> String {
        self.breadcrumb.join(" > ")
    }

    /// True for the synthetic section holding text before the first heading.
    pub fn is_preamble(&self) -> bool {
        self.level == 0
    }
}

/// A heading found by the scanner.
struct Heading {
    line: usize,
    level: u8,
    text: String,
    /// Lines consumed by the heading itself (1 for ATX, 2 for setext).
    span: usize,
}

/// Parse `text` into a section tree. `doc_title` names the synthetic preamble
/// section and roots the breadcrumb of documents whose headings all start
/// below level 1.
///
/// Guarantees:
/// - every line of the input belongs to exactly one section (no text is lost);
/// - a document with no headings yields exactly one section;
/// - output is deterministic for a given input.
pub fn parse(doc_title: &str, text: &str) -> Vec<Section> {
    let lines: Vec<&str> = text.lines().collect();
    let start = skip_frontmatter(&lines);
    let headings = scan_headings(&lines, start);

    // No structure at all: the document is one section. Returning an empty vec
    // here would silently drop the document from every downstream index.
    if headings.is_empty() {
        let body = lines[start..].join("\n");
        return vec![Section {
            id: 0,
            parent: None,
            level: 0,
            heading: doc_title.to_string(),
            body,
            breadcrumb: vec![doc_title.to_string()],
        }];
    }

    let mut sections: Vec<Section> = Vec::new();

    // Text before the first heading becomes a preamble section, but only when
    // it actually holds something — an empty preamble is noise in the tree.
    let first = headings[0].line;
    let preamble = lines[start..first].join("\n");
    if !preamble.trim().is_empty() {
        sections.push(Section {
            id: 0,
            parent: None,
            level: 0,
            heading: doc_title.to_string(),
            body: preamble,
            breadcrumb: vec![doc_title.to_string()],
        });
    }

    // Stack of (section id, level) of the currently open ancestors.
    let mut stack: Vec<(usize, u8)> = Vec::new();

    for (i, h) in headings.iter().enumerate() {
        let body_start = h.line + h.span;
        let body_end = headings.get(i + 1).map(|n| n.line).unwrap_or(lines.len());
        let body = if body_start < body_end {
            lines[body_start..body_end].join("\n")
        } else {
            String::new()
        };

        // Pop ancestors that this heading is not nested under. A document that
        // jumps H1 → H3 keeps the H3 under the H1 rather than inventing an H2.
        while stack.last().is_some_and(|&(_, lvl)| lvl >= h.level) {
            stack.pop();
        }
        let parent = stack.last().map(|&(id, _)| id);

        let mut breadcrumb: Vec<String> = match parent {
            Some(p) => sections[p].breadcrumb.clone(),
            // Top-level headings still get the document name at the root so a
            // breadcrumb always identifies which document it came from.
            None => vec![doc_title.to_string()],
        };
        breadcrumb.push(h.text.clone());

        let id = sections.len();
        sections.push(Section {
            id,
            parent,
            level: h.level,
            heading: h.text.clone(),
            body,
            breadcrumb,
        });
        stack.push((id, h.level));
    }

    sections
}

/// Skip a leading YAML frontmatter block. Returns the first content line.
fn skip_frontmatter(lines: &[&str]) -> usize {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return 0;
    }
    for (i, l) in lines.iter().enumerate().skip(1) {
        if l.trim_end() == "---" {
            return i + 1;
        }
    }
    // Unterminated frontmatter: treat the whole thing as content rather than
    // swallowing the document.
    0
}

/// Find ATX (`## x`) and setext (`x` over `===`) headings, ignoring anything
/// inside a fenced code block — a `# comment` in a shell snippet is not a
/// heading, and treating it as one shreds the tree of every technical document.
fn scan_headings(lines: &[&str], start: usize) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        // Fence toggling. A fence closes only on its own marker character, so
        // a ``` inside a ~~~ block does not end it.
        if let Some(marker) = fence {
            if trimmed.starts_with(marker)
                && trimmed.chars().take_while(|&c| c == marker).count() >= 3
            {
                fence = None;
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some('`');
            i += 1;
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some('~');
            i += 1;
            continue;
        }

        // ATX: 1..6 '#' followed by a space. Without the space, `#hashtag` and
        // `#!/bin/sh` would both parse as headings.
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
            let text = trimmed[hashes + 1..].trim().trim_end_matches('#').trim();
            if !text.is_empty() {
                out.push(Heading {
                    line: i,
                    level: hashes as u8,
                    text: text.to_string(),
                    span: 1,
                });
            }
            i += 1;
            continue;
        }

        // Setext: a non-empty line underlined by === (H1) or --- (H2).
        if let Some(next) = lines.get(i + 1) {
            let underline = next.trim();
            let is_eq = underline.len() >= 3 && underline.chars().all(|c| c == '=');
            let is_dash = underline.len() >= 3 && underline.chars().all(|c| c == '-');
            if (is_eq || is_dash) && !trimmed.is_empty() && !trimmed.starts_with('#') {
                out.push(Heading {
                    line: i,
                    level: if is_eq { 1 } else { 2 },
                    text: trimmed.trim().to_string(),
                    span: 2,
                });
                i += 2;
                continue;
            }
        }

        i += 1;
    }

    out
}

/// Render the tree as an indented table of contents — the "index page" a
/// navigating reader (human or model) reads before choosing where to descend.
pub fn outline(sections: &[Section]) -> String {
    let mut s = String::new();
    for sec in sections {
        if sec.is_preamble() {
            continue;
        }
        let indent = "  ".repeat(sec.level.saturating_sub(1) as usize);
        s.push_str(&format!("{indent}- {}\n", sec.heading));
    }
    s
}

/// Deepest heading level present, 0 for a flat document.
pub fn max_depth(sections: &[Section]) -> u8 {
    sections.iter().map(|s| s.level).max().unwrap_or(0)
}

/// Direct children of `id`.
pub fn children(sections: &[Section], id: usize) -> Vec<&Section> {
    sections.iter().filter(|s| s.parent == Some(id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_follows_heading_levels() {
        let doc = "# Machine\nintro\n\n## Spindle\nspindle text\n\n### Bearing\nbearing text\n\n## Coolant\ncoolant text\n";
        let s = parse("manual.md", doc);
        let by_heading = |h: &str| s.iter().find(|x| x.heading == h).unwrap();

        assert_eq!(by_heading("Machine").parent, None);
        assert_eq!(by_heading("Spindle").parent, Some(by_heading("Machine").id));
        assert_eq!(by_heading("Bearing").parent, Some(by_heading("Spindle").id));
        assert_eq!(by_heading("Coolant").parent, Some(by_heading("Machine").id));
    }

    #[test]
    fn breadcrumb_carries_the_ancestor_chain_and_the_document() {
        let doc = "# Machine\n\n## Spindle\n\n### Bearing\ntorque to 45 Nm\n";
        let s = parse("manual.md", doc);
        let bearing = s.iter().find(|x| x.heading == "Bearing").unwrap();
        assert_eq!(
            bearing.breadcrumb,
            vec!["manual.md", "Machine", "Spindle", "Bearing"]
        );
        assert!(bearing
            .retrieval_text()
            .contains("manual.md > Machine > Spindle > Bearing"));
        assert!(bearing.retrieval_text().contains("torque to 45 Nm"));
    }

    #[test]
    fn headings_inside_code_fences_are_not_headings() {
        let doc = "# Real\ntext\n\n```sh\n# not a heading\necho hi\n```\n\n## Also Real\nmore\n";
        let s = parse("d.md", doc);
        let headings: Vec<&str> = s.iter().map(|x| x.heading.as_str()).collect();
        assert_eq!(headings, vec!["Real", "Also Real"], "{headings:?}");
    }

    #[test]
    fn tilde_fence_is_not_closed_by_a_backtick_fence() {
        let doc = "# Real\n~~~\n# inside\n```\n# still inside\n~~~\n## After\n";
        let s = parse("d.md", doc);
        let headings: Vec<&str> = s.iter().map(|x| x.heading.as_str()).collect();
        assert_eq!(headings, vec!["Real", "After"], "{headings:?}");
    }

    #[test]
    fn shebang_and_hashtag_are_not_headings() {
        let doc = "#!/bin/sh\n#hashtag\n# Real Heading\nbody\n";
        let s = parse("d.md", doc);
        let headings: Vec<&str> = s
            .iter()
            .filter(|x| !x.is_preamble())
            .map(|x| x.heading.as_str())
            .collect();
        assert_eq!(headings, vec!["Real Heading"], "{headings:?}");
        // The two rejected lines are not headings, but they are not lost
        // either — they fall into the preamble section.
        let preamble = s.iter().find(|x| x.is_preamble()).unwrap();
        assert!(preamble.body.contains("#!/bin/sh"));
        assert!(preamble.body.contains("#hashtag"));
    }

    #[test]
    fn setext_headings_are_recognized() {
        let doc = "Title\n=====\nintro\n\nSection\n-------\nbody\n";
        let s = parse("d.md", doc);
        let title = s.iter().find(|x| x.heading == "Title").unwrap();
        let section = s.iter().find(|x| x.heading == "Section").unwrap();
        assert_eq!(title.level, 1);
        assert_eq!(section.level, 2);
        assert_eq!(section.parent, Some(title.id));
        assert!(title.body.contains("intro"));
        assert!(
            !title.body.contains("====="),
            "underline must not leak into body"
        );
    }

    #[test]
    fn frontmatter_is_skipped_not_indexed() {
        let doc = "---\ntitle: X\ntags: [a]\n---\n\n# Real\nbody\n";
        let s = parse("d.md", doc);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].heading, "Real");
        assert!(!s[0].body.contains("tags"));
    }

    #[test]
    fn unterminated_frontmatter_keeps_the_document() {
        let doc = "---\ntitle: X\n\n# Real\nbody\n";
        let s = parse("d.md", doc);
        assert!(s.iter().any(|x| x.heading == "Real"), "{s:?}");
    }

    #[test]
    fn preamble_becomes_a_section_only_when_it_has_text() {
        let with = parse("d.md", "loose intro text\n\n# First\nbody\n");
        assert!(with[0].is_preamble());
        assert!(with[0].body.contains("loose intro"));

        let without = parse("d.md", "# First\nbody\n");
        assert!(!without.iter().any(|x| x.is_preamble()), "{without:?}");
    }

    #[test]
    fn document_without_headings_is_one_section_not_zero() {
        let s = parse("notes.txt", "just a flat note\nwith two lines\n");
        assert_eq!(s.len(), 1);
        assert!(s[0].is_preamble());
        assert!(s[0].body.contains("two lines"));
        assert_eq!(s[0].breadcrumb, vec!["notes.txt"]);
    }

    #[test]
    fn empty_document_still_yields_one_section() {
        let s = parse("empty.md", "");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].heading, "empty.md");
    }

    #[test]
    fn skipped_level_does_not_invent_an_intermediate() {
        let doc = "# One\n\n### Three\nbody\n";
        let s = parse("d.md", doc);
        let three = s.iter().find(|x| x.heading == "Three").unwrap();
        assert_eq!(three.level, 3);
        assert_eq!(
            three.parent,
            Some(s.iter().find(|x| x.heading == "One").unwrap().id)
        );
        assert_eq!(s.len(), 2, "no synthetic H2 may appear: {s:?}");
    }

    #[test]
    fn body_excludes_descendant_text() {
        let doc = "# Parent\nparent body\n\n## Child\nchild body\n";
        let s = parse("d.md", doc);
        let parent = s.iter().find(|x| x.heading == "Parent").unwrap();
        assert!(parent.body.contains("parent body"));
        assert!(!parent.body.contains("child body"));
    }

    #[test]
    fn every_content_line_lands_in_exactly_one_section() {
        let doc = "intro line\n\n# A\na body\n\n## B\nb body\n\n# C\nc body\n";
        let s = parse("d.md", doc);
        let joined: String = s
            .iter()
            .map(|x| x.body.clone())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in ["intro line", "a body", "b body", "c body"] {
            assert!(joined.contains(needle), "lost {needle}");
        }
    }

    #[test]
    fn outline_indents_by_level_and_skips_preamble() {
        let doc = "intro\n\n# One\n\n## Two\n\n### Three\n";
        let s = parse("d.md", doc);
        let o = outline(&s);
        assert!(o.contains("- One"));
        assert!(o.contains("  - Two"));
        assert!(o.contains("    - Three"));
        assert!(!o.contains("d.md"), "preamble is not an outline entry: {o}");
    }

    #[test]
    fn children_and_depth_report_the_tree() {
        let doc = "# One\n\n## A\n\n## B\n\n### C\n";
        let s = parse("d.md", doc);
        let one = s.iter().find(|x| x.heading == "One").unwrap();
        let kids: Vec<&str> = children(&s, one.id)
            .iter()
            .map(|x| x.heading.as_str())
            .collect();
        assert_eq!(kids, vec!["A", "B"]);
        assert_eq!(max_depth(&s), 3);
    }

    #[test]
    fn trailing_hashes_are_stripped_from_headings() {
        let s = parse("d.md", "## Closed Heading ##\nbody\n");
        assert_eq!(s[0].heading, "Closed Heading");
    }

    #[test]
    fn deterministic_across_calls() {
        let doc = "# A\nx\n\n## B\ny\n\n### C\nz\n";
        assert_eq!(parse("d.md", doc), parse("d.md", doc));
    }

    #[test]
    fn heading_only_section_retrieval_text_is_just_the_path() {
        let s = parse("d.md", "# Only\n");
        let only = s.iter().find(|x| x.heading == "Only").unwrap();
        assert_eq!(only.retrieval_text(), "d.md > Only");
    }
}
