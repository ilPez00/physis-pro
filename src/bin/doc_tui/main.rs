//! `physis-pro-doc` — the adaptive document, live, on real files.
//!
//! Open a Markdown or text file. Its blocks are parsed with exact byte spans.
//! Declare which blocks depend on which (copy, calculate, review). Edit a
//! source block and the confirmed dependents change on screen as you type —
//! the exact bytes the proposal would write, before you decide to write them.
//! Commit the edit, accept or reject the proposal, undo the repair without
//! erasing your own edit, then write the result back to the file.
//!
//! Every file keeps its own durable history: `document.json` (raw bytes,
//! blocks, links), `events.jsonl` (append-only, replay order) and
//! `patches.json` (proposals, applied, rejected, reverted), under
//! `$PHYSIS_DOC_HOME` (default `~/.physis/adaptive-doc`), one directory per
//! file. Reopening a file replays that history. If the file changed on disk
//! since the last visit, the external edit is reported and the document is
//! re-parsed at a new revision. Links are kept; the ones whose block was
//! replaced stop resolving and are marked dangling in the links view, never
//! silently re-pointed at whatever now sits in that position.
//!
//! Keys (browse):
//!   j/k ↑/↓ select · g/G top/bottom · e/Enter edit the selected block
//!   Space mark a source · c copy · x calculate (asks the factor) · v review
//!   a accept · r reject · u undo · w write the file
//!   o open a file · L links · h history · ? help · q quit
//!
//! Keys (editing): ←/→ Home/End · ⌫ Del · Enter commit · Esc cancel
//!
//! The document shown while editing is the *candidate*: your typed block plus
//! every replacement the proposal would make. Nothing is persisted until you
//! commit, and the proposal is still yours to reject.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use physis_pro::adaptive_document::identity::{reidentify, Candidate, Transition};
use physis_pro::adaptive_document::corpus::{hyperlinks, Corpus, LinkTarget};
use physis_pro::adaptive_document::observation::Observation;
use physis_pro::address::Mechanism;
use physis_pro::ledger::CausalLedger;
use physis_pro::adaptive_document::proposal::*;
use physis_pro::adaptive_document::store::{Patches, Store};
use physis_pro::adaptive_document::*;

mod annotate;
mod terminal;

use std::collections::BTreeSet;
use std::io::Stdout;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Extensions the picker will open. Anything else is not a text document.
fn is_doc_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "md" | "markdown" | "txt" | "text"
        ),
        None => false,
    }
}

/// A readable, collision-free store directory name for an absolute path.
///
/// The stem keeps it legible when a human lists the store; the hash is what
/// makes it unique, so two files with the same basename cannot share a history.
fn doc_key(abs: &Path) -> String {
    let stem: String = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(40)
        .collect();
    format!("{stem}-{:016x}", fnv1a(abs.to_string_lossy().as_bytes()))
}

fn store_root() -> PathBuf {
    if let Ok(dir) = std::env::var("PHYSIS_DOC_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".physis").join("adaptive-doc")
}

fn actor() -> String {
    std::env::var("PHYSIS_ACTOR")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "you".into())
}

/// Next free `lN` link id. Restarting at 1 would mint an id that already
/// exists, and patch reasons reference link ids by name.
fn next_link_seq(doc: &Document) -> u64 {
    doc.links
        .iter()
        .filter_map(|l| l.id.strip_prefix('l')?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

/// What the proposal would do to the document if `typed` replaced the block.
/// Pure: no store, no sequence, no clock. `None` when the edit is a no-op or
/// the block is gone.
fn preview_edit(doc: &Document, block_id: &str, typed: &str) -> Option<(Document, Patch)> {
    let (next, change) = apply_source_edit(doc, block_id, typed, "you", 0, 0)?;
    let patch = propose_after_source_edit(&next, &change, "preview");
    Some((next, patch))
}

/// The document as it would look with the proposal applied. Used for the live
/// view, so a failed preview degrades to the un-adapted candidate rather than
/// blanking the pane.
fn adapted(candidate: &Document, patch: &Patch) -> Document {
    apply_patch(candidate, patch, "preview", 0, 0)
        .map(|(d, _, _)| d)
        .unwrap_or_else(|_| candidate.clone())
}

/// Does a link still point at blocks that exist in `doc`?
///
/// Block ids are content-derived, so replacing a block's text replaces its id.
/// A link declared against the old text therefore stops resolving — which is a
/// fact to report, not a reason to guess a new target.
fn link_resolves(doc: &Document, link: &Link) -> bool {
    doc.block(&link.source_block_id).is_some() && doc.block(&link.dependent_block_id).is_some()
}

/// Decide what to show for a file given its bytes and whatever the store
/// remembers. Three honest outcomes, never a silent guess:
///
/// - nothing stored: parse at revision 0;
/// - stored and the file is byte-identical: the stored state *is* the state;
/// - stored and the file differs: someone edited it outside this tool, so
///   re-parse at the next revision and carry block identity across the diff
///   ([`reidentify`]). Links are kept, not deleted: a human's declaration
///   should not be destroyed by an edit made elsewhere. Only the links whose
///   block really is gone stop resolving, and that is reported.
fn reconcile(
    stored: Option<Document>,
    abs: &Path,
    raw: &str,
) -> (Document, Option<Transition>, Vec<String>) {
    let id = abs.display().to_string();
    match stored {
        None => (
            parse_blocks(&id, 0, raw),
            None,
            vec![format!("opened {id} at revision 0")],
        ),
        Some(prev) if prev.raw == raw => (
            prev.clone(),
            None,
            vec![format!("reloaded {id} at revision {}", prev.revision)],
        ),
        Some(prev) => {
            let (next, t) = reidentify(&prev, raw);
            let dangling = next.links.iter().filter(|l| !link_resolves(&next, l)).count();
            let mut log = vec![format!("external edit — {}", t.summary())];
            if !t.ambiguous.is_empty() {
                log.push(format!(
                    "{} block(s) could not be identified across the diff and were left new",
                    t.ambiguous.len()
                ));
            }
            if dangling > 0 {
                log.push(format!(
                    "{dangling} link(s) no longer resolve: their block is gone (L marks them dangling)"
                ));
            }
            (next, Some(t), log)
        }
    }
}

/// Modification time and length of a file, the pair the watcher compares.
fn file_stamp(path: &Path) -> Option<(Option<SystemTime>, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok(), m.len()))
}

// ── state ───────────────────────────────────────────────────────────────────

/// One open file: the document, where it lives, and everything the store
/// remembers about it.
struct Session {
    doc: Document,
    path: PathBuf,
    store: Store,
    patches: Patches,
    events: Vec<Change>,
    transitions: Vec<Transition>,
    /// The same successions as `Mechanism::Corrected` events, so a chain of
    /// replacements is walkable by `lineage()`/`descendants()`.
    causes: CausalLedger,
    /// What a model said about blocks here, attributed to the embedder.
    observations: Vec<Observation>,
    /// File bytes as last read or written, so "modified" is a fact, not a flag.
    saved_raw: String,
    /// The mtime/length the watcher last accepted, so our own write does not
    /// come back as an external edit.
    watched: Option<(Option<SystemTime>, u64)>,
    /// The file changed underneath while there were unwritten changes.
    conflict: bool,
    pending: Option<Patch>,
    last_applied: Option<Patch>,
    patch_seq: u64,
    link_seq: u64,
    actor: String,
}

impl Session {
    fn seq(&self) -> u64 {
        self.events.len() as u64
    }

    fn modified(&self) -> bool {
        self.saved_raw != self.doc.raw
    }

    fn save_document(&mut self, status: &mut String) {
        if let Err(e) = self.store.save_document(&self.doc) {
            *status = format!("document store write failed: {e}");
        }
    }

    fn save_patches(&mut self, status: &mut String) {
        if let Err(e) = self.store.save_patches(&self.patches) {
            *status = format!("patch book write failed: {e}");
        }
    }

    fn record(&mut self, change: Change, status: &mut String) {
        if let Err(e) = self.store.append_event(&change) {
            *status = format!("event log write failed: {e}");
        }
        self.events.push(change);
    }

    fn replace_patch(&mut self, updated: Patch, status: &mut String) {
        if let Some(slot) = self.patches.patches.iter_mut().find(|p| p.id == updated.id) {
            *slot = updated;
        } else {
            self.patches.patches.push(updated);
        }
        self.save_patches(status);
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Overlay {
    None,
    Links,
    History,
    Settings,
}

/// The four surfaces. Document is the editor; the rest are the surrounding
/// workspace a document lives in.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Tab {
    Document,
    Graph,
    Files,
    Terminal,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Document, Tab::Graph, Tab::Files, Tab::Terminal];

    fn title(self) -> &'static str {
        match self {
            Tab::Document => " Document ",
            Tab::Graph => " Graph ",
            Tab::Files => " Files ",
            Tab::Terminal => " Terminal ",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// The block editor and the one-line prompts share this buffer.
enum InputKind {
    Block { original: String },
    Factor { source: BlockId, dependent: BlockId },
    /// A shell line for the terminal pane.
    Command,
    /// A filename to create in the files pane.
    NewFile,
    /// The one prompt: `t <command>` is a shell, anything else is physis.
    Physis,
}

struct Input {
    buffer: Vec<char>,
    cursor: usize,
    kind: InputKind,
    label: String,
}

impl Input {
    fn text(&self) -> String {
        self.buffer.iter().collect()
    }

    fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer.remove(self.cursor);
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }
}

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

/// Append to the session log, trimming the oldest lines.
///
/// A free function rather than a method so it can run while a `Session` is
/// mutably borrowed: `self.log` and `self.session` are disjoint fields, but a
/// `&mut self` method would borrow both.
fn push_log(log: &mut Vec<String>, line: String) {
    log.push(line);
    let excess = log.len().saturating_sub(400);
    if excess > 0 {
        log.drain(..excess);
    }
}

struct Picker {
    dir: PathBuf,
    entries: Vec<Entry>,
    sel: usize,
    error: Option<String>,
}

impl Picker {
    fn load(dir: PathBuf) -> Picker {
        let mut entries = Vec::new();
        let mut error = None;
        if dir.parent().is_some() {
            entries.push(Entry {
                name: "../".into(),
                path: dir.parent().map(Path::to_path_buf).unwrap_or_else(|| dir.clone()),
                is_dir: true,
            });
        }
        match std::fs::read_dir(&dir) {
            Ok(read) => {
                let mut found: Vec<Entry> = read
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let path = e.path();
                        let is_dir = path.is_dir();
                        if !is_dir && !is_doc_file(&path) {
                            return None;
                        }
                        Some(Entry {
                            name: e.file_name().to_string_lossy().into_owned(),
                            path,
                            is_dir,
                        })
                    })
                    .collect();
                found.sort_by(|a, b| {
                    b.is_dir
                        .cmp(&a.is_dir)
                        .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
                });
                entries.extend(found);
            }
            Err(e) => error = Some(format!("cannot list {}: {e}", dir.display())),
        }
        Picker {
            dir,
            entries,
            sel: 0,
            error,
        }
    }

    fn selected(&self) -> Option<&Entry> {
        self.entries.get(self.sel)
    }

    fn move_by(&mut self, delta: i64) {
        if self.entries.is_empty() {
            return;
        }
        self.sel = (self.sel as i64 + delta).clamp(0, self.entries.len() as i64 - 1) as usize;
    }
}

struct App {
    /// Where per-file stores live. Injectable so a test can point it at a
    /// tempdir instead of the operator's home.
    store_home: PathBuf,
    session: Option<Session>,
    tab: Tab,
    picker: Picker,
    overlay: Overlay,
    input: Option<Input>,
    sel: usize,
    offset: usize,
    marked: Option<BlockId>,
    link_sel: usize,
    hist_sel: usize,
    graph_sel: usize,
    graph_offset: usize,
    settings_sel: usize,
    /// The files pane arms a delete before performing it.
    delete_armed: Option<PathBuf>,
    term: terminal::Runner,
    term_out: Vec<terminal::Outcome>,
    term_scroll: usize,
    model: annotate::Worker,
    /// Operator decisions: whether a model may be loaded, and which embedder.
    settings: annotate::Settings,
    /// Every store under the root, scanned on demand. Never on a keystroke
    /// path that draws.
    corpus: Corpus,
    /// The last natural-language query and what it ranked.
    search: Option<(String, Vec<(usize, f32)>)>,
    /// Set by the files pane when a file should be handed to $EDITOR. The
    /// terminal lives in `main`, not in `App`, so the swap happens there.
    pending_editor: Option<PathBuf>,
    log: Vec<String>,
    status: String,
    help: bool,
    quit_armed: bool,
    quit: bool,
    last_poll: Instant,
}

/// One node in the document's own graph: a block, or a block that is gone but
/// still named by a link or a transition.
struct GraphNode {
    id: BlockId,
    live: bool,
    /// Position in the document, when it still exists.
    index: Option<usize>,
    text: String,
    kind: &'static str,
}

/// Everything one frame needs, computed once: the document to show, which
/// blocks the proposal touches, and the proposal itself.
struct Live {
    doc: Document,
    changed: BTreeSet<BlockId>,
    preview: Option<(Document, Patch)>,
    error: Option<String>,
}

impl App {
    fn new(picker_dir: PathBuf) -> App {
        let settings = annotate::Settings::load(&store_root().join("settings.json"));
        let mut app = App {
            store_home: store_root(),
            session: None,
            tab: Tab::Files,
            picker: Picker::load(picker_dir),
            overlay: Overlay::None,
            input: None,
            sel: 0,
            offset: 0,
            marked: None,
            link_sel: 0,
            hist_sel: 0,
            graph_sel: 0,
            graph_offset: 0,
            settings_sel: 0,
            delete_armed: None,
            term: terminal::Runner::new(),
            term_out: Vec::new(),
            term_scroll: 0,
            model: annotate::Worker::new(settings.clone()),
            settings,
            corpus: Corpus::default(),
            search: None,
            pending_editor: None,
            log: Vec::new(),
            status: "Files: Enter opens a document, n makes one, d deletes".into(),
            help: false,
            quit_armed: false,
            quit: false,
            last_poll: Instant::now(),
        };
        push_log(&mut app.log, "no file open".into());
        app
    }

