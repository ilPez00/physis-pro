//! Personal-history importers (Phase 7B) — browser bookmarks/history, RSS /
//! read-later lists, and chat/message exports reduced to [`VaultDoc`]s.
//!
//! Pure parsers only: each takes a text export and returns docs; callers
//! register them as labeled core nodes with the shared embedder (the same
//! "source → embed → register labeled node" connector as vault/watch). This
//! keeps a person's life-log recallable by content in the wiki.
//!
//! CLI: `physis-pro history <file>` — web: `POST /api/v1/history/import`.

use std::path::Path;

use crate::vault::VaultDoc;

/// Pick the importer for a file by extension, falling back to well-known
/// dotfile names (shell histories carry no usable extension). Returns `None`
/// when the file is not a recognised personal-history export.
pub fn importer_for(path: &Path) -> Option<fn(&str) -> Vec<VaultDoc>> {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_lowercase();
        if lower == ".zsh_history" || lower == ".bash_history" || lower == ".sh_history" {
            return Some(parse_shell_history);
        }
        if lower.ends_with(".jsonl") && looks_like_claude_session_hint(name) {
            return Some(parse_claude_session);
        }
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    match ext.as_str() {
        "html" | "htm" => Some(parse_bookmarks_html),
        "json" => Some(parse_history_json),
        "opml" | "xml" => Some(parse_opml),
        "txt" | "jsonl" | "ndjson" => Some(parse_messages_jsonl),
        _ => None,
    }
}

/// Filename heuristic for Claude session exports: 36-char UUID stems
/// (`03273da0-…​.jsonl`) as written under `~/.claude/projects/`. The generic
/// message parser would yield zero docs on these (different schema), so route
/// them to the session parser instead of silently importing nothing.
pub(crate) fn looks_like_claude_session_hint(name: &str) -> bool {
    let stem = name.trim_end_matches(".jsonl").trim_end_matches(".JSONL");
    stem.len() == 36
        && stem.bytes().filter(|&b| b == b'-').count() == 4
        && stem
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// Netscape bookmark file: `<!DOCTYPE NETSCAPE-Bookmark-file-1>` — the export
/// format shared by Firefox and Chromium. Each `<A HREF="…">Title</A>` becomes
/// a doc (title = link text, body = URL). Folder lines give context but the
/// links are the retrievable units.
pub fn parse_bookmarks_html(html: &str) -> Vec<VaultDoc> {
    let mut docs = Vec::new();
    let mut depth = 0usize;
    let mut stack: Vec<String> = Vec::new();
    for line in html.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<DT><H3") || trimmed.starts_with("<DT>") && trimmed.contains("<H3")
        {
            // Folder heading, e.g. <DT><H3 ADD_DATE="…">Name</H3>
            if let Some(open) = trimmed.find('>') {
                if let Some(close) = trimmed[open..].find("</H3>") {
                    let name = trimmed[open + 1..open + close].trim();
                    stack.push(name.to_string());
                    depth += 1;
                }
            }
            continue;
        }
        if trimmed.starts_with("</DL>") || trimmed.starts_with("</DL><p>") {
            depth = depth.saturating_sub(1);
            stack.pop();
            continue;
        }
        if let Some(a_start) = trimmed.find("<A HREF=\"") {
            let rest = &trimmed[a_start + "<A HREF=\"".len()..];
            let url = rest.split('"').next().unwrap_or_default().to_string();
            if url.is_empty() {
                continue;
            }
            // Title is text up to </A> (may span the line).
            let after_url = rest.split_once('>').map(|x| x.1).unwrap_or_default();
            let title = after_url
                .split("</A>")
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            let title = if title.is_empty() { url.clone() } else { title };
            let mut body = url;
            if let Some(folder) = stack.last() {
                body.push_str(&format!("\n\nbookmarked in: {folder}"));
            }
            docs.push(VaultDoc {
                source: format!("bookmark {title}"),
                title,
                body,
                headings: vec![],
            });
        }
    }
    docs
}

/// Browser history as a JSON array: `[{ "title": "…", "url": "…", "lastVisitTime": … }]`
/// (Chromium's `History` export / common JSON dumps) — or an object with an
/// `items` array. Each visited page becomes a doc (title, body = URL + title).
pub fn parse_history_json(json: &str) -> Vec<VaultDoc> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let items = match &v {
        serde_json::Value::Array(a) => a.as_slice(),
        serde_json::Value::Object(o) => match o.get("items").and_then(|x| x.as_array()) {
            Some(a) => a.as_slice(),
            None => return vec![],
        },
        _ => return vec![],
    };
    let mut docs = Vec::new();
    for it in items {
        let url = it
            .get("url")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        if url.is_empty() {
            continue;
        }
        let title = it
            .get("title")
            .and_then(|x| x.as_str())
            .filter(|t| !t.is_empty())
            .unwrap_or(&url)
            .to_string();
        docs.push(VaultDoc {
            source: format!("history {url}"),
            title: title.clone(),
            body: format!("{title}\n\n{url}"),
            headings: vec![],
        });
    }
    docs
}

