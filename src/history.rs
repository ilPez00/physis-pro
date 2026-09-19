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
        && stem.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
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
        let title = truncate_bytes(text, 60);
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
            let title = truncate_bytes(&cmd, 80).to_string();
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
    let ts_ok = parts
        .next()
        .is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let dur_ok = parts
        .next()
        .is_some_and(|p| p.bytes().all(|b| b.is_ascii_digit()));
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
            format!("{}…", truncate_bytes(text, BODY_CAP))
        } else {
            text.to_string()
        };
        let title = truncate_bytes(text, 80).to_string();
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
        None => {
            return v
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or_default()
                .to_string()
        }
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

/// Truncate to `max` bytes without splitting a UTF-8 char.
fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Shared sink for pre-extracted agent turns (opencode/codex rows): one doc
/// per turn with a `[agent/role]` title tag. Short stubs (<20 chars) and
/// empties are dropped; caps at 500 docs per session.
pub fn agent_turn_docs(agent: &str, session: &str, rows: &[(String, String)]) -> Vec<VaultDoc> {
    const CAP: usize = 500;
    const BODY_CAP: usize = 4000;
    let mut docs = Vec::new();
    for (idx, (role, text)) in rows.iter().enumerate() {
        if docs.len() >= CAP {
            break;
        }
        let text = text.trim();
        if text.len() < 20 {
            continue;
        }
        let body = if text.len() > BODY_CAP {
            format!("{}…", truncate_bytes(text, BODY_CAP))
        } else {
            text.to_string()
        };
        let short = truncate_bytes(text, 80).to_string();
        docs.push(VaultDoc {
            source: format!("{agent} {session} turn {idx}"),
            title: format!("[{agent}/{role}] {short}"),
            body,
            headings: vec![],
        });
    }
    docs
}

/// Read opencode turns from its SQLite store: newest `max_sessions` sessions
/// by update time, each as `(session_id, [(role, text)])` in time order.
/// Only `text` parts are kept (step markers, tool calls, reasoning parts
/// skipped); texts under 20 chars are dropped by [`agent_turn_docs`].
/// Read-only open; returns empty when the store is missing/unreadable.
#[cfg(feature = "sql")]
pub fn read_opencode_turns(db: &Path, max_sessions: usize) -> Vec<(String, Vec<(String, String)>)> {
    let conn =
        match rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM session ORDER BY time_updated DESC LIMIT ?1")
        .and_then(|mut q| {
            q.query_map([max_sessions as i64], |r| r.get(0))
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<String>>>())
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in ids {
        let rows: Vec<(String, String)> = conn
            .prepare(
                "SELECT m.data, p.data FROM part p \
                 JOIN message m ON m.id = p.message_id \
                 WHERE p.session_id = ?1 ORDER BY p.time_created, p.id",
            )
            .and_then(|mut q| {
                q.query_map([id.as_str()], |r| {
                    let msg: String = r.get(0)?;
                    let part: String = r.get(1)?;
                    Ok((msg, part))
                })
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<(String, String)>>>())
            })
            .unwrap_or_default();
        let mut turns = Vec::new();
        for (msg, part) in &rows {
            let role = serde_json::from_str::<serde_json::Value>(msg)
                .ok()
                .and_then(|v| v.get("role").and_then(|r| r.as_str()).map(str::to_string))
                .unwrap_or_else(|| "unknown".to_string());
            let text = serde_json::from_str::<serde_json::Value>(part)
                .ok()
                .filter(|v| v.get("type").and_then(|t| t.as_str()) == Some("text"))
                .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string));
            if let Some(t) = text {
                turns.push((role, t));
            }
        }
        if !turns.is_empty() {
            out.push((id, turns));
        }
    }
    out
}