    /// Open a file: read it, reconcile with whatever the store remembers, and
    /// make it the session. Any failure leaves the current session untouched.
    fn open_path(&mut self, path: PathBuf) {
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) => {
                self.status = format!("cannot read {}: {e}", path.display());
                return;
            }
        };
        let abs = path.canonicalize().unwrap_or_else(|_| path.clone());
        let store = Store::open(&self.store_home.join(doc_key(&abs)));
        let stored = store.load_document().ok().flatten();
        let (doc, transition, log) = reconcile(stored, &abs, &raw);
        let patches = store.load_patches().unwrap_or_default();
        let events = store.load_events().unwrap_or_default();
        let mut transitions = store.load_transitions().unwrap_or_default();
        let mut causes = store.load_causes().unwrap_or_default();
        let observations = store.load_observations().unwrap_or_default();
        let mut need_annotation = false;
        let mut status = format!("opened {}", abs.display());
        if let Some(t) = transition {
            if let Err(e) = store.append_transition(&t) {
                status = format!("transition log write failed: {e}");
            }
            project_causes(&mut causes, &t, &actor(), &store, &mut status);
            need_annotation = t.warrants_model();
            transitions.push(t);
        }
        let patch_seq = patches
            .patches
            .iter()
            .filter_map(|p| p.id.strip_prefix('p')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let link_seq = next_link_seq(&doc);
        // The patch that produced the current revision is still undoable; an
        // edit since then moved the revision on and it is correctly not offered.
        let last_applied = patches
            .patches
            .iter()
            .rev()
            .find(|p| p.status == PatchStatus::Applied && p.applied_revision == Some(doc.revision))
            .cloned();
        let mut session = Session {
            doc,
            path: abs.clone(),
            store,
            patches,
            events,
            transitions,
            causes,
            observations,
            saved_raw: raw,
            watched: file_stamp(&abs),
            conflict: false,
            pending: None,
            last_applied,
            patch_seq,
            link_seq,
            actor: actor(),
        };
        session.save_document(&mut status);
        for line in log {
            push_log(&mut self.log, line);
        }
        push_log(
            &mut self.log,
            format!(
                "{} event(s), {} patch(es), {} link(s), {} transition(s)",
                session.events.len(),
                session.patches.patches.len(),
                session.doc.links.len(),
                session.transitions.len()
            ),
        );
        self.session = Some(session);
        self.overlay = Overlay::None;
        self.tab = Tab::Document;
        self.sel = 0;
        self.offset = 0;
        self.marked = None;
        self.input = None;
        self.link_sel = 0;
        self.hist_sel = 0;
        self.graph_sel = 0;
        self.graph_offset = 0;
        self.delete_armed = None;
        self.quit_armed = false;
        self.status = format!("{status} — e edits, w writes back to the file");
        match self.settings.model {
            annotate::ModelPolicy::Eager => self.model.warm(),
            annotate::ModelPolicy::Lazy if need_annotation => self.request_annotation(),
            _ => {}
        }
    }

    /// Adopt bytes that came from the file: re-identify block by block, keep
    /// every link whose block survived, and record the transition.
    ///
    /// This is the live path. It runs while the user is not typing, so the
    /// document on screen tracks the document on disk.
    fn adopt_disk(&mut self, raw: String, stamp: (Option<SystemTime>, u64)) {
        let selected = self.selected_id();
        let Some(s) = self.session.as_ref() else { return };
        let (next, t) = reidentify(&s.doc, &raw);
        let summary = t.summary();
        let dangling = next.links.iter().filter(|l| !link_resolves(&next, l)).count();
        let need_annotation = t.warrants_model();
        let mut status = String::new();
        {
            let Some(s) = self.session.as_mut() else { return };
            s.doc = next;
            s.saved_raw = raw;
            s.watched = Some(stamp);
            s.conflict = false;
            s.save_document(&mut status);
            if let Err(e) = s.store.append_transition(&t) {
                status = format!("transition log write failed: {e}");
            }
            let actor = s.actor.clone();
            let mut cstatus = String::new();
            project_causes(&mut s.causes, &t, &actor, &s.store, &mut cstatus);
            if !cstatus.is_empty() {
                status = cstatus;
            }
            s.transitions.push(t);
        }
        if need_annotation {
            self.request_annotation();
        }
        push_log(&mut self.log, format!("live update — {summary}"));
        if dangling > 0 {
            push_log(&mut self.log, format!("{dangling} link(s) dangling"));
        }
        if let (Some(id), Some(s)) = (selected, self.session.as_ref()) {
            if let Some(i) = s.doc.blocks.iter().position(|b| b.id == id) {
                self.sel = i;
            }
        }
        let n = self.session.as_ref().map(|s| s.doc.blocks.len()).unwrap_or(0);
        if self.sel >= n {
            self.sel = 0;
        }
        self.status = if status.is_empty() {
            format!("live update — {summary}")
        } else {
            status
        };
    }

    /// Watch the file. Cheap poll: mtime and length, no watcher dependency.
    ///
    /// Nothing is adopted while a block is being edited — pulling the document
    /// out from under the cursor would be worse than being a moment stale.
    fn poll_file(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(400) {
            return;
        }
        self.last_poll = Instant::now();
        // A block edit holds document bytes the file does not have yet, so
        // adopting would pull the text out from under the cursor. A shell line
        // holds nothing of the document, so it must not stop the watcher.
        if matches!(
            self.input.as_ref().map(|i| &i.kind),
            Some(InputKind::Block { .. }) | Some(InputKind::Factor { .. })
        ) {
            return;
        }
        let Some(s) = self.session.as_ref() else { return };
        let Some(stamp) = file_stamp(&s.path) else { return };
        if Some(stamp) == s.watched {
            return;
        }
        let Ok(raw) = std::fs::read_to_string(&s.path) else { return };
        if raw == s.doc.raw {
            // Our own write, or a touch that changed no bytes.
            if let Some(s) = self.session.as_mut() {
                s.watched = Some(stamp);
                s.conflict = false;
            }
            return;
        }
        if s.modified() {
            if let Some(s) = self.session.as_mut() {
                s.watched = Some(stamp);
                s.conflict = true;
            }
            self.status =
                "the file changed on disk while you have unwritten changes — R reloads it, w overwrites it"
                    .into();
            return;
        }
        self.adopt_disk(raw, stamp);
    }

    /// Discard the in-memory document and take the file as it is on disk.
    fn force_reload(&mut self) {
        let Some(s) = self.session.as_ref() else { return };
        let path = s.path.clone();
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) => {
                self.status = format!("reload failed: {e}");
                return;
            }
        };
        let stamp = file_stamp(&path).unwrap_or((None, 0));
        let same = self.session.as_ref().map(|s| s.doc.raw == raw).unwrap_or(true);
        if same {
            if let Some(s) = self.session.as_mut() {
                s.watched = Some(stamp);
                s.conflict = false;
            }
            self.status = "the file already matches the document".into();
            return;
        }
        self.adopt_disk(raw, stamp);
    }

    /// Ask the model to read the blocks it has not read at their current
    /// bytes. Called only when a transition warranted one, and only for what
    /// is missing, so a second edit does not re-read the whole document.
    fn request_annotation(&mut self) {
        if !self.settings.may_load() {
            self.status = "model off — the diff is recorded, nothing was read (settings: ,)".into();
            return;
        }
        if self.model.busy() {
            return;
        }
        let pending = self.pending_annotations();
        if pending.is_empty() {
            return;
        }
        let label = format!(
            "reading {} block(s) — the diff needed an opinion it could not form",
            pending.len()
        );
        let revision = self.session.as_ref().map(|s| s.doc.revision).unwrap_or(0);
        self.model.request(pending, revision, label);
        push_log(&mut self.log, self.model.label.clone());
    }

    /// The blocks whose current bytes have not been read yet. Split out from
    /// the request so the choice can be tested without building a model.
    fn pending_annotations(&self) -> Vec<annotate::Pending> {
        let Some(s) = self.session.as_ref() else { return Vec::new() };
        s
            .doc
            .blocks
            .iter()
            .filter(|b| {
                !s.observations
                    .iter()
                    .any(|o| o.block_id == b.id && o.content_hash == b.content_hash)
            })
            .take(MAX_ANNOTATE)
            .map(|b| annotate::Pending {
                block_id: b.id.clone(),
                content_hash: b.content_hash,
                text: b.text(&s.doc.raw).to_string(),
            })
            .collect()
    }

    /// Take a finished annotation, if one has arrived. Never blocks.
    fn poll_model(&mut self) {
        let Some(reply) = self.model.poll() else { return };
        match reply {
            annotate::Reply::Ready(observations) => {
                if observations.is_empty() {
                    return;
                }
                let Some(s) = self.session.as_mut() else { return };
                let mut status = String::new();
                for o in observations {
                    match s.observations.iter_mut().find(|x| x.block_id == o.block_id) {
                        Some(slot) => *slot = o,
                        None => s.observations.push(o),
                    }
                }
                if let Err(e) = s.store.save_observations(&s.observations) {
                    status = format!("observations write failed: {e}");
                }
                let fresh = s
                    .observations
                    .iter()
                    .filter(|o| {
                        s.doc
                            .block(&o.block_id)
                            .map(|b| b.content_hash == o.content_hash)
                            .unwrap_or(false)
                    })
                    .count();
                let isolated = s.observations.iter().filter(|o| o.isolated).count();
                let embedder = s
                    .observations
                    .last()
                    .map(|o| o.embedder.clone())
                    .unwrap_or_else(|| "?".into());
                push_log(
                    &mut self.log,
                    format!(
                        "model [{embedder}]: {fresh} block(s) read, {isolated} isolated in this document"
                    ),
                );
                self.status = if status.is_empty() {
                    format!("model [{embedder}] read {fresh} block(s); {isolated} unlike the rest")
                } else {
                    status
                };
            }
            annotate::Reply::Warm(kind) => {
                push_log(&mut self.log, format!("model ready [{kind}]"));
                if self.settings.model == annotate::ModelPolicy::Eager {
                    // Eager means the corpus sees this document even when
                    // nothing in it was ever ambiguous.
                    self.request_annotation();
                }
            }
            annotate::Reply::Ranked(hits) => {
                let query = self
                    .model
                    .query
                    .clone()
                    .unwrap_or_else(|| self.model.label.clone());
                push_log(
                    &mut self.log,
                    format!("physis ranked {} block(s) for {}", hits.len(), truncate(&query, 30)),
                );
                self.status = format!("{} hit(s) — 2 Graph shows them, Enter opens one", hits.len());
                self.search = Some((query, hits));
                self.tab = Tab::Graph;
                self.graph_sel = 0;
            }
            annotate::Reply::Failed(e) => {
                push_log(&mut self.log, format!("model unavailable: {e}"));
                self.status = format!("model unavailable: {e} (the document is unaffected)");
            }
        }
    }

    /// Take a finished shell command, if one has arrived. Never blocks.
    fn poll_terminal(&mut self) {
        let Some(outcome) = self.term.poll() else { return };
        self.status = match outcome.code {
            Some(0) => format!("$ {} → 0", short(&outcome.command)),
            Some(c) => format!("$ {} → exit {c}", short(&outcome.command)),
            None => format!("$ {} → no exit status", short(&outcome.command)),
        };
        self.term_out.push(outcome);
        let excess = self.term_out.len().saturating_sub(50);
        if excess > 0 {
            self.term_out.drain(..excess);
        }
        self.term_scroll = 0;
    }

    /// The observation for a block, if it is still about the current bytes.
    fn observation_for(&self, block_id: &str) -> Option<&Observation> {
        let s = self.session.as_ref()?;
        let block = s.doc.block(block_id)?;
        s.observations
            .iter()
            .find(|o| o.block_id == block_id && o.content_hash == block.content_hash)
    }

    /// Publish the adapted document back to the file it came from.
    fn write_back(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        if !s.modified() {
            self.status = format!("{} already matches the document — nothing to write", s.path.display());
            return;
        }
        match physis_pro::atomic::write_durable(&s.path, s.doc.raw.as_bytes()) {
            Ok(()) => {
                s.saved_raw = s.doc.raw.clone();
                s.watched = file_stamp(&s.path);
                s.conflict = false;
                let msg = format!("wrote {} bytes to {}", s.doc.raw.len(), s.path.display());
                push_log(&mut self.log, msg.clone());
                self.status = msg;
                self.quit_armed = false;
            }
            Err(e) => self.status = format!("write failed: {e} (document unchanged on disk)"),
        }
    }

    fn live(&self) -> Option<Live> {
        let s = self.session.as_ref()?;
        if let Some(input) = &self.input {
            if let InputKind::Block { original } = &input.kind {
                let typed = input.text();
                if &typed != original {
                    if let Some(id) = self.selected_id() {
                        match preview_edit(&s.doc, &id, &typed) {
                            Some((candidate, patch)) => {
                                let changed: BTreeSet<BlockId> =
                                    patch.replacements.iter().map(|r| r.block_id.clone()).collect();
                                let shown = adapted(&candidate, &patch);
                                return Some(Live {
                                    doc: shown,
                                    changed,
                                    preview: Some((candidate, patch)),
                                    error: None,
                                });
                            }
                            None => {
                                return Some(Live {
                                    doc: s.doc.clone(),
                                    changed: BTreeSet::new(),
                                    preview: None,
                                    error: Some("edit produced no observable change".into()),
                                })
                            }
                        }
                    }
                }
            }
        }
        Some(Live {
            doc: s.doc.clone(),
            changed: BTreeSet::new(),
            preview: None,
            error: None,
        })
    }

    fn selected_id(&self) -> Option<BlockId> {
        let s = self.session.as_ref()?;
        s.doc.blocks.get(self.sel).map(|b| b.id.clone())
    }

    fn start_edit(&mut self) {
        let Some(id) = self.selected_id() else { return };
        let Some(s) = self.session.as_ref() else { return };
        let Some(block) = s.doc.block(&id) else { return };
        let text = block.text(&s.doc.raw).to_string();
        self.input = Some(Input {
            buffer: text.chars().collect(),
            cursor: text.chars().count(),
            kind: InputKind::Block { original: text },
            label: "edit source block — dependents adapt live".into(),
        });
        self.status = "Enter commit · Esc cancel · linked blocks adapt as you type".into();
    }

    fn cancel_input(&mut self) {
        self.input = None;
        self.status = "cancelled — document bytes unchanged".into();
    }

    fn commit_edit(&mut self) {
        let Some(input) = self.input.take() else { return };
        let original = match &input.kind {
            InputKind::Block { original } => original.clone(),
            _ => return,
        };
        let typed = input.text();
        let Some(id) = self.selected_id() else { return };
        if typed == original {
            self.status = "no change — nothing recorded".into();
            return;
        }
        let Some(s) = self.session.as_mut() else { return };
        let Some((next, change)) = apply_source_edit(
            &s.doc,
            &id,
            &typed,
            &s.actor,
            s.seq(),
            now(),
        ) else {
            self.status = "edit produced no observable change".into();
            return;
        };
        s.doc = next;
        let mut status = String::new();
        s.save_document(&mut status);
        s.record(change.clone(), &mut status);
        let patch = propose_after_source_edit(&s.doc, &change, &format!("p{}", s.patch_seq));
        s.patch_seq += 1;
        let summary = format!(
            "proposed {} replacement(s), {} review note(s) — a accept · r reject",
            patch.replacements.len(),
            patch.review_only.len()
        );
        push_log(&mut self.log, format!("edit {} rev {}", change.event_id, change.after_revision));
        s.pending = Some(patch.clone());
        s.patches.patches.push(patch);
        s.save_patches(&mut status);
        self.status = if status.is_empty() { summary } else { status };
    }

    fn accept(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        let Some(patch) = s.pending.clone() else {
            self.status = "nothing pending to accept".into();
            return;
        };
        match apply_patch(&s.doc, &patch, &s.actor.clone(), s.seq(), now()) {
            Ok((next, change, applied)) => {
                s.doc = next;
                let mut status = String::new();
                s.save_document(&mut status);
                s.record(change, &mut status);
                push_log(&mut self.log, format!("apply_patch:{} rev {}", applied.id, s.doc.revision));
                s.replace_patch(applied.clone(), &mut status);
                s.last_applied = Some(applied);
                s.pending = None;
                self.status = if status.is_empty() {
                    "applied — u undoes the repair, your source edit stays · w writes the file".into()
                } else {
                    status
                };
            }
            Err(e) => self.status = format!("apply failed: {e}"),
        }
    }

    fn reject(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        let Some(patch) = s.pending.clone() else {
            self.status = "nothing pending to reject".into();
            return;
        };
        match reject_patch(&patch) {
            Ok(rejected) => {
                push_log(&mut self.log, format!("reject_patch:{} (zero bytes changed)", rejected.id));
                let mut status = String::new();
                s.replace_patch(rejected, &mut status);
                s.pending = None;
                self.status = if status.is_empty() {
                    "rejected — document bytes unchanged".into()
                } else {
                    status
                };
            }
            Err(e) => self.status = format!("reject failed: {e}"),
        }
    }

    fn undo(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        let Some(applied) = s.last_applied.clone() else {
            self.status = "nothing applied yet to undo".into();
            return;
        };
        match undo_patch(&s.doc, &applied, &s.actor.clone(), s.seq(), now()) {
            Ok((next, change, reverted)) => {
                s.doc = next;
                let mut status = String::new();
                s.save_document(&mut status);
                s.record(change, &mut status);
                push_log(&mut self.log, format!("undo_patch:{} rev {}", reverted.id, s.doc.revision));
                s.replace_patch(reverted, &mut status);
                s.last_applied = None;
                self.status = if status.is_empty() {
                    "undone — dependents reverted, source edit preserved".into()
                } else {
                    status
                };
            }
            Err(e) => self.status = format!("undo failed: {e} (intervening edits?)"),
        }
    }

    /// Mark the selected block as a link source.
    fn mark(&mut self) {
        self.marked = self.selected_id();
        self.status = match &self.marked {
            Some(id) => format!("marked {id} — select the dependent, then c / x / v"),
            None => "nothing to mark".into(),
        };
    }

    /// Create a confirmed link from the marked block to the selected one.
    /// Confirmed provenance is what makes it auto-propagate; a link that is
    /// merely *proposed* by a parser would be advisory and is not offered here.
    fn link(&mut self, behavior: LinkBehavior, transform: Option<String>) {
        let Some(source) = self.marked.clone() else {
            self.status = "mark a source first: Space on the block to change from".into();
            return;
        };
        let Some(dependent) = self.selected_id() else { return };
        if source == dependent {
            self.status = "source and dependent are the same block".into();
            return;
        }
        let Some(s) = self.session.as_mut() else { return };
        let id = format!("l{}", s.link_seq);
        s.link_seq += 1;
        s.doc.links.push(Link {
            id: id.clone(),
            source_block_id: source.clone(),
            dependent_block_id: dependent.clone(),
            behavior,
            transform_id: transform.clone(),
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: s.doc.revision,
            active: true,
        });
        // The link graph is part of the revision a patch is checked against.
        s.doc.graph_revision += 1;
        let mut status = String::new();
        s.save_document(&mut status);
        push_log(&mut self.log, format!(
            "link {id} {behavior:?}{} {} -> {}",
            transform.map(|t| format!(" [{t}]")).unwrap_or_default(),
            short(&source),
            short(&dependent)
        ));
        self.status = if status.is_empty() {
            format!("{id} {behavior:?} — edit the source and it adapts")
        } else {
            status
        };
        self.marked = None;
    }

    fn toggle_link(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        let Some(link) = s.doc.links.get_mut(self.link_sel) else { return };
        link.active = !link.active;
        let (id, active) = (link.id.clone(), link.active);
        s.doc.graph_revision += 1;
        let mut status = String::new();
        s.save_document(&mut status);
        push_log(&mut self.log, format!("link {id} active={active}"));
        self.status = format!(
            "link {id} {} — graph revision {}",
            if active { "active" } else { "inactive (excluded from traversal)" },
            s.doc.graph_revision
        );
    }

    fn delete_link(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        if self.link_sel >= s.doc.links.len() {
            return;
        }
        let removed = s.doc.links.remove(self.link_sel);
        s.doc.graph_revision += 1;
        if self.link_sel >= s.doc.links.len() && self.link_sel > 0 {
            self.link_sel -= 1;
        }
        let mut status = String::new();
        s.save_document(&mut status);
        push_log(&mut self.log, format!("link {} deleted", removed.id));
        self.status = format!("link {} deleted — graph revision {}", removed.id, s.doc.graph_revision);
    }

    /// Repair a dangling link using the displacement recorded when its block
    /// was replaced.
    ///
    /// Offered, never automatic. Identity was refused here precisely because
    /// the evidence did not support it; a person confirming the succession is
    /// what makes it usable, and the record says how strong the evidence was.
    fn reanchor(&mut self) {
        let Some(s) = self.session.as_ref() else { return };
        let Some(link) = s.doc.links.get(self.link_sel) else { return };
        let (missing, is_source) = if s.doc.block(&link.source_block_id).is_none() {
            (link.source_block_id.clone(), true)
        } else if s.doc.block(&link.dependent_block_id).is_none() {
            (link.dependent_block_id.clone(), false)
        } else {
            self.status = format!("link {} already resolves", link.id);
            return;
        };
        let candidate: Option<Candidate> = s
            .transitions
            .iter()
            .rev()
            .find_map(|t| t.candidates_for(&missing).first().cloned());
        let Some(candidate) = candidate else {
            self.status = format!("no recorded replacement for {missing}");
            return;
        };
        let mut status = String::new();
        let Some(s) = self.session.as_mut() else { return };
        let id = s.doc.links[self.link_sel].id.clone();
        let revision = s.doc.revision;
        {
            let link = &mut s.doc.links[self.link_sel];
            if is_source {
                link.source_block_id = candidate.to.clone();
            } else {
                link.dependent_block_id = candidate.to.clone();
            }
            link.source_revision_at_confirmation = revision;
        }
        s.doc.graph_revision += 1;
        s.save_document(&mut status);
        push_log(
            &mut self.log,
            format!(
                "re-anchor {id}: {missing} -> {} ({} shared word(s){})",
                candidate.to,
                candidate.shared_tokens,
                if candidate.positional { ", positional" } else { "" }
            ),
        );
        self.status = if status.is_empty() {
            format!("{id} re-anchored to {}", candidate.to)
        } else {
            status
        };
    }

    // ── files ───────────────────────────────────────────────────────────────

    /// Arm a delete; a second `d` performs it. Nothing is removed on one press.
    fn arm_delete(&mut self) {
        let Some(entry) = self.picker.selected().cloned() else { return };
        if entry.is_dir {
            self.status = format!("{} is a directory — not removed", entry.name);
            return;
        }
        if self.delete_armed.as_deref() == Some(entry.path.as_path()) {
            match std::fs::remove_file(&entry.path) {
                Ok(()) => {
                    push_log(&mut self.log, format!("deleted {}", entry.path.display()));
                    self.status = format!("deleted {}", entry.path.display());
                    self.delete_armed = None;
                    let dir = self.picker.dir.clone();
                    self.picker = Picker::load(dir);
                }
                Err(e) => self.status = format!("delete failed: {e}"),
            }
        } else {
            self.delete_armed = Some(entry.path.clone());
            self.status = format!("d again to delete {} — this is not undoable", entry.name);
        }
    }

    fn commit_new_file(&mut self) {
        let Some(input) = self.input.take() else { return };
        let name = input.text().trim().to_string();
        if name.is_empty() || name.contains('/') {
            self.status = "a plain file name, no slashes".into();
            return;
        }
        let path = self.picker.dir.join(&name);
        if path.exists() {
            self.status = format!("{name} already exists");
            return;
        }
        match std::fs::write(&path, "# ".to_string() + &name + "\n\n") {
            Ok(()) => {
                push_log(&mut self.log, format!("created {}", path.display()));
                let dir = self.picker.dir.clone();
                self.picker = Picker::load(dir);
                self.open_path(path);
            }
            Err(e) => self.status = format!("cannot create {name}: {e}"),
        }
    }

    // ── graph ───────────────────────────────────────────────────────────────

    /// Nodes are live blocks plus the blocks that are gone but still named by a
    /// link or a transition. A dead node is not clutter: it is the reason a
    /// link dangles, and the thing a re-anchor acts on.
    fn graph_nodes(&self) -> Vec<GraphNode> {
        let Some(s) = self.session.as_ref() else { return Vec::new() };
        let mut nodes: Vec<GraphNode> = s
            .doc
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| GraphNode {
                id: b.id.clone(),
                live: true,
                index: Some(i),
                text: truncate(b.text(&s.doc.raw), 44),
                kind: match b.kind {
                    BlockKind::Heading => "H",
                    BlockKind::Paragraph => "P",
                    BlockKind::Code => "C",
                    BlockKind::Other => "?",
                },
            })
            .collect();
        let live: BTreeSet<BlockId> = nodes.iter().map(|n| n.id.clone()).collect();
        let mut dead: BTreeSet<BlockId> = BTreeSet::new();
        for l in &s.doc.links {
            for id in [&l.source_block_id, &l.dependent_block_id] {
                if !live.contains(id) {
                    dead.insert(id.clone());
                }
            }
        }
        for t in &s.transitions {
            for d in &t.displacements {
                if !live.contains(&d.from) {
                    dead.insert(d.from.clone());
                }
            }
        }
        for id in dead {
            nodes.push(GraphNode {
                id,
                live: false,
                index: None,
                text: "(gone — replaced)".into(),
                kind: "·",
            });
        }
        nodes
    }

    fn on_key_graph(&mut self, code: KeyCode) {
        let n = self.graph_nodes().len();
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit_or_warn(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Down | KeyCode::Char('j') if n > 0 => {
                self.graph_sel = (self.graph_sel + 1) % n;
            }
            KeyCode::Up | KeyCode::Char('k') if n > 0 => {
                self.graph_sel = (self.graph_sel + n - 1) % n;
            }
            KeyCode::Char('g') => self.graph_sel = 0,
            KeyCode::Char('G') => self.graph_sel = n.saturating_sub(1),
            KeyCode::Enter => self.jump_to_selected(),
            KeyCode::Char('r') => self.reanchor(),
            KeyCode::Char('f') => self.follow_hyperlink(),
            KeyCode::Char('x') => self.show_corpus_relations(),
            KeyCode::Char('c') => {
                self.search = None;
                self.rescan_corpus();
            }
            _ => {}
        }
    }

    /// Go to the block the graph cursor is on, in the editor.
    fn jump_to_selected(&mut self) {
        if let Some((_, hits)) = self.search.clone() {
            if let Some((index, _)) = hits.get(self.graph_sel) {
                self.open_corpus_hit(*index);
            }
            return;
        }
        let nodes = self.graph_nodes();
        let Some(node) = nodes.get(self.graph_sel) else { return };
        match node.index {
            Some(i) => {
                self.sel = i;
                self.tab = Tab::Document;
                self.status = format!("jumped to {}", short(&node.id));
            }
            None => {
                self.status = format!(
                    "{} is gone — a link to it can be re-anchored from the Links view",
                    short(&node.id)
                );
            }
        }
    }

    /// Every edge touching a block: links in both directions, and the
    /// successions recorded when a block was replaced.
    fn edges_of(&self, id: &str) -> Vec<String> {
        let Some(s) = self.session.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        for l in &s.doc.links {
            if l.source_block_id == id {
                out.push(format!(
                    "→ {} [{} {}]",
                    short(&l.dependent_block_id),
                    format!("{:?}", l.behavior).to_lowercase(),
                    l.transform_id.clone().unwrap_or_else(|| "—".into())
                ));
            } else if l.dependent_block_id == id {
                out.push(format!("← {} [dependent]", short(&l.source_block_id)));
            }
        }
        for t in &s.transitions {
            for (now_id, was_id) in t.successions() {
                if was_id == id {
                    out.push(format!("⇒ replaced by {} [precedes]", short(&now_id)));
                }
                if now_id == id {
                    out.push(format!("⇐ replaced {}", short(&was_id)));
                }
            }
        }
        out
    }

    // ── terminal ────────────────────────────────────────────────────────────

    fn on_key_terminal(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.quit_or_warn(),
            KeyCode::Esc => self.quit_or_warn(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Enter | KeyCode::Char('i') => {
                self.input = Some(Input {
                    buffer: Vec::new(),
                    cursor: 0,
                    kind: InputKind::Command,
                    label: "shell — runs as you, in the document's directory".into(),
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.term_scroll = self.term_scroll.saturating_sub(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.term_scroll = (self.term_scroll + 1).min(self.term_out.len().saturating_sub(1));
            }
            KeyCode::Char('c') => {
                self.term_out.clear();
                self.term_scroll = 0;
                self.status = "terminal output cleared".into();
            }
            _ => {}
        }
    }

    /// Run the typed line. The pane keeps focus so it behaves like a shell.
    fn run_command(&mut self) {
        let Some(input) = self.input.as_ref() else { return };
        let line = input.text();
        let cwd = self
            .session
            .as_ref()
            .and_then(|s| s.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        if line.trim().is_empty() {
            return;
        }
        self.term.run(&line, &cwd);
        self.status = format!("$ {line}");
        if let Some(input) = self.input.as_mut() {
            input.buffer.clear();
            input.cursor = 0;
        }
    }

    // ── the prompt, the corpus, and hyperlinks ──────────────────────────────

    /// One line, two verbs. `t ` (or `!`) is a shell line; everything else is a
    /// question for physis, which searches the corpus for it.
    fn commit_prompt(&mut self) {
        let Some(input) = self.input.take() else { return };
        let line = input.text();
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let shell = trimmed
            .strip_prefix("t ")
            .or_else(|| trimmed.strip_prefix('!'))
            .map(str::to_string);
        match shell {
            Some(command) => {
                self.tab = Tab::Terminal;
                let cwd = self
                    .session
                    .as_ref()
                    .and_then(|s| s.path.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    });
                self.term.run(&command, &cwd);
                self.status = format!("$ {command}");
            }
            None => self.search_corpus(trimmed),
        }
    }

    /// Rescan the corpus from disk. Cheap enough for a keystroke, bounded so a
    /// large store cannot stall the frame.
    fn rescan_corpus(&mut self) {
        self.corpus = Corpus::scan(&self.store_home, 64, 2000);
        self.status = format!("corpus: {}", self.corpus);
        push_log(&mut self.log, format!("corpus: {}", self.corpus));
    }

    /// Ask physis about a natural-language line: rank the corpus by meaning.
    fn search_corpus(&mut self, query: String) {
        if !self.settings.may_load() {
            self.status = "the model is off, so nothing can be ranked — enable it in settings (,)".into();
            return;
        }
        if self.corpus.is_empty() {
            self.rescan_corpus();
        }
        if self.corpus.is_empty() {
            self.status = "the corpus has no embedded blocks yet — open a document and let a diff need a model".into();
            return;
        }
        if self.model.busy() {
            self.status = "the model is already working; try again in a moment".into();
            return;
        }
        let entries: Vec<(usize, Vec<f32>)> = self
            .corpus
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i, e.vector.clone()))
            .collect();
        let label = format!(
            "searching {} block(s) for {:?}",
            entries.len(),
            truncate(&query, 30)
        );
        self.model.request_search(query.clone(), entries, label.clone());
        push_log(&mut self.log, label);
        self.status = "asking physis…".into();
    }

    /// Open a corpus hit: its document, at its block.
    fn open_corpus_hit(&mut self, index: usize) {
        let Some((document, block)) = self
            .corpus
            .entries
            .get(index)
            .map(|e| (e.document.clone(), e.block_id.clone()))
        else {
            return;
        };
        let path = PathBuf::from(&document);
        if !path.exists() {
            self.status = format!("{} is no longer on disk", path.display());
            return;
        }
        self.open_path(path);
        if let Some(s) = self.session.as_ref() {
            if let Some(i) = s.doc.blocks.iter().position(|b| b.id == block) {
                self.sel = i;
                self.tab = Tab::Document;
                self.status = format!("opened {} at {}", short(&document), short(&block));
                return;
            }
        }
        self.status = format!("opened {} (that block is no longer there)", short(&document));
    }

    /// Turn the graph into the selected block's cross-document relations.
    ///
    /// Reuses the ranked-result list, so a corpus relation is navigated exactly
    /// the way a search hit is: `Enter` opens it. Observed similarity is an
    /// offer to look, never an edge that changes anything.
    fn show_corpus_relations(&mut self) {
        let Some(id) = self.selected_id() else { return };
        let Some(obs) = self.observation_for(&id).cloned() else {
            self.status = "this block has not been read by a model — nothing to relate".into();
            return;
        };
        if self.corpus.is_empty() {
            self.rescan_corpus();
        }
        let document = self.document_id();
        let hits = self.corpus.related(&obs.vector, &document, 10);
        if hits.is_empty() {
            self.status = format!(
                "no other document resembles this block ({})",
                self.corpus
            );
            return;
        }
        self.status = format!("{} related block(s) elsewhere — Enter opens one", hits.len());
        self.search = Some((format!("related to {}", short(&id)), hits));
        self.tab = Tab::Graph;
        self.graph_sel = 0;
    }

    /// The open document's path, which is what the corpus records as its id.
    fn document_id(&self) -> String {
        self.session
            .as_ref()
            .map(|s| s.doc.id.clone())
            .unwrap_or_default()
    }

    /// Hyperlinks in the selected block, with their targets resolved.
    fn hyperlinks_of_selected(&self) -> Vec<(String, String, LinkTarget)> {
        let Some(s) = self.session.as_ref() else { return Vec::new() };
        let Some(block) = s.doc.blocks.get(self.sel) else { return Vec::new() };
        hyperlinks(block.text(&s.doc.raw))
            .into_iter()
            .map(|l| (l.label.clone(), l.target.clone(), l.kind))
            .collect()
    }

    /// Follow the first hyperlink in the selected block that resolves to a
    /// document on disk. A URL is shown, never opened: opening a browser is not
    /// something a keypress in a text editor should do unasked.
    fn follow_hyperlink(&mut self) {
        let Some(s) = self.session.as_ref() else { return };
        let Some(block) = s.doc.blocks.get(self.sel) else { return };
        let base = s.path.parent().map(Path::to_path_buf).unwrap_or_default();
        let links = hyperlinks(block.text(&s.doc.raw));
        let Some(link) = links.first() else {
            self.status = "this block has no hyperlink".into();
            return;
        };
        match link.kind {
            LinkTarget::Document => match link.resolve(&base) {
                Some(path) if path.exists() => self.open_path(path),
                Some(path) => self.status = format!("{} does not exist", path.display()),
                None => self.status = format!("cannot resolve {}", link.target),
            },
            LinkTarget::Url => {
                self.status = format!("{} — a URL, not opened", link.target);
            }
            LinkTarget::Anchor => {
                self.status = format!("{} — a fragment of this document", link.target);
            }
            LinkTarget::Image => {
                self.status = format!("{} — an image, not a place to go", link.target);
            }
            LinkTarget::Other => {
                self.status = format!("{} — not a document or a URL", link.target);
            }
        }
    }

    // ── input routing ───────────────────────────────────────────────────────

    fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            self.help = false;
            return;
        }
        if code != KeyCode::Char('q') {
            self.quit_armed = false;
        }
        if self.input.is_some() {
            self.on_key_input(code);
            return;
        }
        match self.overlay {
            Overlay::Links => return self.on_key_links(code),
            Overlay::History => return self.on_key_history(code),
            Overlay::Settings => return self.on_key_settings(code),
            Overlay::None => {}
        }
        // The prompt is global: one line, two verbs.
        if code == KeyCode::Char(',') {
            self.overlay = Overlay::Settings;
            self.settings_sel = 0;
            return;
        }
        if code == KeyCode::Char(':') {
            self.input = Some(Input {
                buffer: Vec::new(),
                cursor: 0,
                kind: InputKind::Physis,
                label: "t <command> runs a shell line; anything else asks physis".into(),
            });
            self.status = "t <command> for a shell · anything else searches the corpus · Esc cancels"
                .into();
            return;
        }
        // Tab switching is global, so no view is a trap.
        match code {
            KeyCode::Tab => {
                self.tab = Tab::ALL[(self.tab.index() + 1) % Tab::ALL.len()];
                return;
            }
            KeyCode::BackTab => {
                self.tab = Tab::ALL[(self.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len()];
                return;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::Document;
                return;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Graph;
                return;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::Files;
                return;
            }
            KeyCode::Char('4') => {
                self.tab = Tab::Terminal;
                return;
            }
            _ => {}
        }
        match self.tab {
            Tab::Document => self.on_key_browse(code),
            Tab::Graph => self.on_key_graph(code),
            Tab::Files => self.on_key_files(code),
            Tab::Terminal => self.on_key_terminal(code),
        }
    }

    fn on_key_input(&mut self, code: KeyCode) {
        let Some(input) = self.input.as_mut() else { return };
        match code {
            KeyCode::Esc => self.cancel_input(),
            KeyCode::Enter => match input.kind {
                InputKind::Block { .. } => self.commit_edit(),
                InputKind::Factor { .. } => self.commit_factor(),
                InputKind::Command => self.run_command(),
                InputKind::NewFile => self.commit_new_file(),
                InputKind::Physis => self.commit_prompt(),
            },
            KeyCode::Char(c) => input.insert(c),
            KeyCode::Backspace => input.backspace(),
            KeyCode::Delete => input.delete(),
            KeyCode::Left => input.cursor = input.cursor.saturating_sub(1),
            KeyCode::Right => input.cursor = (input.cursor + 1).min(input.buffer.len()),
            KeyCode::Home => input.cursor = 0,
            KeyCode::End => input.cursor = input.buffer.len(),
            _ => {}
        }
    }

    /// Turn the typed factor into a registered `scale:<n>` transform. The
    /// module refuses unknown transform ids at proposal time, so validate here
    /// rather than creating a link that can never resolve.
    fn commit_factor(&mut self) {
        let Some(input) = self.input.take() else { return };
        let (source, dependent) = match &input.kind {
            InputKind::Factor { source, dependent } => (source.clone(), dependent.clone()),
            _ => return,
        };
        let typed = input.text();
        let Ok(n) = typed.trim().parse::<i64>() else {
            self.status = format!("\"{typed}\" is not an integer factor — try again or Esc");
            self.input = Some(Input {
                buffer: typed.chars().collect(),
                cursor: typed.chars().count(),
                kind: InputKind::Factor { source, dependent },
                label: "calculate: factor (integer) — the dependent's first number is replaced".into(),
            });
            return;
        };
        let transform = format!("scale:{n}");
        if apply_transform(&transform, "1", "1").is_none() {
            self.status = format!("{transform} is not a registered transform");
            return;
        }
        // Re-create the link now that the factor is known.
        self.marked = Some(source);
        self.sel = self
            .session
            .as_ref()
            .and_then(|s| s.doc.blocks.iter().position(|b| b.id == dependent))
            .unwrap_or(self.sel);
        self.link(LinkBehavior::Calculate, Some(transform));
    }

    fn on_key_browse(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit_or_warn(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('o') => {
                let dir = self
                    .session
                    .as_ref()
                    .and_then(|s| s.path.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    });
                self.picker = Picker::load(dir);
                self.tab = Tab::Files;
            }
            KeyCode::Char('L') => {
                self.link_sel = 0;
                self.overlay = Overlay::Links;
            }
            KeyCode::Char('h') => {
                self.hist_sel = 0;
                self.overlay = Overlay::History;
            }
            KeyCode::Char('w') => self.write_back(),
            KeyCode::Char('R') => self.force_reload(),
            KeyCode::Down | KeyCode::Char('j') => self.move_block(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_block(-1),
            KeyCode::Char('g') => self.sel = 0,
            KeyCode::Char('G') => {
                self.sel = self
                    .session
                    .as_ref()
                    .map(|s| s.doc.blocks.len().saturating_sub(1))
                    .unwrap_or(0)
            }
            KeyCode::Enter | KeyCode::Char('e') => self.start_edit(),
            KeyCode::Char(' ') => self.mark(),
            KeyCode::Char('c') => self.link(LinkBehavior::Copy, Some("copy".into())),
            KeyCode::Char('v') => self.link(LinkBehavior::Review, None),
            KeyCode::Char('x') => self.start_factor(),
            KeyCode::Char('a') => self.accept(),
            KeyCode::Char('r') => self.reject(),
            KeyCode::Char('u') => self.undo(),
            _ => {}
        }
    }

    fn start_factor(&mut self) {
        let Some(source) = self.marked.clone() else {
            self.status = "mark a source first: Space on the block to change from".into();
            return;
        };
        let Some(dependent) = self.selected_id() else { return };
        if source == dependent {
            self.status = "source and dependent are the same block".into();
            return;
        }
        self.input = Some(Input {
            buffer: "2".chars().collect(),
            cursor: 1,
            kind: InputKind::Factor { source, dependent },
            label: "calculate: factor (integer) — the dependent's first number is replaced".into(),
        });
        self.status = "Enter confirms the factor · Esc cancels".into();
    }

    fn move_block(&mut self, delta: i64) {
        let n = self.session.as_ref().map(|s| s.doc.blocks.len()).unwrap_or(0);
        if n == 0 {
            return;
        }
        self.sel = (self.sel as i64 + delta).clamp(0, n as i64 - 1) as usize;
    }

    fn quit_or_warn(&mut self) {
        let modified = self.session.as_ref().map(|s| s.modified()).unwrap_or(false);
        if modified && !self.quit_armed {
            self.quit_armed = true;
            self.status =
                "the file has unwritten changes — w writes them, q again quits anyway".into();
            return;
        }
        self.quit = true;
    }

    fn on_key_files(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.delete_armed = None;
                self.quit_or_warn();
            }
            KeyCode::Char('n') => {
                self.input = Some(Input {
                    buffer: Vec::new(),
                    cursor: 0,
                    kind: InputKind::NewFile,
                    label: "new file name — created in the directory listed".into(),
                });
                self.status = "Enter creates the file and opens it · Esc cancels".into();
            }
            KeyCode::Char('d') => self.arm_delete(),
            KeyCode::Char('e') => match self.picker.selected().cloned() {
                Some(entry) if !entry.is_dir => {
                    self.pending_editor = Some(entry.path);
                    self.status = "handing the file to $EDITOR — the editor takes the screen".into();
                }
                Some(_) => self.status = "that is a directory".into(),
                None => {}
            },
            KeyCode::Char('g') => {
                if let Some(entry) = self.picker.selected().cloned() {
                    if entry.is_dir {
                        self.status = "that is a directory".into();
                    } else {
                        self.open_path(entry.path);
                        self.tab = Tab::Graph;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.picker.move_by(1),
            KeyCode::Up | KeyCode::Char('k') => self.picker.move_by(-1),
            KeyCode::Backspace => {
                let up = self.picker.dir.parent().map(Path::to_path_buf);
                if let Some(up) = up {
                    self.picker = Picker::load(up);
                }
            }
            KeyCode::Enter => {
                let Some(entry) = self.picker.selected().cloned() else { return };
                if entry.is_dir {
                    self.picker = Picker::load(entry.path);
                } else {
                    self.open_path(entry.path);
                }
            }
            _ => {}
        }
    }

    fn on_key_links(&mut self, code: KeyCode) {
        let n = self.session.as_ref().map(|s| s.doc.links.len()).unwrap_or(0);
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('L') => self.overlay = Overlay::None,
            KeyCode::Down | KeyCode::Char('j') if n > 0 => {
                self.link_sel = (self.link_sel + 1) % n;
            }
            KeyCode::Up | KeyCode::Char('k') if n > 0 => {
                self.link_sel = (self.link_sel + n - 1) % n;
            }
            KeyCode::Char('t') => self.toggle_link(),
            KeyCode::Char('d') => self.delete_link(),
            KeyCode::Char('r') => self.reanchor(),
            _ => {}
        }
    }

    /// Two rows, cycled in place and saved as they change. A setting that needs
    /// a separate save is a setting people lose.
    fn on_key_settings(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(',') => self.overlay = Overlay::None,
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_sel = (self.settings_sel + 1) % 2;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_sel = (self.settings_sel + 1) % 2;
            }
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => self.cycle_setting(1),
            KeyCode::Left => self.cycle_setting(-1),
            _ => {}
        }
    }

    fn cycle_setting(&mut self, delta: i32) {
        if self.settings_sel == 0 {
            let next = if delta >= 0 {
                self.settings.model.next()
            } else {
                self.settings.model.next().next()
            };
            self.settings.model = next;
        } else {
            let all = annotate::Settings::EMBEDDERS;
            let i = all
                .iter()
                .position(|e| *e == self.settings.embedder)
                .unwrap_or(0);
            let n = all.len() as i32;
            let j = ((i as i32 + delta).rem_euclid(n)) as usize;
            self.settings.embedder = all[j].to_string();
        }
        self.apply_settings();
    }

    fn apply_settings(&mut self) {
        let path = self.store_home.join("settings.json");
        let saved = match self.settings.save(&path) {
            Ok(()) => String::new(),
            Err(e) => format!(" (not saved: {e})"),
        };
        self.model.set_settings(self.settings.clone());
        self.status = format!(
            "model {} · embedder {}{saved}",
            self.settings.model.label(),
            self.settings.embedder_label()
        );
        push_log(&mut self.log, self.status.clone());
        if self.settings.model == annotate::ModelPolicy::Eager {
            self.model.warm();
        }
    }

    fn on_key_history(&mut self, code: KeyCode) {
        let n = self
            .session
            .as_ref()
            .map(|s| s.events.len() + s.patches.patches.len())
            .unwrap_or(0);
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => self.overlay = Overlay::None,
            KeyCode::Down | KeyCode::Char('j') if n > 0 => {
                self.hist_sel = (self.hist_sel + 1).min(n.saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.hist_sel = self.hist_sel.saturating_sub(1);
            }
            KeyCode::Char('g') => self.hist_sel = 0,
            _ => {}
        }
    }
}