/// OPML feed list (RSS readers / read-later services): `<outline type="rss"
/// text="Name" title="Name" xmlUrl="…" htmlUrl="…" />`. Each feed becomes a doc
/// (title = text/title, body = xmlUrl + htmlUrl). Folders (outlines without
/// xmlUrl) track nesting so a feed's body names its folder.
pub fn parse_opml(xml: &str) -> Vec<VaultDoc> {
    let mut docs = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Pop on every closing outline (also "</outline>" alone on a line).
        let closes = trimmed.matches("</outline").count();
        if closes > 0 {
            for _ in 0..closes {
                stack.pop();
            }
        }
        if trimmed.contains("<outline") {
            let self_closing = trimmed.ends_with("/>");
            let is_feed = trimmed.contains("xmlUrl");
            let title = attr(trimmed, "text")
                .or_else(|| attr(trimmed, "title"))
                .unwrap_or_default();

            if is_feed {
                let xml_url = attr(trimmed, "xmlUrl").unwrap_or_default();
                let html_url = attr(trimmed, "htmlUrl").unwrap_or_default();
                let mut body = xml_url;
                if !html_url.is_empty() {
                    body.push_str(&format!("\n\npage: {html_url}"));
                }
                if let Some(folder) = stack.last() {
                    body.push_str(&format!("\n\nfeed in: {folder}"));
                }
                docs.push(VaultDoc {
                    source: format!("feed {title}"),
                    title,
                    body,
                    headings: vec![],
                });
            } else if !title.is_empty() && !self_closing {
                // Folder opener that will be closed by a later </outline>.
                stack.push(title);
            }
        }
    }
    docs
}

/// Chat/message export as JSON lines (WhatsApp/Telegram style):
/// one object per message `{ "from"/"sender": "…", "text"/"message": "…" }`.
/// Each message becomes a doc; the body carries who said it.
pub fn parse_messages_jsonl(input: &str) -> Vec<VaultDoc> {
    let mut docs = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let text = v
            .get("text")
            .or_else(|| v.get("message"))
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .trim();
        if text.is_empty() {
            continue;
        }
        let sender = v
            .get("from")
            .or_else(|| v.get("sender"))
            .and_then(|x| x.as_str());
        let title = if text.len() > 60 { &text[..60] } else { text };
        let mut body = text.to_string();
        if let Some(s) = sender {
            body.insert_str(0, &format!("{s}: "));
        }
        docs.push(VaultDoc {
            source: format!("message {title}"),
            title: title.to_string(),
            body,
            headings: vec![],
        });
    }
    docs
}

/// Read a named attribute out of a tag string (`<outline text="X" …/>`).
fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    let val = &rest[..end];
    (!val.is_empty()).then(|| val.to_string())
}

/// Shell history (zsh extended `: <ts>:<dur>;cmd`, plain bash lines, and
/// backslash continuations) reduced to one doc per unique command. Commands
/// shorter than 3 chars and blank lines are dropped; repetition counts are
/// kept in the body ("ran 14×") so habitual workflows stay visible without
/// one node per keystroke. Caps at 3000 docs; order is first-seen.
pub fn parse_shell_history(input: &str) -> Vec<VaultDoc> {
    const CAP: usize = 3000;
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut current = String::new();
    let flush = |cmd: &mut String,
                     order: &mut Vec<String>,
                     counts: &mut std::collections::HashMap<String, usize>| {
        let c = cmd.trim().to_string();
        cmd.clear();
        if c.len() < 3 {
            return;
        }
        if !counts.contains_key(&c) {
            order.push(c.clone());
        }
        *counts.entry(c).or_insert(0) += 1;
    };
    for raw in input.lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            continue;
        }
        // Backslash continuation: append to the open command, else every
        // physical line starts a new entry (bash is one-command-per-line;
        // zsh stores multiline commands with a trailing backslash).
        let continued = current.ends_with('\\');
        if continued {
            current.pop();
            current.push(' ');
            current.push_str(line.trim());
            continue;
        }
        if let Some(cmd) = strip_zsh_prefix(line) {
            flush(&mut current, &mut order, &mut counts);
            current = cmd.to_string();
        } else if line.starts_with(':') {
            // Malformed timestamp line — treat the tail as a command.
            flush(&mut current, &mut order, &mut counts);
            current = line.to_string();
        } else {
            flush(&mut current, &mut order, &mut counts);
            current = line.trim().to_string();
        }
    }
    flush(&mut current, &mut order, &mut counts);
    order
        .into_iter()
        .take(CAP)
        .map(|cmd| {
            let n = counts.get(&cmd).copied().unwrap_or(1);
            let title = if cmd.len() > 80 { cmd[..80].to_string() } else { cmd.clone() };
            let body = if n > 1 {
                format!("shell: {cmd} (ran {n}×)")
            } else {
                format!("shell: {cmd}")
            };
            VaultDoc {
                source: format!("shell {title}"),
                title,
                body,
                headings: vec![],
            }
        })
        .collect()
}

