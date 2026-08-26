//! Mailbox import (PLAN Phase 10 D7) — the space·time·cause treatment for
//! email. Real mail lives in exports, not as loose `.eml` files, so this
//! module walks an export dir (`.eml` files + RFC-4155 `.mbox` bundles) and
//! yields one doc per message with its native headers: From/Subject become
//! the recallable label, Date becomes the message time.
//!
//! Feature-gated on `email` (enabled by default since D7): without the
//! parser there is nothing honest to import — labels would be empty shells.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One imported message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MailDoc {
    /// Origin within the export (`<file>.eml` or `<file>.mbox#3`).
    pub source: String,
    pub from: String,
    pub subject: String,
    /// Message `Date` header when parseable, else file mtime.
    pub date: Option<chrono::DateTime<chrono::Utc>>,
    /// Plain-text body (first text/plain part, multipart concatenated).
    pub body: String,
    /// Recallable label: `mail:<subject> — <from>`.
    pub label: String,
}

/// Walk an export directory for mail: every `.eml` is one message, every
/// `.mbox` is split into its messages. Non-mail files are ignored so a mixed
/// archive can be pointed at wholesale.
pub fn scan_mail_dir(dir: &Path) -> anyhow::Result<Vec<MailDoc>> {
    let mut docs = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if !path.is_file() {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("eml") | Some("msg") => {
                if let Ok(Some(d)) = import_eml(&path) {
                    docs.push(d);
                }
            }
            Some("mbox") => docs.extend(import_mbox(&path)),
            _ => {}
        }
    }
    Ok(docs)
}

fn import_eml(path: &Path) -> anyhow::Result<Option<MailDoc>> {
    let raw = std::fs::read(path)?;
    Ok(parse_message(&raw).map(|d| MailDoc {
        source: path.display().to_string(),
        ..d
    }))
}

/// Split an mbox bundle on sender lines (`^From `, RFC-4155) and parse each
/// chunk. Unparseable chunks are skipped — a 10k-message bundle must not die
/// on one malformed message.
fn import_mbox(path: &Path) -> Vec<MailDoc> {
    let Ok(raw) = std::fs::read(path) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&raw);
    let mut docs = Vec::new();
    let mut chunk = String::new();
    let mut index = 0usize;
    let name = path.display().to_string();
    let flush = |chunk: &mut String, index: usize, docs: &mut Vec<MailDoc>, name: &str| {
        if chunk.trim().is_empty() {
            return;
        }
        // The leading "From " separator line is not part of the message.
        let body_text = chunk
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or(chunk);
        if let Some(mut d) = parse_message(body_text.as_bytes()) {
            d.source = format!("{name}#{index}");
            docs.push(d);
        }
        chunk.clear();
    };
    for line in text.lines() {
        if line.starts_with("From ") && !chunk.is_empty() {
            flush(&mut chunk, index, &mut docs, &name);
            index += 1;
        }
        chunk.push_str(line);
        chunk.push('\n');
    }
    flush(&mut chunk, index, &mut docs, &name);
    docs
}

#[cfg(feature = "email")]
fn parse_message(raw: &[u8]) -> Option<MailDoc> {
    use mailparse::{dateparse, parse_mail, MailHeaderMap};

    let mail = parse_mail(raw).ok()?;
    let mut body = String::new();
    collect_text_plain(&mail, &mut body);
    let headers = mail.get_headers();
    let from = headers.get_first_value("From").unwrap_or_default();
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    if from.is_empty() && subject.is_empty() && body.trim().is_empty() {
        return None;
    }
    let date = headers
        .get_first_value("Date")
        .and_then(|d| dateparse(&d).ok())
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0));
    let subject = subject.trim().to_string();
    let from_display = from.trim().to_string();
    let label = format!(
        "mail:{} — {}",
        if subject.is_empty() {
            "(no subject)"
        } else {
            &subject
        },
        from_display
    );
    Some(MailDoc {
        source: String::new(),
        from: from_display,
        subject,
        date,
        body,
        label,
    })
}

#[cfg(not(feature = "email"))]
fn parse_message(_raw: &[u8]) -> Option<MailDoc> {
    None
}

#[cfg(feature = "email")]
fn collect_text_plain(mail: &mailparse::ParsedMail, out: &mut String) {
    if mail.subparts.is_empty() {
        if mail.ctype.mimetype.starts_with("text/plain") {
            if let Ok(s) = mail.get_body() {
                out.push_str(&s);
                out.push('\n');
            }
        }
    } else {
        for p in &mail.subparts {
            collect_text_plain(p, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EML: &[u8] = b"From: Ada Lovelace <ada@example.org>\n\
To: gio@example.dev\n\
Subject: press tonnage report\n\
Date: Mon, 24 Aug 2026 08:15:00 +0000\n\
Message-ID: <abc@example.org>\n\
Content-Type: text/plain; charset=utf-8\n\
\n\
Hydraulic press drifted 2 tons this week.\n";

    #[test]
    fn eml_yields_label_from_subject_and_sender_with_native_date() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("report.eml");
        std::fs::write(&f, EML).unwrap();
        let docs = scan_mail_dir(tmp.path()).unwrap();
        assert_eq!(docs.len(), 1);
        let d = &docs[0];
        assert!(d.source.ends_with("report.eml"));
        assert_eq!(d.subject, "press tonnage report");
        assert!(d.from.contains("ada@example.org"));
        assert_eq!(
            d.date.map(|t| t.to_rfc3339()),
            Some("2026-08-24T08:15:00+00:00".to_string())
        );
        assert!(d.label.contains("press tonnage report"));
        assert!(d.label.contains("ada@example.org"));
        assert!(d.body.contains("drifted 2 tons"));
    }

    #[test]
    fn mbox_splits_into_indexed_messages_and_skips_garbage() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("inbox.mbox");
        std::fs::write(
            &f,
            "From MAILER Sat Aug 22 10:00:00 2026\n\
From: one@example.org\nSubject: first\n\nbody one\n\
From MAILER Sun Aug 23 11:00:00 2026\n\
From: two@example.org\nSubject: second\n\nbody two\n\
this chunk is not a message at all\n",
        )
        .unwrap();
        let docs = scan_mail_dir(tmp.path()).unwrap();
        assert_eq!(docs.len(), 2, "{docs:?}");
        assert!(docs[0].source.ends_with("inbox.mbox#0"));
        assert!(docs[1].source.ends_with("inbox.mbox#1"));
        assert_eq!(docs[0].subject, "first");
        assert_eq!(docs[1].subject, "second");
    }

    #[test]
    fn non_mail_files_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("note.md"), "# not mail").unwrap();
        std::fs::write(tmp.path().join("data.json"), "{}").unwrap();
        assert!(scan_mail_dir(tmp.path()).unwrap().is_empty());
    }
}