/// Blocks read per model request. Bounded so a large document cannot turn one
/// transition into a minute of embedding.
const MAX_ANNOTATE: usize = 40;

/// Project a transition's successions into the causal ledger and persist it.
///
/// Free function rather than a method so it can run while a `Session` is
/// borrowed: `self.log` and `self.session` are disjoint fields, but a `&mut
/// self` method would borrow both.
fn project_causes(
    ledger: &mut CausalLedger,
    t: &Transition,
    actor: &str,
    store: &Store,
    status: &mut String,
) {
    for (now_id, was_id) in t.successions() {
        ledger.record(Mechanism::Corrected, actor, now_id, Some(was_id));
    }
    if let Err(e) = store.save_causes(ledger) {
        *status = format!("causal ledger write failed: {e}");
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn short(id: &str) -> String {
    id.chars().take(18).collect()
}

fn truncate(s: &str, max: usize) -> String {
    let one_line = s.replace('\n', " ");
    if one_line.chars().count() <= max {
        one_line
    } else {
        one_line.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

// ── terminal ────────────────────────────────────────────────────────────────

struct Term {
    term: Terminal<CrosstermBackend<Stdout>>,
}

impl Term {
    fn new() -> Result<Term, Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut out = std::io::stdout();
        execute!(out, EnterAlternateScreen)?;
        let term = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Term { term })
    }
}

impl Term {
    /// Give the screen back, for a child that needs it (an editor, a pager).
    fn suspend(&mut self) -> std::io::Result<()> {
        disable_raw_mode()?;
        execute!(self.term.backend_mut(), LeaveAlternateScreen)
    }

    /// Take it back, and repaint from scratch: whatever the child drew is
    /// still in the scrollback otherwise.
    fn resume(&mut self) -> std::io::Result<()> {
        enable_raw_mode()?;
        execute!(self.term.backend_mut(), EnterAlternateScreen)?;
        self.term.clear()
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.term.backend_mut(), LeaveAlternateScreen);
        let _ = self.term.show_cursor();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // An optional path is how every TUI editor starts; it is not a command line.
    let arg = std::env::args().nth(1);
    let mut app = match arg {
        Some(p) => {
            let path = PathBuf::from(p);
            if path.is_dir() {
                App::new(path)
            } else {
                let mut app = App::new(
                    path.parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from(".")),
                );
                app.open_path(path);
                app
            }
        }
        None => App::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
    };
    let mut term = Term::new()?;
    while !app.quit {
        if let Some(path) = app.pending_editor.take() {
            app.status = run_editor(&mut term, &path);
            // The editor may have rewritten the file. Force a poll past the
            // throttle so the document picks it up immediately.
            app.last_poll = Instant::now() - Duration::from_secs(5);
        }
        app.poll_file();
        app.poll_model();
        app.poll_terminal();
        term.term.draw(|f| draw(f, &app))?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.on_key(key.code, key.modifiers);
            }
        }
    }
    Ok(())
}