/// Strip the zsh extended-history prefix (`: <epoch>:<duration>;`). Returns
/// `None` for plain bash lines.
fn strip_zsh_prefix(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(':')?.trim_start();
    let semi = rest.find(';')?;
    let (head, cmd) = rest.split_at(semi);
    // Head must look like `<digits>:<digits>` — otherwise the `;` belongs to
    // the command itself (e.g. `a; b`) and this is a plain bash line.
    let mut parts = head.split(':');
    let ts_ok = parts.next().is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let dur_ok = parts.next().is_some_and(|p| p.bytes().all(|b| b.is_ascii_digit()));
    if ts_ok && dur_ok && parts.next().is_none() {
        Some(cmd[1..].trim())
    } else if ts_ok && head.contains(':') {
        // `: ts;cmd` without duration — accept, command follows the `;`.
        Some(cmd[1..].trim())
    } else {
        None
    }
}

/// One Claude Code session export (`~/.claude/projects/*/UUID.jsonl`):
/// user prompts become `prompt` docs, assistant text becomes `answer` docs
/// (tool calls and thinking blocks skipped), `summary` records become their
/// own doc. Queue/attachment/system records are skipped. Caps at 500 docs.
pub fn parse_claude_session(input: &str) -> Vec<VaultDoc> {
    const CAP: usize = 500;
    const BODY_CAP: usize = 4000;
    let mut docs = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        if docs.len() >= CAP {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let rtype = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let (kind, text) = match rtype {
            "user" => ("prompt", message_text(&v)),
            "assistant" => ("answer", message_text(&v)),
            "summary" | "file-history-summary" => ("summary", message_text(&v)),
            _ => continue,
        };
        let text = text.trim();
        if text.len() < 20 {
            continue;
        }
        let body = if text.len() > BODY_CAP {
            format!("{}…", &text[..BODY_CAP])
        } else {
            text.to_string()
        };
        let title = if text.len() > 80 {
            text[..80].to_string()
        } else {
            text.to_string()
        };
        docs.push(VaultDoc {
            source: format!("session msg {idx}"),
            title: format!("[{kind}] {title}"),
            body,
            headings: vec![],
        });
    }
    docs
}