/// Read codex threads from its SQLite store: newest `max_threads` threads by
/// last item time, each as `(thread_id, [(role, text)])` in rollout order.
/// `userMessage` text and `agentMessage` text kept; command executions
/// skipped (shell history already covers commands).
/// Read-only open; returns empty when the store is missing/unreadable.
#[cfg(feature = "sql")]
pub fn read_codex_turns(db: &Path, max_threads: usize) -> Vec<(String, Vec<(String, String)>)> {
    let conn =
        match rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
    let ids: Vec<String> = conn
        .prepare(
            "SELECT thread_id FROM thread_items \
             GROUP BY thread_id ORDER BY MAX(created_at_ms) DESC LIMIT ?1",
        )
        .and_then(|mut q| {
            q.query_map([max_threads as i64], |r| r.get(0))
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<String>>>())
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in ids {
        let rows: Vec<(String, String)> = conn
            .prepare(
                "SELECT item_type, item_json FROM thread_items \
                 WHERE thread_id = ?1 ORDER BY rollout_ordinal",
            )
            .and_then(|mut q| {
                q.query_map([id.as_str()], |r| {
                    let t: String = r.get(0)?;
                    let j: String = r.get(1)?;
                    Ok((t, j))
                })
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<(String, String)>>>())
            })
            .unwrap_or_default();
        let mut turns = Vec::new();
        for (item_type, item_json) in &rows {
            let v: serde_json::Value = match serde_json::from_str(item_json) {
                Ok(v) => v,
                Err(_) => continue,
            };
            match item_type.as_str() {
                "userMessage" => {
                    let text = v
                        .get("content")
                        .and_then(|c| c.as_array())
                        .map(|blocks| {
                            blocks
                                .iter()
                                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    if !text.is_empty() {
                        turns.push(("user".to_string(), text));
                    }
                }
                "agentMessage" => {
                    if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                        turns.push(("assistant".to_string(), t.to_string()));
                    }
                }
                _ => continue,
            }
        }
        if !turns.is_empty() {
            out.push((id, turns));
        }
    }
    out
}

/// Read the mesh memory store (memd sqlite, gossiped across the fleet) as
/// docs: every live row, kind + host folded into the title tag
/// (`[mem/trap@omo]`), body carries the date + text. This is the fleet-view
/// pull — traps, decisions, runbooks and facts other machines recorded land
/// in the local store. Skips deleted rows; caps at 2000. Read-only open;
/// returns empty when the store is missing/unreadable.
#[cfg(feature = "sql")]
pub fn read_memories(db: &Path) -> Vec<VaultDoc> {
    const CAP: usize = 2000;
    let conn =
        match rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
    let rows: Vec<(String, String, String, String, f64)> = conn
        .prepare("SELECT kind, host, title, body, ts FROM mem WHERE deleted = 0 ORDER BY ts DESC LIMIT ?1")
        .and_then(|mut q| {
            q.query_map([CAP as i64], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<(String, String, String, String, f64)>>>())
        })
        .unwrap_or_default();
    rows.into_iter()
        .map(|(kind, host, title, body, ts)| {
            let date = chrono_date(ts);
            VaultDoc {
                source: format!("mesh {kind}@{host}"),
                title: format!("[mem/{kind}@{host}] {title}"),
                body: format!("{date} {host}: {body}"),
                headings: vec![],
            }
        })
        .collect()
}

/// Unix seconds → `YYYY-MM-DD`, best-effort without a date dependency.
#[cfg(feature = "sql")]
fn chrono_date(ts: f64) -> String {
    const DAYS: &[u32] = &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut days = (ts / 86400.0) as i64;
    let mut year = 1970i64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if days < yd {
            break;
        }
        days -= yd;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mut month = 1u32;
    for (i, d) in DAYS.iter().enumerate() {
        let mut dim = *d as i64;
        if i == 1 && leap {
            dim = 29;
        }
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }
    format!("{year:04}-{:02}-{:02}", month, days + 1)
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
        assert!(
            docs[0].body.contains("ran 2×"),
            "repetition kept: {}",
            docs[0].body
        );
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
        assert!(
            docs[0].title.starts_with("[prompt]"),
            "kind tag: {}",
            docs[0].title
        );
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

    #[test]
    fn test_agent_turn_docs_tags_and_skips_stubs() {
        let rows = vec![
            (
                "user".to_string(),
                "how do I tune the spindle PID loop".to_string(),
            ),
            ("assistant".to_string(), "ok".to_string()),
            (
                "assistant".to_string(),
                "Set P first, then I, then D with small steps".to_string(),
            ),
        ];
        let docs = agent_turn_docs("opencode", "ses1", &rows);
        assert_eq!(docs.len(), 2);
        assert!(
            docs[0].title.starts_with("[opencode/user]"),
            "{}",
            docs[0].title
        );
        assert!(
            docs[1].title.starts_with("[opencode/assistant]"),
            "{}",
            docs[1].title
        );
        assert!(docs[0].source.contains("ses1 turn 0"));
    }

    #[test]
    fn test_truncate_never_splits_utf8() {
        // 70 ASCII + em-dashes straddling byte 80: must not panic.
        let text = format!("{}—pad—{}", "x".repeat(70), "y".repeat(40));
        let rows = vec![("user".to_string(), text)];
        let docs = agent_turn_docs("t", "s", &rows);
        assert_eq!(docs.len(), 1);
        assert!(docs[0].title.is_char_boundary(docs[0].title.len()));
        let shell = format!("{}—{}", "z".repeat(100), "w".repeat(10));
        let sdocs = parse_shell_history(&shell);
        assert_eq!(sdocs.len(), 1);
    }

    #[test]
    fn test_chrono_date_known_days() {
        assert_eq!(chrono_date(0.0), "1970-01-01");
        assert_eq!(chrono_date(1787616000.0), "2026-08-25");
        assert_eq!(chrono_date(1767225600.0), "2026-01-01");
    }

    #[cfg(feature = "sql")]
    fn fixture_db(dir: &std::path::Path, name: &str, schema: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        let conn = rusqlite::Connection::open(&p).unwrap();
        conn.execute_batch(schema).unwrap();
        p
    }

    #[test]
    #[cfg(feature = "sql")]
    fn test_read_opencode_turns_newest_first_text_only() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(
            dir.path(),
            "opencode.db",
            "CREATE TABLE session (id TEXT PRIMARY KEY, time_updated INTEGER NOT NULL);
             CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);
             CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);
             INSERT INTO session VALUES ('old', 1), ('new', 2);
             INSERT INTO message VALUES ('m1', 'new', 1, 1, '{\"role\":\"user\"}');
             INSERT INTO part VALUES ('p1', 'm1', 'new', 1, 1, '{\"type\":\"text\",\"text\":\"tune the spindle PID loop carefully\"}');
             INSERT INTO part VALUES ('p2', 'm1', 'new', 2, 2, '{\"type\":\"step-start\"}');",
        );
        let out = read_opencode_turns(&db, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "new");
        assert_eq!(out[0].1.len(), 1, "step-start skipped");
        assert_eq!(out[0].1[0].0, "user");
        // Missing db → empty, not an error.
        assert!(read_opencode_turns(&dir.path().join("nope.db"), 10).is_empty());
    }

    #[test]
    #[cfg(feature = "sql")]
    fn test_read_memories_skips_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(
            dir.path(),
            "mem.db",
            "CREATE TABLE mem (id TEXT PRIMARY KEY, seq INTEGER, ts REAL NOT NULL, host TEXT NOT NULL, agent TEXT NOT NULL DEFAULT 'unknown', kind TEXT NOT NULL, scope TEXT NOT NULL DEFAULT 'global', title TEXT NOT NULL, body TEXT NOT NULL, tags TEXT NOT NULL DEFAULT '', source TEXT NOT NULL DEFAULT '', deleted INTEGER NOT NULL DEFAULT 0);
             INSERT INTO mem VALUES ('a', 1, 1787616000.0, 'omo', 'x', 'trap', 'global', 'never push --force', 'it rewrites shared history', '', '', 0);
             INSERT INTO mem VALUES ('b', 2, 1787616000.0, 'omo', 'x', 'trap', 'global', 'gone', 'deleted row', '', '', 1);",
        );
        let docs = read_memories(&db);
        assert_eq!(docs.len(), 1);
        assert!(
            docs[0].title.starts_with("[mem/trap@omo]"),
            "{}",
            docs[0].title
        );
        assert!(
            docs[0].body.contains("2026-08-25"),
            "date folded in: {}",
            docs[0].body
        );
    }
}