// ── drawing ─────────────────────────────────────────────────────────────────

/// Hand the terminal to `$EDITOR` and take it back. Returns what to report.
fn run_editor(term: &mut Term, path: &Path) -> String {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let args: Vec<&str> = parts.collect();
    if term.suspend().is_err() {
        return "could not leave the alternate screen".into();
    }
    let result = std::process::Command::new(program)
        .args(&args)
        .arg(path)
        .status();
    let _ = term.resume();
    match result {
        Ok(s) if s.success() => format!(
            "{editor} closed — the document is reloaded if the file changed"
        ),
        Ok(s) => format!("{editor} exited with {s}"),
        Err(e) => format!("cannot run {editor}: {e}"),
    }
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    let size = f.size();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(size);

    draw_header(f, rows[0], app);

    match app.tab {
        Tab::Document => match app.session.as_ref() {
            Some(session) => {
                if let Some(live) = &app.live() {
                    let cols = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
                        .split(rows[1]);
                    draw_document(f, cols[0], app, live);
                    draw_why(f, cols[1], app, session, live);
                }
            }
            None => draw_files(f, rows[1], app),
        },
        Tab::Graph => draw_graph(f, rows[1], app),
        Tab::Files => draw_files(f, rows[1], app),
        Tab::Terminal => draw_terminal(f, rows[1], app),
    }

    draw_footer(f, rows[2], app);

    match app.overlay {
        Overlay::Links => draw_links(f, size, app),
        Overlay::History => draw_history(f, size, app),
        Overlay::Settings => draw_settings(f, size, app),
        Overlay::None => {}
    }
    if app.help {
        draw_help(f, size);
    }
}