/// Pull readable text out of a Claude record's `message.content`, which is
/// either a plain string (user) or an array of `{type, text}` blocks
/// (assistant: text/tool_use/thinking — only `text` is kept).
fn message_text(v: &serde_json::Value) -> String {
    let content = match v.get("message").and_then(|m| m.get("content")) {
        Some(c) => c,
        None => return v
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string(),
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    content
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Read a file and reduce it through the importer matching its name/extension.
/// Returns `(docs, name)` or an error naming the problem.
pub fn import_file(path: &Path) -> anyhow::Result<(Vec<VaultDoc>, String)> {
    let importer = importer_for(path)
        .ok_or_else(|| anyhow::anyhow!("no personal-history importer for {}", path.display()))?;
    let text = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok((importer(&text), name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bookmarks_html_parses_links_and_folders() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
  <DT><H3 ADD_DATE="1">Work</H3>
  <DL><p>
    <DT><A HREF="https://shopfloor.example/" ADD_DATE="2">Shop floor</A>
  </DL><p>
  <DT><A HREF="https://q1pro.local/" ADD_DATE="3">Printer</A>
</DL><p>"#;
        let docs = parse_bookmarks_html(html);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].title, "Shop floor");
        assert!(docs[0].body.contains("shopfloor.example"), "url in body");
        assert!(docs[0].body.contains("Work"), "folder context");
        assert_eq!(docs[1].title, "Printer");
    }

    #[test]
    fn test_history_json_array_and_object() {
        let arr = r#"[{"title":"Autoclave Manual","url":"https://d.example/autoclave","lastVisitTime":1}]"#;
        let docs = parse_history_json(arr);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "Autoclave Manual");
        assert!(docs[0].body.contains("https://d.example/autoclave"));

        let obj = r#"{"items":[{"title":"","url":"https://e.example/x"}]}"#;
        let docs = parse_history_json(obj);
        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].title, "https://e.example/x",
            "empty title falls back to url"
        );

        assert!(parse_history_json("not json").is_empty());
    }

    #[test]
    fn test_opml_parses_feeds_and_folders() {
        let opml = r#"<?xml version="1.0"?>
<opml version="2.0">
  <body>
    <outline text="Manufacturing">
      <outline type="rss" text="IoT Blog" xmlUrl="https://iot.example/rss" htmlUrl="https://iot.example"/>
      <outline type="rss" text="OEE Weekly" xmlUrl="https://oee.example/feed"/>
    </outline>
  </body>
</opml>"#;
        let docs = parse_opml(opml);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].title, "IoT Blog");
        assert!(docs[0].body.contains("iot.example/rss"));
        assert!(docs[0].body.contains("Manufacturing"), "folder context");
        assert_eq!(docs[1].title, "OEE Weekly");
    }

    #[test]
    fn test_messages_jsonl_parses_senders() {
        let jsonl = "{\"from\":\"gio\",\"text\":\"check the spindle bearing\"}\n{\"from\":\"line2\",\"message\":\"ok\"}\nnot json\n";
        let docs = parse_messages_jsonl(jsonl);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].title, "check the spindle bearing");
        assert!(docs[0].body.starts_with("gio: "), "sender prefixed");
        assert_eq!(docs[1].body, "line2: ok");
    }

    #[test]
    fn test_importer_for_extensions() {
        assert!(importer_for(Path::new("b.html")).is_some());
        assert!(importer_for(Path::new("b.htm")).is_some());
        assert!(importer_for(Path::new("h.json")).is_some());
        assert!(importer_for(Path::new("feeds.opml")).is_some());
        assert!(importer_for(Path::new("chat.jsonl")).is_some());
        assert!(importer_for(Path::new("notes.txt")).is_some());
        assert!(importer_for(Path::new("data.csv")).is_none());
    }

    #[test]
    fn test_shell_history_zsh_bash_dedupe() {
        let input = ": 1786010752:0;sudo apt install -y zsh\nls\n: 1786010769:0;sudo apt install -y zsh\n  \ncd\ncargo test --lib pack\n";
        let docs = parse_shell_history(input);
        // apt line deduped with count, `ls`/`cd` dropped (<3 chars), blank dropped.
        assert_eq!(docs.len(), 2);
        assert!(docs[0].body.contains("ran 2×"), "repetition kept: {}", docs[0].body);
        assert!(docs[0].body.starts_with("shell: "), "shell prefix");
        assert_eq!(docs[1].title, "cargo test --lib pack");
        assert!(docs[1].body.starts_with("shell: cargo test"));
    }

    #[test]
    fn test_shell_history_semicolons_are_not_prefixes() {
        // A bash line containing `;` must not be mistaken for a zsh prefix.
        let docs = parse_shell_history("make foo; make bar\n");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "make foo; make bar");
    }

    #[test]
    fn test_claude_session_prompts_answers_skip_blobs() {
        let jsonl = concat!(
            "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Review this change for security vulnerabilities in the classifier module\"}}\n",
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"The classifier looks solid overall, here are three findings to consider carefully\"},{\"type\":\"tool_use\",\"name\":\"Read\"}]}}\n",
            "{\"type\":\"queue-operation\",\"operation\":\"enqueue\",\"content\":\"huge diff blob\"}\n",
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"name\":\"Bash\"}]}}\n",
            "not json\n",
        );
        let docs = parse_claude_session(jsonl);
        assert_eq!(docs.len(), 2);
        assert!(docs[0].title.starts_with("[prompt]"), "kind tag: {}", docs[0].title);
        assert!(docs[0].body.contains("security vulnerabilities"));
        assert!(docs[1].title.starts_with("[answer]"), "{}", docs[1].title);
        assert!(!docs[1].body.contains("tool_use"), "no tool blobs");
    }

    #[test]
    fn test_importer_for_dotfiles_and_sessions() {
        assert!(importer_for(Path::new(".zsh_history")).is_some());
        assert!(importer_for(Path::new(".bash_history")).is_some());
        assert!(importer_for(Path::new("03273da0-92f8-4a92-ad22-47c70d2c9243.jsonl")).is_some());
        // Generic jsonl still goes to the message parser.
        assert!(importer_for(Path::new("chat.jsonl")).is_some());
    }
}
