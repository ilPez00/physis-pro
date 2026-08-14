use anyhow::{anyhow, Result};
use gray_matter::{engine::YAML, Matter};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PageMeta {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub confidence: Option<f64>,
    pub sources: Option<Vec<String>>,
    pub updated: Option<String>,
}

pub fn parse_frontmatter(content: &str) -> Result<PageMeta> {
    let matter = Matter::<YAML>::new();
    let result = matter.parse(content);
    let pod = result
        .data
        .ok_or_else(|| anyhow!("No frontmatter found"))?;

    let meta: PageMeta = pod
        .deserialize()
        .map_err(|e| anyhow!("Failed to deserialize frontmatter: {}", e))?;

    if meta.title.is_empty() {
        return Err(anyhow!("Frontmatter missing required field: title"));
    }
    if meta.description.is_empty() {
        return Err(anyhow!("Frontmatter missing required field: description"));
    }

    Ok(meta)
}

#[allow(dead_code)]
pub fn strip_frontmatter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    // Find the closing ---
    let after_open = &content[3..];
    if let Some(close_pos) = after_open.find("\n---") {
        let end = 3 + close_pos + 4; // skip past "\n---"
        // Skip the newline after the closing fence
        let rest = &content[end..];
        if rest.starts_with('\n') {
            &content[end + 1..]
        } else {
            rest
        }
    } else {
        content
    }
}

#[allow(dead_code)]
pub fn extract_section(content: &str, heading: &str) -> Option<String> {
    let heading_lower = heading.to_lowercase();
    let opts = Options::empty();
    let parser = Parser::new_ext(content, opts);

    let events: Vec<Event> = parser.collect();

    let mut target_level: Option<HeadingLevel> = None;
    let mut in_heading = false;
    let mut collecting = false;
    let mut heading_text = String::new();
    let mut result = String::new();

    'outer: for event in &events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                if collecting {
                    if let Some(tl) = target_level {
                        if heading_level_num(*level) <= heading_level_num(tl) {
                            break 'outer;
                        }
                    }
                } else {
                    in_heading = true;
                    target_level = Some(*level);
                    heading_text.clear();
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if in_heading {
                    in_heading = false;
                    if heading_text.to_lowercase().contains(&heading_lower) {
                        collecting = true;
                    } else {
                        target_level = None;
                    }
                    heading_text.clear();
                }
            }
            Event::Text(text) => {
                if in_heading {
                    heading_text.push_str(text);
                } else if collecting {
                    result.push_str(text);
                    result.push(' ');
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if collecting {
                    result.push('\n');
                }
            }
            _ => {}
        }
    }

    if !result.is_empty() {
        Some(result.trim().to_string())
    } else {
        None
    }
}

fn heading_level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"---
title: Test Page
description: A test description for the page.
tags: [rust, testing]
confidence: 0.9
---

# Test Page

Some intro text.

## Overview

Overview content here.

## Details

Details content here.
"#;

    #[test]
    fn test_parse_frontmatter() {
        let meta = parse_frontmatter(SAMPLE).unwrap();
        assert_eq!(meta.title, "Test Page");
        assert_eq!(meta.tags, vec!["rust", "testing"]);
        assert!((meta.confidence.unwrap() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_strip_frontmatter() {
        let stripped = strip_frontmatter(SAMPLE);
        assert!(stripped.contains("# Test Page"));
        assert!(!stripped.contains("title:"));
    }

    #[test]
    fn test_extract_section() {
        let section = extract_section(SAMPLE, "overview");
        assert!(section.is_some());
        assert!(section.unwrap().contains("Overview content"));
    }

    #[test]
    fn test_parse_missing_title() {
        let bad = "---\ndescription: foo\n---\n";
        assert!(parse_frontmatter(bad).is_err());
    }
}