fn draw_header(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let mut spans = vec![Span::styled(
        " physis-pro-doc ",
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let style = if *tab == app.tab {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("{}", i + 1), style));
        spans.push(Span::styled(
            tab.title(),
            if *tab == app.tab {
                style
            } else {
                Style::default().fg(Color::Gray)
            },
        ));
    }
    match &app.session {
        None => spans.push(Span::raw("  no document open")),
        Some(s) => {
            spans.push(Span::raw(format!(
                "  {}  rev {} · graph {} · {} blocks · {} links",
                truncate(&s.path.display().to_string(), 46),
                s.doc.revision,
                s.doc.graph_revision,
                s.doc.blocks.len(),
                s.doc.links.len()
            )));
            if s.modified() {
                spans.push(Span::styled(
                    "  ● modified (w writes)",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            }
            if s.conflict {
                spans.push(Span::styled(
                    "  ⇄ file changed on disk (R reloads)",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(p) = &s.pending {
                spans.push(Span::styled(
                    format!(
                        "  PENDING {} ({} replacement{}, {} review)",
                        p.id,
                        p.replacements.len(),
                        if p.replacements.len() == 1 { "" } else { "s" },
                        p.review_only.len()
                    ),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        area,
    );
}

fn draw_footer(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let keys: &str = if app.input.is_some() {
        match app.input.as_ref().map(|i| &i.kind) {
            Some(InputKind::Command) => " Enter runs it · Esc leaves the shell · output appears above ",
            Some(InputKind::NewFile) => " Enter creates the file and opens it · Esc cancels ",
            _ => " ←/→ Home/End · ⌫ Del · Enter confirm · Esc cancel ",
        }
    } else {
        match app.overlay {
            Overlay::Links => " j/k select · t toggle active · d delete · r re-anchor · Esc close ",
            Overlay::History => " j/k scroll · g top · Esc close ",
            Overlay::Settings => " j/k row · ←/→ change · saved as you change it · Esc close ",
            Overlay::None => match app.tab {
                Tab::Document => {
                    " j/k · e edit · c/x/v link · a accept · u undo · w write · : physis · L links · h history · ? help · q quit "
                }
                Tab::Graph => {
                    " j/k node · Enter open · f link · x corpus relations · r re-anchor · c rescan · : physis "
                }
                Tab::Files => {
                    " j/k · Enter physis doc · e $EDITOR · g graph · ⌫ up · n new · d delete · : prompt "
                }
                Tab::Terminal => {
                    " Enter/i shell · j/k scroll · c clear · : physis · 1-4 tabs · q quit — runs as you "
                }
            },
        }
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(keys, Style::default().fg(Color::Cyan))),
            Line::from(Span::styled(
                format!(" {}", app.status),
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        area,
    );
}

fn draw_files(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .picker
        .entries
        .iter()
        .map(|e| {
            let (icon, style) = if e.is_dir {
                ("▸", Style::default().fg(Color::Cyan))
            } else {
                ("·", Style::default().fg(Color::Gray))
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{icon} "), style),
                Span::raw(e.name.clone()),
            ]))
        })
        .collect();
    let height = area.height.saturating_sub(2) as usize;
    let mut offset = 0usize;
    if height > 0 && app.picker.sel >= height {
        offset = app.picker.sel + 1 - height;
    }
    let mut state = ListState::default();
    state.select(if app.picker.entries.is_empty() {
        None
    } else {
        Some(app.picker.sel)
    });
    *state.offset_mut() = offset;
    let list = List::new(items).block(
        Block::default()
            .title(format!(" open a document — {} ", app.picker.dir.display()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_stateful_widget(list, area, &mut state);
    if let Some(e) = &app.picker.error {
        f.render_widget(
            Clear,
            Rect {
                x: area.x + 2,
                y: area.y + 1,
                width: area.width.saturating_sub(4),
                height: 3,
            },
        );
        f.render_widget(
            Paragraph::new(Span::styled(e.clone(), Style::default().fg(Color::Red))).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            ),
            Rect {
                x: area.x + 2,
                y: area.y + 1,
                width: area.width.saturating_sub(4),
                height: 3,
            },
        );
    }
}

fn draw_document(f: &mut ratatui::Frame, area: Rect, app: &App, live: &Live) {
    let editing = app.input.is_some();
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(if editing { 3 } else { 0 }),
        ])
        .split(area);

    let items: Vec<ListItem> = live
        .doc
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let selected = i == app.sel;
            let changed = live.changed.contains(&b.id);
            let marked = app.marked.as_deref() == Some(b.id.as_str());
            let kind = match b.kind {
                BlockKind::Heading => "H",
                BlockKind::Paragraph => "P",
                BlockKind::Code => "C",
                BlockKind::Other => "?",
            };
            let text = truncate(b.text(&live.doc.raw), 56);
            let mut spans = vec![
                Span::styled(
                    if selected { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(format!("{kind} "), Style::default().fg(Color::DarkGray)),
                Span::raw(text),
            ];
            if changed {
                spans.push(Span::styled(
                    "  ◆ adapts",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ));
            }
            if marked {
                spans.push(Span::styled("  *source", Style::default().fg(Color::Yellow)));
            }
            if b.locked {
                spans.push(Span::styled("  locked", Style::default().fg(Color::DarkGray)));
            }
            let mut line = Line::from(spans);
            if selected {
                line = line.style(Style::default().bg(Color::Rgb(30, 40, 55)));
            }
            ListItem::new(line)
        })
        .collect();

    let height = parts[0].height.saturating_sub(2) as usize;
    let mut offset = app.offset;
    if app.sel < offset {
        offset = app.sel;
    } else if height > 0 && app.sel >= offset + height {
        offset = app.sel + 1 - height;
    }
    let mut state = ListState::default();
    state.select(if live.doc.blocks.is_empty() {
        None
    } else {
        Some(app.sel)
    });
    *state.offset_mut() = offset;

    let title = if live.changed.is_empty() {
        format!(" document · {} blocks ", live.doc.blocks.len())
    } else {
        format!(
            " document (showing {} proposed change{}) ",
            live.changed.len(),
            if live.changed.len() == 1 { "" } else { "s" }
        )
    };
    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if live.changed.is_empty() {
                Color::DarkGray
            } else {
                Color::Green
            })),
    );
    f.render_stateful_widget(list, parts[0], &mut state);

    if let Some(input) = &app.input {
        let width = parts[1].width.saturating_sub(4) as usize;
        let start = input.cursor.saturating_sub(width.saturating_sub(1));
        let before: String = input.buffer[start..input.cursor].iter().collect();
        let at = input.buffer.get(input.cursor).copied().unwrap_or(' ');
        let after: String = input.buffer.iter().skip(input.cursor + 1).take(width).collect();
        let line = Line::from(vec![
            Span::raw(before),
            Span::styled(at.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow)),
            Span::raw(after),
        ]);
        f.render_widget(
            Paragraph::new(line).block(
                Block::default()
                    .title(format!(" {} ", input.label))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ),
            parts[1],
        );
    }
}

fn draw_why(f: &mut ratatui::Frame, area: Rect, app: &App, session: &Session, live: &Live) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(9)])
        .split(area);

    let mut lines: Vec<Line> = Vec::new();
    if let Some(e) = &live.error {
        lines.push(Line::from(Span::styled(
            format!(" {e}"),
            Style::default().fg(Color::Yellow),
        )));
    }
    match &live.preview {
        Some((_, patch)) => {
            lines.push(Line::from(Span::styled(
                " live preview — nothing persisted until Enter",
                Style::default().fg(Color::Yellow),
            )));
            push_patch_lines(&mut lines, patch);
        }
        None => match &session.pending {
            Some(p) => {
                lines.push(Line::from(Span::styled(
                    format!(" pending patch {}", p.id),
                    Style::default().fg(Color::Yellow),
                )));
                push_patch_lines(&mut lines, p);
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " a accept · r reject",
                    Style::default().fg(Color::Cyan),
                )));
            }
            None => {
                for l in [
                    " no pending change.",
                    " e edits the selected block; Space then c/x/v declares a",
                    " dependency, and confirmed dependents adapt as you type.",
                ] {
                    lines.push(Line::from(Span::styled(
                        l,
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        },
    }
    lines.push(Line::from(""));
    if app.model.busy() {
        lines.push(Line::from(Span::styled(
            format!(" {}", app.model.label),
            Style::default().fg(Color::Yellow),
        )));
    } else if !session.observations.is_empty() {
        let embedder = session
            .observations
            .last()
            .map(|o| o.embedder.clone())
            .unwrap_or_default();
        let isolated = session.observations.iter().filter(|o| o.isolated).count();
        lines.push(Line::from(Span::styled(
            format!(
                " model [{embedder}]: {} block(s) read, {isolated} isolated here",
                session.observations.len()
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }
    if let Some(t) = session.transitions.last() {
        lines.push(Line::from(Span::styled(
            format!(" last transition — {}", t.summary()),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!(
            " {} link(s) · {} active · {} dangling · undo {}",
            session.doc.links.len(),
            session.doc.links.iter().filter(|l| l.active).count(),
            session
                .doc
                .links
                .iter()
                .filter(|l| !link_resolves(&session.doc, l))
                .count(),
            session
                .last_applied
                .as_ref()
                .map(|p| p.id.as_str())
                .unwrap_or("—")
        ),
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" why / preview ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if live.preview.is_some() {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    })),
            )
            .wrap(Wrap { trim: false }),
        parts[0],
    );

    let start = app.log.len().saturating_sub(7);
    let events: Vec<Line> = app
        .log
        .iter()
        .skip(start)
        .map(|l| Line::from(Span::styled(format!(" {l}"), Style::default().fg(Color::Gray))))
        .collect();
    f.render_widget(
        Paragraph::new(events)
            .block(
                Block::default()
                    .title(" session log · h for full history ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: true }),
        parts[1],
    );
}

fn push_patch_lines(lines: &mut Vec<Line>, patch: &Patch) {
    for r in &patch.replacements {
        lines.push(Line::from(vec![
            Span::styled("  replace  ", Style::default().fg(Color::Green)),
            Span::styled(truncate(&r.expected_old_bytes, 22), Style::default().fg(Color::Red)),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(truncate(&r.new_bytes, 22), Style::default().fg(Color::Green)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("           via {}", r.reasons.join(", ")),
            Style::default().fg(Color::DarkGray),
        )));
    }
    for n in &patch.review_only {
        lines.push(Line::from(vec![
            Span::styled("  review   ", Style::default().fg(Color::Yellow)),
            Span::styled(truncate(&n.reason, 40), Style::default().fg(Color::Yellow)),
        ]));
    }
    if patch.replacements.is_empty() && patch.review_only.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no dependent reacts to this block.",
            Style::default().fg(Color::DarkGray),
        )));
    }
}

fn overlay_area(size: Rect, h_pct: u16, w_pct: u16) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - h_pct) / 2),
            Constraint::Percentage(h_pct),
            Constraint::Percentage((100 - h_pct) / 2),
        ])
        .split(size);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - w_pct) / 2),
            Constraint::Percentage(w_pct),
            Constraint::Percentage((100 - w_pct) / 2),
        ])
        .split(rows[1]);
    cols[1]
}

