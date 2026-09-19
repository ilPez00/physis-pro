//! Text dashboard: the same content the HUD shows, rendered for a terminal.
//!
//! The HUD and this share one rule — logic builds strings, a frontend prints
//! them. Every section here is produced by the same renderer the TUI calls, so
//! the CLI and the TUI cannot drift apart.

/// One titled block of already-rendered text.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: String,
    pub body: String,
}

impl Section {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }
}

/// Render sections under a title, clamped to `width` columns. An empty body is
/// shown as `(empty)` rather than vanishing: a missing section is information.
pub fn render(title: &str, sections: &[Section], width: usize) -> String {
    let width = width.clamp(40, 200);
    let rule = "─".repeat(width);
    let mut out = String::new();
    out.push_str(title);
    out.push('\n');
    out.push_str(&rule);
    out.push('\n');
    for (i, s) in sections.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{}\n", s.title.to_uppercase()));
        if s.body.trim().is_empty() {
            out.push_str("  (empty)\n");
        } else {
            for line in s.body.lines() {
                out.push_str(&truncate(line, width));
                out.push('\n');
            }
        }
    }
    out
}

/// Truncate to `width` columns, marking the cut. Counts `char`s, not bytes, so
/// a multi-byte marker in a path does not split mid-character.
fn truncate(line: &str, width: usize) -> String {
    if line.chars().count() <= width {
        return line.to_string();
    }
    let keep = width.saturating_sub(1);
    let mut s: String = line.chars().take(keep).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_every_section_and_marks_an_empty_one() {
        let out = render(
            "physis",
            &[
                Section::new("agents", "AGENTS: JEV ●"),
                Section::new("tasks", ""),
            ],
            60,
        );
        assert!(out.starts_with("physis\n"));
        assert!(out.contains("AGENTS\n"));
        assert!(out.contains("AGENTS: JEV ●"));
        assert!(out.contains("TASKS\n"));
        assert!(
            out.contains("(empty)"),
            "an empty section is shown, not dropped"
        );
    }

    #[test]
    fn truncates_long_lines_by_characters_not_bytes() {
        let out = render("t", &[Section::new("s", "é".repeat(100).as_str())], 40);
        for line in out.lines() {
            assert!(line.chars().count() <= 40, "line too long: {line:?}");
        }
        assert!(out.contains('…'));
    }
}