/// The document's own graph: blocks as nodes, links and recorded successions
/// as edges. There is no layout algorithm — a node list with the selected
/// node's edges beside it is the honest amount of structure for a document
/// this size, and it stays readable at any terminal width.
fn draw_graph(f: &mut ratatui::Frame, area: Rect, app: &App) {
    if app.session.is_none() {
        f.render_widget(
            Paragraph::new(Span::styled(
                " open a document first — the graph is this document's own",
                Style::default().fg(Color::DarkGray),
            ))
            .block(Block::default().borders(Borders::ALL).border_style(
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }
    let nodes = app.graph_nodes();
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    let items: Vec<ListItem> = match &app.search {
        Some((_, hits)) => hits
            .iter()
            .map(|(i, score)| {
                let e = &app.corpus.entries[*i];
                let cell = e
                    .cell
                    .as_ref()
                    .map(|(d, m)| format!("{d}×{m}"))
                    .unwrap_or_else(|| "—".into());
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{score:.3} "), Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{:<9}", truncate(&cell, 9)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(format!("{}  ", truncate(&e.text, 38))),
                    Span::styled(
                        truncate(&short(&e.document), 22),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect(),
        None => nodes
            .iter()
            .map(|n| {
                let (mark, colour) = if n.live {
                    ("●", Color::Green)
                } else {
                    ("○", Color::Red)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{mark} "), Style::default().fg(colour)),
                    Span::styled(format!("{:<2}", n.kind), Style::default().fg(Color::DarkGray)),
                    Span::styled(short(&n.id), Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("  {}", n.text)),
                ]))
            })
            .collect(),
    };

    let height = cols[0].height.saturating_sub(2) as usize;
    let mut offset = app.graph_offset;
    if app.graph_sel < offset {
        offset = app.graph_sel;
    } else if height > 0 && app.graph_sel >= offset + height {
        offset = app.graph_sel + 1 - height;
    }
    let mut state = ListState::default();
    state.select(if nodes.is_empty() { None } else { Some(app.graph_sel) });
    *state.offset_mut() = offset;
    let live = nodes.iter().filter(|n| n.live).count();
    let title = match &app.search {
        Some((query, hits)) => format!(
            " physis — {} hit(s) for {:?} · Enter opens one · c back to the graph ",
            hits.len(),
            truncate(query, 30)
        ),
        None => format!(
            " graph — {} live block(s), {} gone · {} · Enter opens · r re-anchors ",
            live,
            nodes.len() - live,
            app.corpus
        ),
    };
    f.render_stateful_widget(
        List::new(items).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        cols[0],
        &mut state,
    );

    let mut lines: Vec<Line> = Vec::new();
    match nodes.get(app.graph_sel) {
        None => lines.push(Line::from(Span::styled(
            " no blocks",
            Style::default().fg(Color::DarkGray),
        ))),
        Some(node) => {
            lines.push(Line::from(Span::styled(
                format!(" {}{}", if node.live { "" } else { "(gone) " }, node.text),
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(Span::styled(
                format!(" {}", short(&node.id)),
                Style::default().fg(Color::DarkGray),
            )));
            if let Some(obs) = app.observation_for(&node.id) {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " model",
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    format!("   {}", obs.summary()),
                    Style::default().fg(Color::Gray),
                )));
                for (id, score) in obs.neighbours.iter().take(3) {
                    lines.push(Line::from(Span::styled(
                        format!("   {:.2}  {}", score, short(id)),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                if obs.isolated {
                    lines.push(Line::from(Span::styled(
                        "   unlike anything else in this document",
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }
            let links = app.hyperlinks_of_selected();
            if !links.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " hyperlinks  (f follows the first)",
                    Style::default().fg(Color::Cyan),
                )));
                for (label, target, kind) in links {
                    lines.push(Line::from(Span::styled(
                        format!("   [{label}]({target})  {kind:?}"),
                        Style::default().fg(Color::Gray),
                    )));
                }
            }
            if let Some(obs) = app.observation_for(&node.id) {
                let corpus = app.corpus.related(&obs.vector, &app.document_id(), 3);
                if !corpus.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        " corpus relations",
                        Style::default().fg(Color::Cyan),
                    )));
                    for (i, score) in corpus {
                        lines.push(Line::from(Span::styled(
                            format!(
                                "   {score:.3}  {}  {}",
                                truncate(&app.corpus.entries[i].text, 30),
                                short(&app.corpus.entries[i].document)
                            ),
                            Style::default().fg(Color::Gray),
                        )));
                    }
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " edges",
                Style::default().fg(Color::Cyan),
            )));
            let edges = app.edges_of(&node.id);
            if edges.is_empty() {
                lines.push(Line::from(Span::styled(
                    "   none declared",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            for e in edges {
                lines.push(Line::from(Span::styled(
                    format!("   {e}"),
                    Style::default().fg(Color::Gray),
                )));
            }
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" selected ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

/// A shell, and what it said. The only input is a line a person typed.
fn draw_terminal(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(3)])
        .split(area);

    let mut lines: Vec<Line> = Vec::new();
    for o in &app.term_out {
        lines.push(Line::from(vec![
            Span::styled(
                format!("$ {}", o.command),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!(
                    "   [{}]",
                    match o.code {
                        Some(c) => c.to_string(),
                        None => "no status".into(),
                    }
                ),
                Style::default().fg(if o.ok() { Color::Green } else { Color::Red }),
            ),
        ]));
        for l in o.stdout.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {l}"),
                Style::default().fg(Color::Gray),
            )));
        }
        for l in o.stderr.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {l}"),
                Style::default().fg(Color::Red),
            )));
        }
        if o.truncated {
            lines.push(Line::from(Span::styled(
                "  … output truncated, tail kept",
                Style::default().fg(Color::Yellow),
            )));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " nothing run yet. Enter starts a shell line; it runs as you, in the",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            " document's directory. Nothing here is ever run automatically.",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let inner = rows[0].height.saturating_sub(2) as usize;
    let from = lines.len().saturating_sub(inner + app.term_scroll);
    let visible: Vec<Line> = lines.into_iter().skip(from).take(inner).collect();
    f.render_widget(
        Paragraph::new(visible)
            .block(
                Block::default()
                    .title(format!(
                        " terminal — {} command(s){} · {} ",
                        app.term_out.len(),
                        if app.term.busy() { " · running" } else { "" },
                        app.term_out
                            .last()
                            .map(|o| truncate(&o.cwd.display().to_string(), 40))
                            .unwrap_or_else(|| "no directory yet".into())
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if app.term.busy() {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    })),
            )
            .wrap(Wrap { trim: false }),
        rows[0],
    );

    let (text, style) = match &app.input {
        Some(i) if matches!(i.kind, InputKind::Command) => (
            i.text(),
            Style::default().fg(Color::Yellow),
        ),
        _ => (
            "(Enter or i for a shell line)".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style))).block(
            Block::default()
                .title(" shell ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if app.input.is_some() {
                    Color::Yellow
                } else {
                    Color::DarkGray
                })),
        ),
        rows[1],
    );
}

fn draw_links(f: &mut ratatui::Frame, size: Rect, app: &App) {
    let area = overlay_area(size, 70, 76);
    f.render_widget(Clear, area);
    let Some(s) = &app.session else { return };
    // One item per link, plus a second line naming the block that took a
    // dangling endpoint's place — the recorded displacement, not a guess.
    let mut items: Vec<ListItem> = Vec::new();
    for l in &s.doc.links {
        let resolves = link_resolves(&s.doc, l);
        let (state, colour) = if !resolves {
            ("dangling", Color::Red)
        } else if l.active {
            ("active  ", Color::Green)
        } else {
            ("inactive", Color::DarkGray)
        };
        let src = s
            .doc
            .block(&l.source_block_id)
            .map(|b| truncate(b.text(&s.doc.raw), 26))
            .unwrap_or_else(|| "(missing)".into());
        let dst = s
            .doc
            .block(&l.dependent_block_id)
            .map(|b| truncate(b.text(&s.doc.raw), 26))
            .unwrap_or_else(|| "(missing)".into());
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{:<9}", l.id), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{state} "), Style::default().fg(colour)),
            Span::styled(
                format!("{:<9}", format!("{:?}", l.behavior).to_lowercase()),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{:<12}", l.transform_id.clone().unwrap_or_else(|| "—".into())),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{src} → {dst}")),
        ])));
        if resolves {
            continue;
        }
        let missing = if s.doc.block(&l.source_block_id).is_none() {
            &l.source_block_id
        } else {
            &l.dependent_block_id
        };
        let offer = s
            .transitions
            .iter()
            .rev()
            .find_map(|t| t.candidates_for(missing).first());
        let line = match offer {
            Some(c) => Line::from(Span::styled(
                format!(
                    "         r re-anchor → {}  ({} shared word(s){})",
                    truncate(
                        s.doc
                            .block(&c.to)
                            .map(|b| b.text(&s.doc.raw))
                            .unwrap_or("(unknown)"),
                        34
                    ),
                    c.shared_tokens,
                    if c.positional { ", positional" } else { "" }
                ),
                Style::default().fg(Color::Yellow),
            )),
            None => Line::from(Span::styled(
                "         no replacement recorded for that block",
                Style::default().fg(Color::DarkGray),
            )),
        };
        items.push(ListItem::new(line));
    }
    let mut state = ListState::default();
    state.select(if s.doc.links.is_empty() {
        None
    } else {
        Some(app.link_sel.min(s.doc.links.len() - 1))
    });
    let list = List::new(items).block(
        Block::default()
            .title(format!(
                " links — {} declared, {} active · t toggle · d delete ",
                s.doc.links.len(),
                s.doc.links.iter().filter(|l| l.active).count()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_history(f: &mut ratatui::Frame, size: Rect, app: &App) {
    let area = overlay_area(size, 76, 80);
    f.render_widget(Clear, area);
    let Some(s) = &app.session else { return };

    // Events replay in store order, which is what the file says. Patches are
    // bookkeeping, not a second history, so they are listed separately.
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            format!(
                "events.jsonl — {} transition(s), replay order = store sequence",
                s.events.len()
            ),
            Style::default().fg(Color::Cyan),
        )),
    ];
    for (i, e) in s.events.iter().enumerate() {
        let selected = i == app.hist_sel;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  #{:<3} {:<22} rev {} → {}  {}",
                i,
                truncate(&e.operation, 22),
                e.before_revision,
                e.after_revision,
                truncate(&e.event_id, 40)
            ),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("patches.json — {} record(s)", s.patches.patches.len()),
        Style::default().fg(Color::Cyan),
    )));
    for p in &s.patches.patches {
        lines.push(Line::from(Span::styled(
            format!(
                "  {:<6} {:<9} rev {} → {}  {} replacement(s), {} review",
                p.id,
                format!("{:?}", p.status).to_lowercase(),
                p.expected_document_revision,
                p.applied_revision
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "—".into()),
                p.replacements.len(),
                p.review_only.len()
            ),
            Style::default().fg(Color::Gray),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "transitions.jsonl — {} external passage(s), block identity across each diff",
            s.transitions.len()
        ),
        Style::default().fg(Color::Cyan),
    )));
    for t in &s.transitions {
        lines.push(Line::from(Span::styled(
            format!("  {}", t.summary()),
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "     {} · diff {:016x}",
                t.complexity(),
                t.diff_hash
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "the store keeps bytes, not a rendered history: this is the log as written.",
        Style::default().fg(Color::DarkGray),
    )));

    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll = app.hist_sel.min(lines.len().saturating_sub(1));
    let scroll = scroll.min(lines.len().saturating_sub(inner_height.min(lines.len())));
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" history ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// The two decisions the operator makes about the model, and what each costs.
fn draw_settings(f: &mut ratatui::Frame, size: Rect, app: &App) {
    let area = overlay_area(size, 60, 76);
    f.render_widget(Clear, area);
    let row = |sel: bool, name: &str, value: &str, note: &str| -> Vec<Line> {
        vec![
            Line::from(vec![
                Span::styled(
                    if sel { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{name:<10}"),
                    Style::default()
                        .fg(if sel { Color::White } else { Color::Gray })
                        .add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::styled(value.to_string(), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(Span::styled(
                format!("            {note}"),
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " These are yours to decide: the cost is yours too.",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));
    lines.extend(row(
        app.settings_sel == 0,
        "model",
        app.settings.model.label(),
        match app.settings.model {
            annotate::ModelPolicy::Off => {
                "no session, no vectors. The diff, the ledger, the graph, the files pane and the terminal all work."
            }
            annotate::ModelPolicy::Lazy => {
                "a session is built the first time a diff produces something the exact evidence could not account for."
            }
            annotate::ModelPolicy::Eager => {
                "a session is built when a document opens, and the whole document is read so the corpus sees it."
            }
        },
    ));
    lines.push(Line::from(""));
    lines.extend(row(
        app.settings_sel == 1,
        "embedder",
        app.settings.embedder_label(),
        match app.settings.embedder.as_str() {
            "" => "let the engine pick its best candidate — ONNX semantics, hundreds of MB, seconds to build.",
            "random-projection" => {
                "offline and model-free: instant, deterministic, no model file. Coarser meaning; good enough for ranking, not for reading."
            }
            _ => "named explicitly; if it is not available the engine falls back and the observation says which one answered.",
        },
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            " saved as you change it · {} · state: {}",
            app.store_home.join("settings.json").display(),
            match &app.model.state {
                annotate::State::Off => "off".to_string(),
                annotate::State::Idle => "no session".to_string(),
                annotate::State::Working => "building".to_string(),
                annotate::State::Ready(k) => format!("ready [{k}]"),
                annotate::State::Failed(e) => format!("failed: {e}"),
            }
        ),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        " j/k row · ←/→ change · Esc close",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" settings ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_help(f: &mut ratatui::Frame, size: Rect) {
    let area = overlay_area(size, 88, 78);
    f.render_widget(Clear, area);
    let text = vec![
        Line::from(Span::styled(
            "adaptive document — what each key does",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  1 Document · 2 Graph · 3 Files · 4 Terminal   (Tab cycles)",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("  Graph           blocks as nodes; links and recorded replacements"),
        Line::from("                  as edges. Enter opens the block, r re-anchors,"),
        Line::from("                  x shows the blocks in other documents that read alike."),
        Line::from("  Files           Enter opens in this editor, e hands the file to"),
        Line::from("                  $EDITOR, g opens its graph, n makes one, d deletes."),
        Line::from("  Terminal        a shell line, run as you in the document's"),
        Line::from("                  directory. Nothing is ever run automatically."),
        Line::from(""),
        Line::from(Span::styled(
            "  :               one prompt. `t <command>` is a shell line; anything",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "                  else is a question for physis, which ranks the corpus.",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ,               settings: whether a model may be loaded at all, and",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "                  which embedder. Off costs nothing and keeps every",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "                  other feature working; it only loses the reading.",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("  o               jump to the files pane"),
        Line::from("  j / k / ↑ ↓     select a block"),
        Line::from("  g / G           first / last block"),
        Line::from("  e or Enter      edit the selected block in place"),
        Line::from(""),
        Line::from("  while editing:  ← → Home End · ⌫ Del · Enter commit · Esc cancel"),
        Line::from("  the confirmed dependents change on screen as you type. Nothing"),
        Line::from("  is written until Enter, and the proposal is still yours to reject."),
        Line::from(""),
        Line::from("  Space           mark the selected block as a link SOURCE"),
        Line::from("  c               link marked → selected as COPY (verbatim source)"),
        Line::from("  x               link marked → selected as CALCULATE (asks factor)"),
        Line::from("  v               link marked → selected as REVIEW (flag only)"),
        Line::from(""),
        Line::from("  a               accept the pending patch (writes document bytes)"),
        Line::from("  r               reject it (zero document bytes change)"),
        Line::from("  u               undo the last applied patch"),
        Line::from("  w               write the document back to the file on disk"),
        Line::from("  R               reload from disk, discarding unwritten changes"),
        Line::from("  L               links: t toggle active, d delete, r re-anchor"),
        Line::from("  h               full history: events, patches and identity transitions"),
        Line::from(""),
        Line::from("  A dangling link can be re-anchored (r) to the block that took"),
        Line::from("  its place, using the succession recorded when it was replaced."),
        Line::from(""),
        Line::from("  Corpus relations are of two kinds and are kept apart: a hyperlink"),
        Line::from("  is a person saying \"this points there\" (exact, follow it with f),"),
        Line::from("  and a cosine between two documents' blocks is the model saying"),
        Line::from("  \"these read alike\" (offered, never an edge that propagates)."),
        Line::from(""),
        Line::from("  A model is loaded only when a diff produces something the exact"),
        Line::from("  evidence could not account for — an unpaired region, or several"),
        Line::from("  blocks appearing at once. It runs off the UI thread, it is"),
        Line::from("  attributed to the embedder that produced it, and it never decides"),
        Line::from("  identity or re-anchors anything."),
        Line::from(""),
        Line::from("  The file is watched. When it changes on disk the document is"),
        Line::from("  re-identified block by block: a block whose text was edited keeps"),
        Line::from("  its id and its links. A block that is gone is reported, never"),
        Line::from("  re-pointed. Editing in the app is never interrupted by this."),
        Line::from(""),
        Line::from(Span::styled(
            "  Block insertion and deletion are not offered: the model's identity",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  guarantee is byte-exact replacement in place. A new or removed",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  block would invalidate the ids the links point at.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  any key to dismiss",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title(" help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The AD04 fixture, as text a person would actually write. Tests need a
    /// document; the application no longer ships one.
    const SAMPLE: &str =
        "# Field report\n\nWe ordered 18 units of bolts.\n\nBudget is 36 euros.\n\nCheck supplier terms.\n";

    fn fixture() -> Document {
        let mut doc = parse_blocks("field-report", 0, SAMPLE);
        let source = doc.blocks[1].id.clone();
        let budget = doc.blocks[2].id.clone();
        let review = doc.blocks[3].id.clone();
        doc.links = vec![
            Link {
                id: "l-calc".into(),
                source_block_id: source.clone(),
                dependent_block_id: budget,
                behavior: LinkBehavior::Calculate,
                transform_id: Some("scale:2".into()),
                provenance: LinkProvenance::HumanConfirmed,
                source_revision_at_confirmation: 0,
                active: true,
            },
            Link {
                id: "l-review".into(),
                source_block_id: source,
                dependent_block_id: review,
                behavior: LinkBehavior::Review,
                transform_id: None,
                provenance: LinkProvenance::HumanConfirmed,
                source_revision_at_confirmation: 0,
                active: true,
            },
        ];
        doc.graph_revision = 1;
        doc
    }

    fn source_id(doc: &Document) -> BlockId {
        doc.blocks[1].id.clone()
    }

    #[test]
    fn preview_shows_the_exact_dependent_values() {
        let doc = fixture();
        let (_, patch) =
            preview_edit(&doc, &source_id(&doc), "We ordered 3 units of bolts.").expect("edit");
        let replacement = patch.replacements.first().expect("a calculated dependent");
        assert_eq!(replacement.expected_old_bytes, "Budget is 36 euros.");
        assert_eq!(replacement.new_bytes, "Budget is 6 euros.");
        assert!(patch.review_only.iter().any(|n| n.reason.contains("review link")));
    }

    #[test]
    fn preview_is_none_when_nothing_changes() {
        let doc = fixture();
        let unchanged = doc.block(&source_id(&doc)).unwrap().text(&doc.raw).to_string();
        assert!(preview_edit(&doc, &source_id(&doc), &unchanged).is_none());
    }

    #[test]
    fn adapted_document_is_what_accept_would_write() {
        let doc = fixture();
        let (candidate, patch) =
            preview_edit(&doc, &source_id(&doc), "We ordered 3 units of bolts.").expect("edit");
        let shown = adapted(&candidate, &patch);
        let accepted = apply_patch(&candidate, &patch, "you", 0, 0).expect("accept").0;
        assert_eq!(shown.raw, accepted.raw);
        assert!(shown.raw.contains("Budget is 6 euros."));
    }

    #[test]
    fn copy_link_replaces_the_dependent_verbatim() {
        let mut doc = fixture();
        let source = source_id(&doc);
        let dependent = doc.blocks[2].id.clone();
        doc.links.push(Link {
            id: "l-copy".into(),
            source_block_id: source.clone(),
            dependent_block_id: dependent,
            behavior: LinkBehavior::Copy,
            transform_id: Some("copy".into()),
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        });
        let (_, patch) = preview_edit(&doc, &source, "We ordered 3 units of bolts.").expect("edit");
        assert!(
            patch
                .replacements
                .iter()
                .any(|r| r.new_bytes == "We ordered 3 units of bolts."),
            "copy yields the source bytes, not a paraphrase"
        );
    }

    #[test]
    fn link_ids_resume_after_a_restart() {
        let doc = fixture();
        assert_eq!(next_link_seq(&doc), 1, "seed links are l-calc / l-review");
        let mut with_link = doc.clone();
        with_link.links.push(Link {
            id: "l1".into(),
            source_block_id: doc.blocks[1].id.clone(),
            dependent_block_id: doc.blocks[2].id.clone(),
            behavior: LinkBehavior::Review,
            transform_id: None,
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        });
        assert_eq!(next_link_seq(&with_link), 2);
    }

    #[test]
    fn doc_keys_are_readable_and_distinct_per_path() {
        // Two files with the same basename must not share a history.
        let a = doc_key(Path::new("/tmp/one/report.md"));
        let b = doc_key(Path::new("/tmp/two/report.md"));
        assert!(a.starts_with("report-"), "{a}");
        assert_ne!(a, b);
        assert_eq!(a, doc_key(Path::new("/tmp/one/report.md")), "stable across runs");
        // A basename full of path-hostile characters still yields a usable name.
        let odd = doc_key(Path::new("/tmp/a b:c*/weird name.md"));
        assert!(odd.starts_with("weird_name-"), "{odd}");
    }

    #[test]
    fn reconcile_parses_a_new_file_at_revision_zero() {
        let (doc, t, log) = reconcile(None, Path::new("/tmp/new.md"), SAMPLE);
        assert_eq!(doc.revision, 0);
        assert_eq!(doc.raw, SAMPLE);
        assert!(t.is_none(), "nothing to re-identify on a first open");
        assert!(log[0].contains("revision 0"));
    }

    #[test]
    fn reconcile_reuses_stored_state_when_the_file_is_untouched() {
        let stored = fixture();
        let (doc, t, _) = reconcile(Some(stored.clone()), Path::new("/tmp/f.md"), SAMPLE);
        assert_eq!(doc.revision, stored.revision);
        assert!(t.is_none(), "no transition when the bytes did not move");
        assert_eq!(doc.links.len(), 2, "links survive an unchanged reopen");
    }

    #[test]
    fn an_external_edit_to_a_linked_block_keeps_the_link_attached() {
        // The whole reason identity is carried across a diff: someone edits the
        // budget outside the tool, and the link that names that block still
        // names it, so the next source edit still propagates into it.
        let stored = fixture();
        let budget = stored.blocks[2].id.clone();
        let rewritten =
            "# Field report\n\nWe ordered 18 units of bolts.\n\nBudget is 9 euros.\n\nCheck supplier terms.\n";
        let (doc, t, log) = reconcile(Some(stored.clone()), Path::new("/tmp/f.md"), rewritten);
        let t = t.expect("an external edit is a transition");
        assert_eq!(doc.revision, stored.revision + 1);
        assert_eq!(doc.links.len(), 2, "no human declaration is destroyed");
        assert_eq!(doc.graph_revision, stored.graph_revision, "the graph did not change");
        let calc = doc.links.iter().find(|l| l.id == "l-calc").unwrap();
        assert!(link_resolves(&doc, calc), "the edited block kept its identity");
        assert_eq!(calc.dependent_block_id, budget);
        assert_eq!(doc.block(&budget).unwrap().text(&doc.raw), "Budget is 9 euros.");
        assert_eq!(t.carried, 4);
        assert!(log.iter().any(|l| l.contains("external edit")));

        // And the link still does its job on the next edit.
        let source = doc.blocks[1].id.clone();
        let (_, patch) = preview_edit(&doc, &source, "We ordered 3 units of bolts.").expect("edit");
        assert_eq!(
            patch.replacements.first().map(|r| r.new_bytes.as_str()),
            Some("Budget is 6 euros."),
            "the carried block is still the dependent"
        );
    }

    #[test]
    fn a_deleted_block_leaves_its_link_dangling_and_inert() {
        // The block really is gone, so the link must not invent a target: it is
        // reported as dangling, and it contributes nothing to a proposal.
        let stored = fixture();
        let rewritten = "# Field report\n\nWe ordered 18 units of bolts.\n\nCheck supplier terms.\n";
        let (doc, t, log) = reconcile(Some(stored), Path::new("/tmp/f.md"), rewritten);
        let t = t.expect("a deletion is a transition");
        assert!(t.ambiguous.is_empty(), "a deletion is a decision");
        assert_eq!(t.orphaned.len(), 1);
        let calc = doc.links.iter().find(|l| l.id == "l-calc").unwrap();
        assert!(!link_resolves(&doc, calc));
        assert!(log.iter().any(|l| l.contains("no longer resolve")));
        let source = doc.blocks[1].id.clone();
        let (_, patch) = preview_edit(&doc, &source, "We ordered 3 units of bolts.").expect("edit");
        assert!(patch.replacements.is_empty(), "a dangling link replaces nothing");
    }

    #[test]
    fn factor_prompt_accepts_only_registered_transforms() {
        // The TUI validates the typed factor against the module's own registry
        // instead of creating a link that can never resolve.
        assert!(apply_transform("scale:3", "1", "1").is_some());
        assert!(apply_transform("scale:-1", "1", "1").is_some());
        assert!("not a number".parse::<i64>().is_err());
        assert!(apply_transform("scale:2.5", "1", "1").is_none());
    }

    /// Render one frame and read the resulting screen, which is what a person
    /// sees. A pty capture cannot be asserted on: ratatui emits only the cells
    /// that changed, so an edited word arrives without its line.
    fn screen_of(app: &App) -> String {
        let backend = ratatui::backend::TestBackend::new(120, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        term.backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    fn app_on(dir: &Path, name: &str, text: &str) -> (App, PathBuf) {
        let file = dir.join(name);
        std::fs::write(&file, text).unwrap();
        let mut app = App::new(dir.to_path_buf());
        app.store_home = dir.join("store");
        app.open_path(file.clone());
        (app, file)
    }

    #[test]
    fn the_document_pane_shows_an_externally_edited_block() {
        // The live path, without waiting on the 400 ms poll: the file changes
        // underneath and the pane shows the new bytes.
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        assert!(screen_of(&app).contains("Budget is 36 euros."));
        std::fs::write(&file, SAMPLE.replace("36 euros", "12 euros")).unwrap();
        app.force_reload();
        let screen = screen_of(&app);
        assert!(screen.contains("Budget is 12 euros."), "the pane follows the file");
        assert!(!screen.contains("Budget is 36 euros."), "the old text is gone");
    }

    #[test]
    fn the_poll_adopts_an_external_edit_and_keeps_the_link() {
        // The same thing through the watcher's own entry point.
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        {
            let s = app.session.as_mut().unwrap();
            s.doc.links.push(Link {
                id: "l1".into(),
                source_block_id: s.doc.blocks[1].id.clone(),
                dependent_block_id: s.doc.blocks[2].id.clone(),
                behavior: LinkBehavior::Calculate,
                transform_id: Some("scale:2".into()),
                provenance: LinkProvenance::HumanConfirmed,
                source_revision_at_confirmation: 0,
                active: true,
            });
            s.doc.graph_revision += 1;
            let mut status = String::new();
            s.save_document(&mut status);
            s.watched = None; // pretend the poll has not seen the file yet
        }
        let budget = app.session.as_ref().unwrap().doc.blocks[2].id.clone();
        std::fs::write(&file, SAMPLE.replace("36 euros", "12 euros")).unwrap();
        app.last_poll = Instant::now() - Duration::from_secs(5);
        app.poll_file();

        let s = app.session.as_ref().unwrap();
        assert_eq!(s.doc.revision, 1, "the external edit moved the revision");
        assert_eq!(s.transitions.len(), 1, "the transition was recorded");
        assert_eq!(s.transitions[0].carried, s.doc.blocks.len());
        assert_eq!(s.doc.blocks[2].id, budget, "the edited block kept its id");
        assert!(s.doc.block(&budget).is_some(), "the link still resolves");
        assert!(screen_of(&app).contains("Budget is 12 euros."));
    }

    #[test]
    fn a_dangling_link_is_re_anchored_from_the_recorded_displacement() {
        // The block that carried the link was replaced by unrelated text, so
        // identity was refused. The succession is still recorded, and `r`
        // repairs the link with it.
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        {
            let s = app.session.as_mut().unwrap();
            s.doc.links.push(Link {
                id: "l1".into(),
                source_block_id: s.doc.blocks[1].id.clone(),
                dependent_block_id: s.doc.blocks[2].id.clone(),
                behavior: LinkBehavior::Calculate,
                transform_id: Some("scale:2".into()),
                provenance: LinkProvenance::HumanConfirmed,
                source_revision_at_confirmation: 0,
                active: true,
            });
            s.doc.graph_revision += 1;
            let mut status = String::new();
            s.save_document(&mut status);
        }
        let old_dependent = app.session.as_ref().unwrap().doc.links[0].dependent_block_id.clone();

        // "Approved unanimously." shares no word with "Budget is 36 euros."
        std::fs::write(
            &file,
            "# Field report\n\nWe ordered 18 units of bolts.\n\nApproved unanimously.\n\nCheck supplier terms.\n",
        )
        .unwrap();
        app.force_reload();

        {
            let s = app.session.as_ref().unwrap();
            assert!(!link_resolves(&s.doc, &s.doc.links[0]), "the block is gone");
            assert_eq!(s.transitions.len(), 1);
            let c = s.transitions[0].candidates_for(&old_dependent);
            assert_eq!(c.len(), 1, "the replacement was recorded");
            assert_eq!(c[0].shared_tokens, 0);
        }
        app.overlay = Overlay::Links;
        assert!(
            screen_of(&app).contains("re-anchor"),
            "the links view offers the repair"
        );
        app.overlay = Overlay::None;

        app.link_sel = 0;
        app.reanchor();
        let s = app.session.as_ref().unwrap();
        assert!(link_resolves(&s.doc, &s.doc.links[0]), "the link resolves again");
        assert_ne!(s.doc.links[0].dependent_block_id, old_dependent);
        assert_eq!(s.doc.graph_revision, 2, "the graph changed");
        assert_eq!(
            s.doc.block(&s.doc.links[0].dependent_block_id).unwrap().text(&s.doc.raw),
            "Approved unanimously."
        );
    }

    #[test]
    fn the_history_overlay_lists_events_patches_and_transitions() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        std::fs::write(&file, SAMPLE.replace("36 euros", "12 euros")).unwrap();
        app.force_reload();
        app.overlay = Overlay::History;
        let screen = screen_of(&app);
        assert!(screen.contains("events.jsonl"), "the event log is listed");
        assert!(screen.contains("patches.json"), "the patch book is listed");
        assert!(screen.contains("transitions.jsonl"), "the identity transitions are listed");
        assert!(screen.contains("carried"), "and what each transition carried");
        assert!(
            screen.contains("warrants a model") || screen.contains("minted"),
            "and whether the passage was complex enough to need one"
        );
    }

    #[test]
    fn the_causal_ledger_walks_a_chain_of_replacements() {
        // Two replacements in a row: the newest block's lineage must reach back
        // through both, which is the whole point of recording the succession
        // rather than only listing it.
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        std::fs::write(
            &file,
            "# Field report\n\nWe ordered 18 units of bolts.\n\nApproved unanimously.\n\nCheck supplier terms.\n",
        )
        .unwrap();
        app.force_reload();
        let first = app.session.as_ref().unwrap().doc.blocks[2].id.clone();
        std::fs::write(
            &file,
            "# Field report\n\nWe ordered 18 units of bolts.\n\nNoted.\n\nCheck supplier terms.\n",
        )
        .unwrap();
        app.force_reload();
        let second = app.session.as_ref().unwrap().doc.blocks[2].id.clone();

        let s = app.session.as_ref().unwrap();
        assert_eq!(s.transitions.len(), 2);
        assert_eq!(s.causes.len(), 2, "one Corrected event per succession");
        let chain: Vec<&str> = s
            .causes
            .lineage(&second)
            .iter()
            .map(|e| e.object.as_str())
            .collect();
        assert!(chain.contains(&second.as_str()), "lineage includes the block");
        assert!(chain.contains(&first.as_str()), "and what it replaced");
        let down: Vec<&str> = s
            .causes
            .descendants(&first)
            .iter()
            .map(|e| e.object.as_str())
            .collect();
        assert!(down.contains(&second.as_str()), "and walks forward again");
    }

    #[test]
    fn a_simple_edit_does_not_reach_for_a_model() {
        // The gate. A value edited in place is already accounted for, so no
        // worker is started and no model is loaded.
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        std::fs::write(&file, SAMPLE.replace("36 euros", "12 euros")).unwrap();
        app.force_reload();
        assert!(!app.model.busy(), "no model for an edit the diff explained");
        assert_eq!(app.pending_annotations().len(), 4, "nothing read yet");
        let s = app.session.as_ref().unwrap();
        assert_eq!(s.transitions.len(), 1);
        assert!(!s.transitions[0].warrants_model());
    }

    #[test]
    fn only_unread_blocks_are_sent_to_the_model() {
        // A second pass must not re-read what has already been read at the
        // same bytes.
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(tmp.path(), "report.md", SAMPLE);
        let all = app.pending_annotations();
        assert_eq!(all.len(), 4);
        let block = {
            let s = app.session.as_ref().unwrap();
            (s.doc.blocks[0].id.clone(), s.doc.blocks[0].content_hash)
        };
        {
            let s = app.session.as_mut().unwrap();
            s.observations.push(Observation {
                block_id: block.0.clone(),
                revision: s.doc.revision,
                content_hash: block.1,
                embedder: "test".into(),
                cell: None,
                score: 0.0,
                entries: Vec::new(),
                neighbours: Vec::new(),
                isolated: false,
                vector: Vec::new(),
            });
        }
        let left = app.pending_annotations();
        assert_eq!(left.len(), 3);
        assert!(!left.iter().any(|p| p.block_id == block.0));
        // An observation about other bytes does not count as read.
        {
            let s = app.session.as_mut().unwrap();
            s.observations[0].content_hash = block.1 ^ 0xdead_beef;
        }
        assert_eq!(app.pending_annotations().len(), 4);
    }

    #[test]
    fn the_graph_lists_live_and_gone_blocks_with_their_edges() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, file) = app_on(tmp.path(), "report.md", SAMPLE);
        {
            let s = app.session.as_mut().unwrap();
            s.doc.links.push(Link {
                id: "l1".into(),
                source_block_id: s.doc.blocks[1].id.clone(),
                dependent_block_id: s.doc.blocks[2].id.clone(),
                behavior: LinkBehavior::Calculate,
                transform_id: Some("scale:2".into()),
                provenance: LinkProvenance::HumanConfirmed,
                source_revision_at_confirmation: 0,
                active: true,
            });
            let mut status = String::new();
            s.save_document(&mut status);
        }
        let gone = app.session.as_ref().unwrap().doc.blocks[2].id.clone();
        std::fs::write(
            &file,
            "# Field report\n\nWe ordered 18 units of bolts.\n\nApproved unanimously.\n\nCheck supplier terms.\n",
        )
        .unwrap();
        app.force_reload();

        let nodes = app.graph_nodes();
        assert_eq!(nodes.iter().filter(|n| n.live).count(), 4);
        assert_eq!(nodes.iter().filter(|n| !n.live).count(), 1, "the gone block is a node");
        assert!(nodes.iter().any(|n| n.id == gone && !n.live));
        let edges = app.edges_of(&gone);
        assert!(edges.iter().any(|e| e.contains("replaced by")), "{edges:?}");
        let source = app.session.as_ref().unwrap().doc.blocks[1].id.clone();
        assert!(
            app.edges_of(&source).iter().any(|e| e.contains("calculate")),
            "the outgoing link is an edge of its source too"
        );

        app.tab = Tab::Graph;
        let screen = screen_of(&app);
        assert!(screen.contains("graph —"));
        assert!(screen.contains("gone"));
    }

    #[test]
    fn the_files_pane_creates_opens_and_deletes() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(tmp.path(), "report.md", SAMPLE);
        app.tab = Tab::Files;
        app.picker = Picker::load(tmp.path().to_path_buf());
        assert!(screen_of(&app).contains("report.md"));

        // n, type a name, Enter: created and opened.
        app.on_key(KeyCode::Char('n'), KeyModifiers::NONE);
        for c in "notes.md".chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(tmp.path().join("notes.md").exists(), "the file was created");
        assert_eq!(app.tab, Tab::Document, "and opened");

        // d twice deletes; once only arms.
        app.tab = Tab::Files;
        app.picker = Picker::load(tmp.path().to_path_buf());
        app.picker.sel = app
            .picker
            .entries
            .iter()
            .position(|e| e.name == "notes.md")
            .expect("notes.md is listed");
        app.on_key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(tmp.path().join("notes.md").exists(), "one press only arms");
        app.on_key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(!tmp.path().join("notes.md").exists(), "the second press removes");
    }

    #[test]
    fn every_tab_is_reachable_by_key_and_has_a_pane() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(tmp.path(), "report.md", SAMPLE);
        for (key, tab) in [
            ('1', Tab::Document),
            ('2', Tab::Graph),
            ('3', Tab::Files),
            ('4', Tab::Terminal),
        ] {
            app.on_key(KeyCode::Char(key), KeyModifiers::NONE);
            assert!(app.tab == tab, "key {key} selects its tab");
            let screen = screen_of(&app);
            assert!(screen.contains(tab.title()), "{:?} renders a pane", tab);
        }
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert!(app.tab == Tab::Document, "Tab cycles");
    }

    /// Put a vector on a block so it counts as read by a model.
    fn fake_observation(app: &mut App, block: usize, vector: Vec<f32>) {
        let s = app.session.as_mut().unwrap();
        let (id, hash, revision) = {
            let b = &s.doc.blocks[block];
            (b.id.clone(), b.content_hash, s.doc.revision)
        };
        s.observations.push(Observation {
            block_id: id,
            revision,
            content_hash: hash,
            embedder: "test".into(),
            cell: Some(("MEASURE".into(), "OPERATE".into())),
            score: 0.9,
            entries: Vec::new(),
            neighbours: Vec::new(),
            isolated: false,
            vector,
        });
        s.store.save_observations(&s.observations).unwrap();
    }

    #[test]
    fn the_prompt_runs_a_shell_line_and_asks_physis_otherwise() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(tmp.path(), "report.md", SAMPLE);
        app.tab = Tab::Document;

        // `t ...` is a shell line, and it goes to the terminal pane.
        app.on_key(KeyCode::Char(':'), KeyModifiers::NONE);
        for c in "t echo prompt-ok".chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.tab == Tab::Terminal, "a shell line lands in the terminal");
        assert!(app.term.busy());

        // Anything else is a question, and with nothing embedded there is
        // nothing to rank — it says so rather than loading a model for nothing.
        app.tab = Tab::Document;
        app.on_key(KeyCode::Char(':'), KeyModifiers::NONE);
        for c in "what did we order".chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!app.model.busy(), "no model for an empty corpus");
        assert!(app.status.contains("corpus"), "{}", app.status);
    }

    #[test]
    fn the_corpus_relates_blocks_across_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(tmp.path(), "one.md", SAMPLE);
        fake_observation(&mut app, 2, vec![1.0, 0.0, 0.0]);
        let other = tmp.path().join("two.md");
        std::fs::write(&other, SAMPLE).unwrap();
        app.open_path(other.clone());
        fake_observation(&mut app, 2, vec![0.9, 0.1, 0.0]);

        app.rescan_corpus();
        assert_eq!(app.corpus.documents, 2, "{}", app.corpus);
        assert_eq!(app.corpus.len(), 2);
        let mine = app.document_id();
        let related = app.corpus.related(&[1.0, 0.0, 0.0], &mine, 3);
        assert_eq!(related.len(), 1, "the other document is the relation");
        assert!(app.corpus.entries[related[0].0].document.ends_with("one.md"));
        // In-document neighbours and corpus neighbours are different questions.
        assert!(app.corpus.related(&[1.0, 0.0, 0.0], &app.corpus.entries[related[0].0].document, 3)
            .iter()
            .any(|(i, _)| app.corpus.entries[*i].document == mine));
    }

    #[test]
    fn a_hyperlink_in_a_block_is_listed_and_followed() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("other.md"), "# Other\n\nBody.\n").unwrap();
        let (mut app, _file) = app_on(
            tmp.path(),
            "report.md",
            "# Report\n\nSee [the other](other.md) for the numbers.\n",
        );
        app.sel = 1;
        let links = app.hyperlinks_of_selected();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].0, "the other");
        assert!(matches!(links[0].2, LinkTarget::Document));

        app.follow_hyperlink();
        let s = app.session.as_ref().unwrap();
        assert!(
            s.path.ends_with("other.md"),
            "following a document link opens it: {}",
            s.path.display()
        );
    }

    #[test]
    fn a_url_hyperlink_is_reported_and_not_opened() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(
            tmp.path(),
            "report.md",
            "# Report\n\nSee [the docs](https://example.com/x).\n",
        );
        app.sel = 1;
        app.follow_hyperlink();
        assert!(app.status.contains("URL"), "{}", app.status);
        assert!(
            app.session.as_ref().unwrap().path.ends_with("report.md"),
            "a URL must not navigate anywhere"
        );
    }

    #[test]
    fn the_files_pane_hands_a_file_to_editor_or_to_the_graph() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut app, _file) = app_on(tmp.path(), "report.md", SAMPLE);
        app.tab = Tab::Files;
        app.picker = Picker::load(tmp.path().to_path_buf());
        app.picker.sel = app
            .picker
            .entries
            .iter()
            .position(|e| e.name == "report.md")
            .unwrap();

        app.on_key(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(
            app.pending_editor.as_deref(),
            Some(tmp.path().join("report.md").as_path()),
            "e hands the file to $EDITOR"
        );
        app.pending_editor = None;

        app.on_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert!(app.tab == Tab::Graph, "g opens the graph of that file");
        assert!(app.session.as_ref().unwrap().path.ends_with("report.md"));
    }

    #[test]
    fn picker_shows_dirs_and_documents_only() {
        assert!(is_doc_file(Path::new("a.md")));
        assert!(is_doc_file(Path::new("a.MARKDOWN")));
        assert!(is_doc_file(Path::new("a.txt")));
        assert!(!is_doc_file(Path::new("a.rs")));
        assert!(!is_doc_file(Path::new("a.pdf")));
        assert!(!is_doc_file(Path::new("Makefile")));
    }
}
