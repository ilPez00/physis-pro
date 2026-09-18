// provenance: moved from physis-core src/bin/world.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
// ponytail: dead under the current bin set until a `physis-world` bin wires
// `main()` in; delete if no bin ever lands.
#![allow(dead_code)]
//! `physis-world` — the navigational substrate, over the machine's own log.
//!
//! The filesystem stays exactly where it is. This is a second layer on top of
//! it: the same files, addressed by *where they sit in a world* rather than by
//! path. Nothing here writes to your files, and `observe` only ever appends.
//!
//! ```text
//!   FILES → observations (append-only)
//!         → representations (one embedder, named in every answer)
//!         → structure (entity links + mutual-kNN)
//!         → world states (ordered by POSITION IN THE LOG, not by clock)
//!         → navigation (locate / neighbors / history / changed / retrieve)
//! ```
//!
//! Every answer prints the chain it came from — observation seq → representation
//! → operator → derived edge — because a navigational layer that cannot say why
//! it said something is an oracle, and this project does not ship oracles.
//!
//! What the measurements say about this shell, stated where a user sees it:
//!
//! * the retrieval arm (entity link + earlier position + similarity) beats
//!   order-blind similarity on corpora whose links are good (E57 0.700 vs 0.013;
//!   E59 0.357 vs 0.071), and **loses** to it where the extracted links are bad
//!   (E60 0.209 vs 0.349). So `retrieve` runs the linked arm and prints the
//!   blind arm beside it rather than choosing for you.
//! * transition labels come from set operations over the log order, measured at
//!   0.486 against a shuffled-order null of 0.291 (E56). They are derivations,
//!   not facts.
use std::collections::HashMap;

use crate::ported::observe_adapters::{watch_fs, watch_git, watch_shell};
use physis_core::embed::VectorEmbed;
use physis_core::observe::{self, Observation};
use physis_core::worldstate as ws;

struct World {
    obs: Vec<Observation>,
    texts: Vec<String>,
    ents: Vec<Vec<String>>,
    vecs: Vec<Vec<f32>>,
    sim: Vec<Vec<f32>>,
    adj: Vec<Vec<usize>>,
    embedder: String,
    /// `Some(first)` when this position repeats an earlier (source, subject).
    dup_of: Vec<Option<usize>>,
    bm25: physis_core::rag::Bm25Index,
    terms: Vec<Vec<String>>,
    /// Mean BM25 top-1/top-2 margin: how decisively lexical evidence separates
    /// this world's records. Low means near-twins, which is when only an
    /// identity link can partition before ranking (E63, E65).
    margin: f64,
}

impl World {
    fn load(k: usize) -> anyhow::Result<Self> {
        let obs = observe::read(&observe::log_path())?;
        if obs.is_empty() {
            anyhow::bail!(
                "no observations yet — run `physis-world observe --root <dir>` first ({})",
                observe::log_path().display()
            );
        }
        let texts: Vec<String> = obs
            .iter()
            .map(|o| {
                if o.body.is_empty() {
                    o.subject.clone()
                } else {
                    format!("{} {}", o.subject, o.body)
                }
            })
            .collect();
        // Duplicates already in the log stay in the log: it is append-only, and
        // deleting history to tidy a display is exactly the move this
        // architecture exists to refuse. They are collapsed HERE, at read time,
        // for ranking only — the first occurrence of a (source, subject) pair
        // keeps its position and later copies are marked.
        let mut first_seen: HashMap<(String, String), usize> = HashMap::new();
        let mut dup_of: Vec<Option<usize>> = vec![None; obs.len()];
        for (i, o) in obs.iter().enumerate() {
            let key = (o.source.clone(), o.subject.clone());
            match first_seen.get(&key) {
                Some(&f) => dup_of[i] = Some(f),
                None => {
                    first_seen.insert(key, i);
                }
            }
        }
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        // Links: three declared rules, unioned. E60 measured each one; the code
        // rule is the only one that scores on identifier-heavy text and emits
        // nothing on plain prose, so the union costs nothing where it does not
        // apply.
        let code = ws::extract_code_identifiers(&refs);
        let np = ws::extract_noun_phrases(&refs, 3);
        let ids = ws::extract_entities_rules(&refs, 3, true, true, false);
        let mut ents = ws::merge_entities(&ws::merge_entities(&code, &np), &ids);

        // Interpreter links, off by default, read from a cache file rather than
        // called per query. Measured twice, on two text regimes:
        //
        //   corpus            rules  blind  interp+rules  partial  gold
        //   docs (E61, n=43)  0.209  0.349     0.488       0.326   0.698
        //   log  (E62, n=159) 0.296  0.340     0.465       0.377   0.528
        //
        // "partial" = the model asked only where the rules came up empty. It is
        // worse than full coverage on both regimes, and on docs it is worse than
        // using no links at all. There is no cheap half-measure.
        let mut interp_used = 0usize;
        if let Ok(path) = std::env::var("PHYSIS_WORLD_INTERP") {
            match std::fs::read_to_string(&path) {
                Ok(body) => {
                    let rows: Vec<serde_json::Value> =
                        serde_json::from_str(&body).unwrap_or_default();
                    let by_seq: HashMap<u64, Vec<String>> = rows
                        .iter()
                        .filter_map(|r| {
                            let seq = r["seq"].as_u64()?;
                            let links: Vec<String> = r["links"]
                                .as_array()?
                                .iter()
                                .filter_map(|v| v.as_str())
                                .map(ws::entity_key)
                                .filter(|k| k.len() > 2)
                                .collect();
                            Some((seq, links))
                        })
                        .collect();
                    for (i, o) in obs.iter().enumerate() {
                        if let Some(extra) = by_seq.get(&o.seq) {
                            for k in extra {
                                if !ents[i].contains(k) {
                                    ents[i].push(k.clone());
                                }
                            }
                            interp_used += 1;
                        }
                    }
                    eprintln!(
                        "interpreter links merged for {interp_used}/{} observations from {path}",
                        obs.len()
                    );
                    if interp_used * 2 < obs.len() {
                        eprintln!(
                            "WARNING: the cache covers under half the log. Partial interpreter \
                             coverage measured worse than full on both corpora (0.377 vs 0.465 on \
                             the log; 0.326 vs 0.488 on docs) and, on docs, worse than no links \
                             at all (0.349)."
                        );
                    }
                }
                Err(e) => eprintln!("interpreter cache {path} unreadable ({e}) — rules only"),
            }
        }

        // Representation. Default is the counted word table, because E64
        // measured it as indistinguishable from the model on the arms this shell
        // actually uses (log: gold-link 0.497 vs 0.528, top3 0.742 vs 0.730;
        // docs: 0.674 vs 0.698, top3 0.953 vs 0.977) while building in 6 ms
        // against 12 s for 388 sentences — and this log has 3232. The model is
        // better at unfiltered similarity (blind 0.349 vs 0.279 on docs), so
        // PHYSIS_WORLD_REP=model restores it.
        let rep = std::env::var("PHYSIS_WORLD_REP").unwrap_or_else(|_| "table-count".into());
        let (vecs, kind) = if rep == "model" {
            let (embedder, kind) = physis_core::embed::select(768);
            if kind == "random-projection" {
                eprintln!("note: no model on disk — lexical hash. Set PHYSIS_MODEL_DIR.");
            }
            (
                refs.iter().map(|t| embedder.embed(t)).collect::<Vec<_>>(),
                kind.to_string(),
            )
        } else {
            let t0 = std::time::Instant::now();
            let table = ws::word_table_count(&refs, 512, 4);
            let v: Vec<Vec<f32>> = refs
                .iter()
                .map(|t| ws::sentence_from_table(&table, t, 512))
                .collect();
            eprintln!(
                "representation: counted word table ({} words, {:?}, no model)",
                table.len(),
                t0.elapsed()
            );
            (v, "table-count".to_string())
        };
        let sim = ws::sim_matrix(&vecs);
        let adj = ws::mutual_knn(&sim, k);
        let texts_owned: Vec<String> = texts.clone();
        // Which filter this corpus wants, measured rather than flagged (E65).
        let margin = ws::lexical_margin(&texts_owned, 150);
        let terms: Vec<Vec<String>> = texts
            .iter()
            .map(|t| physis_core::rag::bm25_terms(t))
            .collect();
        Ok(Self {
            obs,
            texts,
            ents,
            vecs,
            sim,
            adj,
            embedder: kind,
            dup_of,
            bm25: physis_core::rag::Bm25Index::build(&texts_owned),
            terms,
            margin,
        })
    }

    fn n(&self) -> usize {
        self.obs.len()
    }

    /// Positions whose extracted entity set contains the key.
    fn positions_of(&self, term: &str) -> Vec<usize> {
        let key = ws::entity_key(term);
        (0..self.n())
            .filter(|&i| {
                self.dup_of[i].is_none()
                    && self.ents[i]
                        .iter()
                        .any(|e| *e == key || e.split('-').any(|p| p == key))
            })
            .collect()
    }

    /// Positions matching the raw string. Used only as a declared fallback:
    /// the link layer's recall is the measured bottleneck (E60), so "no link"
    /// must never be printed as "not here" — that would reintroduce Gap 3, a
    /// retriever with no way to say what it does not contain.
    fn string_match(&self, term: &str) -> Vec<usize> {
        let needle = term.to_lowercase();
        (0..self.n())
            .filter(|&i| self.dup_of[i].is_none() && self.texts[i].to_lowercase().contains(&needle))
            .collect()
    }

    /// Links first, string match second, and it says which one answered.
    fn resolve(&self, term: &str) -> (Vec<usize>, &'static str) {
        let linked = self.positions_of(term);
        if !linked.is_empty() {
            return (linked, "entity-link");
        }
        (self.string_match(term), "string-match (FALLBACK — the structural layer missed this)")
    }

    fn line(&self, i: usize) -> String {
        let o = &self.obs[i];
        let t = self.texts[i].replace('\n', " ");
        let t: String = t.chars().take(90).collect();
        format!("  [{i:>5}] {:<8} {}", o.source, t)
    }

    /// The provenance chain for one derived answer.
    fn chain(&self, i: usize, operator: &str) -> String {
        let o = &self.obs[i];
        format!(
            "    trace: observation seq {} ({}, {}) → representation {} → operator {} → position {}",
            o.seq,
            o.source,
            o.at.format("%Y-%m-%d %H:%M"),
            self.embedder,
            operator,
            i
        )
    }

    fn transitions(&self) -> Vec<&'static str> {
        let refs: Vec<&str> = self.texts.iter().map(|s| s.as_str()).collect();
        let facts: Vec<Option<(String, String, String)>> = vec![None; self.n()];
        ws::derive_transitions(&self.ents, &facts, &refs)
    }

    /// The measured retrieval arm: entity link + earlier position + similarity.
    fn linked(&self, p: usize, top: usize) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..p)
            .filter(|&q| {
                self.dup_of[q].is_none()
                    && self.ents[q].iter().any(|e| self.ents[p].contains(e))
            })
            .collect();
        idx.sort_by(|&a, &b| self.sim[p][b].partial_cmp(&self.sim[p][a]).unwrap());
        idx.truncate(top);
        idx
    }

    /// The lexical arm: BM25 over earlier positions. E63 measured it at or
    /// above every extraction arm on real text (docs 0.535 vs 0.488 for
    /// rules+interpreter; log 0.447 vs 0.465, indistinguishable) at no cost —
    /// and at 0.013 against the entity arm's 0.700 on a corpus of near-identical
    /// templates, where only an identity link can partition before ranking.
    fn lexical(&self, p: usize, top: usize) -> Vec<usize> {
        let mut scored: Vec<(usize, f32)> = (0..p)
            .filter(|&q| self.dup_of[q].is_none())
            .map(|q| (q, self.bm25.score(q, &self.terms[p])))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(top);
        scored.into_iter().map(|(q, _)| q).collect()
    }

    fn blind(&self, p: usize, top: usize) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.n())
            .filter(|&q| q != p && self.dup_of[q].is_none())
            .collect();
        idx.sort_by(|&a, &b| self.sim[p][b].partial_cmp(&self.sim[p][a]).unwrap());
        idx.truncate(top);
        idx
    }

    /// Query entry point. **BM25 by default, not the vector space.**
    ///
    /// Found by running it the other way: with the counted table as the query
    /// representation, "which experiment measured the null for the grid"
    /// returned `strings.xml` and two i18n YAML files. A free-text query is
    /// short and its words are common, which is the exact case E64 measured the
    /// table as weakest on (unfiltered similarity 0.279 vs the model's 0.349) —
    /// and E63 measured BM25 as the best arm on real text anyway. The model
    /// remains available for this path via PHYSIS_WORLD_QUERY=model, which is
    /// the only place in the shell where running it is defensible.
    fn nearest_to_query(&self, q: &str, top: usize) -> Vec<usize> {
        if std::env::var("PHYSIS_WORLD_QUERY").as_deref() == Ok("model") {
            let (e, _) = physis_core::embed::select(768);
            let qv = e.embed(q);
            let mut idx: Vec<usize> = (0..self.n()).filter(|&i| self.dup_of[i].is_none()).collect();
            idx.sort_by(|&a, &b| {
                ws::cosine(&qv, &self.vecs[b])
                    .partial_cmp(&ws::cosine(&qv, &self.vecs[a]))
                    .unwrap()
            });
            idx.truncate(top);
            return idx;
        }
        let qt = physis_core::rag::bm25_terms(q);
        let mut scored: Vec<(usize, f32)> = (0..self.n())
            .filter(|&i| self.dup_of[i].is_none())
            .map(|i| (i, self.bm25.score(i, &qt)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(top);
        scored.into_iter().map(|(i, _)| i).collect()
    }
}

/// Where a copied payload lives when no system clipboard exists.
///
/// Everything in this shell until now only read and appended. Copy/paste is the
/// first verb that puts something *out*, so it is deliberately small: it moves
/// text plus its provenance, it never touches a source file, and pasting into a
/// file requires naming that file.
fn clipboard_path() -> std::path::PathBuf {
    physis_core::store::data_dir().join("clipboard.json")
}

/// System clipboard if one exists, file otherwise. Reports which was used
/// instead of silently doing something different from what the user expects —
/// a headless or ssh session has no clipboard, and pretending otherwise is how
/// a copy silently goes nowhere.
fn clipboard_write(payload: &str) -> String {
    for (bin, args) in [
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
        ("pbcopy", vec![]),
    ] {
        if which(bin) {
            use std::io::Write;
            if let Ok(mut child) = std::process::Command::new(bin)
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(mut si) = child.stdin.take() {
                    let _ = si.write_all(payload.as_bytes());
                }
                let _ = child.wait();
                return bin.to_string();
            }
        }
    }
    let _ = std::fs::create_dir_all(clipboard_path().parent().unwrap());
    let _ = std::fs::write(clipboard_path(), payload);
    format!("file ({})", clipboard_path().display())
}

fn clipboard_read() -> Option<(String, String)> {
    for (bin, args) in [
        ("wl-paste", vec!["--no-newline"]),
        ("xclip", vec!["-selection", "clipboard", "-o"]),
        ("xsel", vec!["--clipboard", "--output"]),
        ("pbpaste", vec![]),
    ] {
        if which(bin) {
            if let Ok(out) = std::process::Command::new(bin).args(&args).output() {
                if out.status.success() {
                    return Some((String::from_utf8_lossy(&out.stdout).to_string(), bin.into()));
                }
            }
        }
    }
    std::fs::read_to_string(clipboard_path())
        .ok()
        .map(|s| (s, "file".into()))
}

fn which(bin: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The copied envelope: the text, and where it came from. Structure survives the
/// copy — that is the whole difference from selecting characters in a terminal.
#[derive(serde::Serialize, serde::Deserialize)]
struct Clip {
    text: String,
    source: String,
    subject: String,
    seq: u64,
    at: String,
    links: Vec<String>,
    copied_at: String,
}

fn cmd_observe(root: &str, max_files: usize) -> anyhow::Result<()> {
    let path = observe::log_path();
    let known = observe::read(&path).unwrap_or_default();
    let before = known.len();
    let mut fresh: Vec<Observation> = Vec::new();
    fresh.extend(watch_fs(std::path::Path::new(root), &known, max_files));
    fresh.extend(watch_git(std::path::Path::new(root), 200));
    if let Some(home) = std::env::var_os("HOME") {
        let hist = std::path::Path::new(&home).join(".zsh_history");
        if hist.exists() {
            fresh.extend(watch_shell(&hist, &known, 200));
        }
    }
    // `watch_git` takes no `known` list, so a second `observe` on the same
    // repository re-appends every commit it already recorded. Found by running
    // this shell twice and seeing two positions with cosine 1.000 to each other
    // (`why 1650` → position 570, the same commit). The log is append-only by
    // design, so the fix belongs here, at the writer: drop anything whose
    // (source, subject) pair is already in the log.
    let seen: std::collections::HashSet<(String, String)> = known
        .iter()
        .map(|o| (o.source.clone(), o.subject.clone()))
        .collect();
    let before_dedup = fresh.len();
    fresh.retain(|o| !seen.contains(&(o.source.clone(), o.subject.clone())));
    let duplicates = before_dedup - fresh.len();
    // `append` returns (first_seq_written, last_seq_written) — NOT a count.
    // The first version of this line printed the first of those as "new" and
    // said "1733 new, 1732 in log", which is the kind of off-by-a-whole-meaning
    // that a reader trusts because it is printed by the tool that knows.
    let written = fresh.len();
    let (first_seq, last_seq) = observe::append(&path, &mut fresh)?;
    if duplicates > 0 {
        println!("{duplicates} already-recorded observations dropped before append");
    }
    if written == 0 {
        println!("nothing new — the log already holds {before} observations");
    } else {
        println!(
            "observed {written} new (seq {first_seq}..{last_seq}); log now holds {} → {}",
            before + written,
            path.display()
        );
    }
    println!("append-only: nothing in {root} was read for content beyond what the watchers record, and nothing was written.");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verb = args.first().map(|s| s.as_str()).unwrap_or("help");
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();

    if verb == "observe" {
        let root = if arg(1) == "--root" { arg(2) } else { ".".into() };
        let max_files = args
            .iter()
            .position(|a| a == "--max")
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(400);
        return cmd_observe(&root, max_files);
    }
    if matches!(verb, "help" | "-h" | "--help") {
        println!(
            "physis-world — a navigational layer over the filesystem, not a replacement\n\n\
             observe --root <dir>   append what is there now to the log (never writes your files)\n\
             inspect                what the world contains\n\
             locate <term>          where a thing sits: positions, sources, links\n\
             neighbors <term>       what is structurally near it\n\
             history <term>         every position that mentions it, in log order\n\
             changed <term>         positions where its derived transition is not persistence\n\
             compare <a> <b>        shared links, shared neighbourhood, similarity\n\
             trace <position>       the chain behind one position\n\
             why <position>         what this position follows from, and by which arm\n\
             retrieve <query...>    linked arm and blind arm, side by side\n\
             copy <term|position>   copy an object WITH its provenance and links\n\
             paste [--into FILE]    show the copied object, or append it to FILE\n"
        );
        return Ok(());
    }

    let w = World::load(5)?;

    match verb {
        "inspect" => {
            let mut by_source: HashMap<&str, usize> = HashMap::new();
            for o in &w.obs {
                *by_source.entry(o.source.as_str()).or_insert(0) += 1;
            }
            let linked = (0..w.n()).filter(|&i| !w.ents[i].is_empty()).count();
            let edges: usize = w.adj.iter().map(|a| a.len()).sum::<usize>() / 2;
            let parts = ws::components(&w.adj);
            let ncomp = parts.iter().max().map(|m| m + 1).unwrap_or(0);
            println!("world: {} observations, representation {}", w.n(), w.embedder);
            let mut srcs: Vec<_> = by_source.into_iter().collect();
            srcs.sort();
            for (s, c) in srcs {
                println!("  source {s:<10} {c}");
            }
            println!(
                "  {linked} positions carry at least one extracted link ({:.0}%)",
                100.0 * linked as f64 / w.n() as f64
            );
            println!("  {edges} mutual-kNN edges, {ncomp} components");
            println!(
                "  lexical margin {:.3} → recommended filter: {} (E65: right on every corpus where the arms differ, wrong on one where they tie)",
                w.margin,
                ws::filter_from_margin(w.margin)
            );
            let t = w.transitions();
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for l in &t {
                *counts.entry(l).or_insert(0) += 1;
            }
            let mut cs: Vec<_> = counts.into_iter().collect();
            cs.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            print!("  derived transitions:");
            for (l, c) in cs {
                print!(" {l}={c}");
            }
            println!("\n  (derivation measured at 0.486 vs a shuffled-order null of 0.291 — E56)");
        }
        "locate" => {
            let term = arg(1);
            let hits = w.positions_of(&term);
            if hits.is_empty() {
                // Fallback, and labelled as one. The link layer's recall is the
                // measured bottleneck (E60), so "no link" must not be reported
                // as "not here" — that would be Gap 3, a retriever that cannot
                // say what it does not contain, reintroduced by the interface.
                let needle = term.to_lowercase();
                let raw: Vec<usize> = (0..w.n())
                    .filter(|&i| w.texts[i].to_lowercase().contains(&needle))
                    .collect();
                if raw.is_empty() {
                    println!("'{term}': no extracted link and no string match in {} observations.", w.n());
                    println!("  that is a statement about this log, not about your disk. Widen it with `observe --root <dir> --max <n>`.");
                    return Ok(());
                }
                println!("'{term}': no extracted link — FALLING BACK TO STRING MATCH ({} hits).", raw.len());
                println!("  the structural layer missed this; what follows is plain text search, not navigation.");
                for &i in raw.iter().take(10) {
                    println!("{}", w.line(i));
                }
                return Ok(());
            }
            println!("'{term}' — {} positions", hits.len());
            for &i in hits.iter().take(10) {
                println!("{}", w.line(i));
                println!("{}", w.chain(i, "entity-link"));
            }
            let first = hits[0];
            let nb = &w.adj[first];
            if !nb.is_empty() {
                println!("\n  structurally near its first position:");
                for &j in nb.iter().take(5) {
                    println!("{}", w.line(j));
                }
            }
        }
        "neighbors" => {
            let term = arg(1);
            let (hits, how) = w.resolve(&term);
            let Some(&p) = hits.first() else {
                println!("'{term}': no link and no string match in {} observations.", w.n());
                return Ok(());
            };
            println!("resolved by {how}");
            println!("structural neighbourhood of '{term}' (position {p}, mutual-kNN k=5)");
            if w.adj[p].is_empty() {
                println!("  none — it is in no mutual-kNN edge. NOT an error: mutual means both directions agreed.");
            }
            for &j in &w.adj[p] {
                println!("{}   cos {:.3}", w.line(j), w.sim[p][j]);
                println!("{}", w.chain(j, "mutual-kNN"));
            }
        }
        "history" => {
            let term = arg(1);
            let (hits, how) = w.resolve(&term);
            let t = w.transitions();
            if hits.is_empty() {
                println!("'{term}': no link and no string match in {} observations.", w.n());
                return Ok(());
            }
            println!("resolved by {how}");
            println!("'{term}' through the log (position = the temporal coordinate, no clock used)");
            for &i in &hits {
                println!("  pos {i:>5}  {:<10} {}", t[i], w.texts[i].chars().take(80).collect::<String>());
            }
        }
        "changed" => {
            let term = arg(1);
            let t = w.transitions();
            let hits: Vec<usize> = w
                .positions_of(&term)
                .into_iter()
                .filter(|&i| t[i] != "persist" && t[i] != "none")
                .collect();
            println!("'{term}' — {} positions where the derived transition is not persistence", hits.len());
            for i in hits.iter().take(15) {
                println!("  pos {i:>5}  {:<10} {}", t[*i], w.texts[*i].chars().take(80).collect::<String>());
            }
            println!("  derivations, not facts: `disappear` rests on a six-word cue list, `recur` on an unswept gap (E56).");
        }
        "compare" => {
            let (a, b) = (arg(1), arg(2));
            let (pa, pb) = (w.positions_of(&a), w.positions_of(&b));
            let (Some(&ia), Some(&ib)) = (pa.first(), pb.first()) else {
                println!("one of '{a}' / '{b}' is not in this world.");
                return Ok(());
            };
            let sa: Vec<&String> = w.ents[ia].iter().collect();
            let shared: Vec<&&String> = sa.iter().filter(|e| w.ents[ib].contains(e)).collect();
            println!("compare '{a}' (pos {ia}) with '{b}' (pos {ib})");
            println!("  similarity            {:.3}", w.sim[ia][ib]);
            println!("  shared links          {shared:?}");
            let na: Vec<usize> = w.adj[ia].clone();
            let shared_nb: Vec<usize> = na.iter().copied().filter(|j| w.adj[ib].contains(j)).collect();
            println!("  shared neighbours     {shared_nb:?}");
            println!("{}", w.chain(ia, "compare"));
        }
        "trace" => {
            let p: usize = arg(1).parse().unwrap_or(0);
            if p >= w.n() {
                println!("position {p} is past the end of the log ({} observations).", w.n());
                return Ok(());
            }
            let o = &w.obs[p];
            println!("position {p}");
            println!("  observation  seq {} · {} · {} · by {}", o.seq, o.source, o.at, o.produced_by.clone().unwrap_or_else(|| "-".into()));
            println!("  subject      {}", o.subject);
            if !o.body.is_empty() {
                println!("  body         {}", o.body.chars().take(200).collect::<String>());
            }
            println!("  representation {} ({} dims)", w.embedder, w.vecs[p].len());
            println!("  extracted links {:?}", w.ents[p]);
            println!("  kNN neighbours  {:?}", w.adj[p]);
            println!("  derived transition {}", w.transitions()[p]);
            println!("  every line above is a function of the observation and the named operator; nothing was asserted by a model.");
        }
        "why" => {
            let p: usize = arg(1).parse().unwrap_or(0);
            if p >= w.n() {
                println!("position {p} is past the end of the log.");
                return Ok(());
            }
            println!("what position {p} follows from", );
            println!("{}", w.line(p));
            println!(
                "\n  linked arm (entity link + earlier position + similarity){}:",
                if ws::filter_from_margin(w.margin) == "entity-link" {
                    "  ← recommended for this world"
                } else {
                    ""
                }
            );
            for j in w.linked(p, 3) {
                println!("{}   cos {:.3}", w.line(j), w.sim[p][j]);
            }
            println!(
                "\n  lexical arm (BM25 over earlier positions, no model, no links){}:",
                if ws::filter_from_margin(w.margin) == "bm25+position" {
                    "  ← recommended for this world"
                } else {
                    ""
                }
            );
            for j in w.lexical(p, 3) {
                println!("{}", w.line(j));
            }
            println!("\n  blind arm (similarity alone, order ignored):");
            for j in w.blind(p, 3) {
                println!("{}   cos {:.3}", w.line(j), w.sim[p][j]);
            }
            println!(
                "\n  all three are shown because which one wins is corpus-dependent and measured.\n                   On real text the lexical arm leads: 0.535 vs 0.488 (docs), 0.447 vs 0.465 (log) — E63.\n                   On near-identical templates only the linked arm works: 0.700 vs 0.013 — E57.\n                   Redundancy decides, not sophistication."
            );
        }
        "retrieve" => {
            let q = args[1..].join(" ");
            if q.is_empty() {
                println!("retrieve <query...>");
                return Ok(());
            }
            let hits = w.nearest_to_query(&q, 5);
            println!("query: {q}\n\n  nearest positions (representation {}):", w.embedder);
            for &i in &hits {
                println!("{}", w.line(i));
            }
            if let Some(&seed) = hits.first() {
                let linked = w.linked(seed, 5);
                println!("\n  navigating from position {seed} by entity link + earlier position:");
                if linked.is_empty() {
                    println!("    no earlier position shares a link with it — the arm has nothing to add here, which E60 says is when plain similarity is the safer answer.");
                }
                for j in linked {
                    println!("{}", w.line(j));
                    println!("{}", w.chain(j, "link+position+cosine"));
                }
            }
        }
        "copy" => {
            let target = arg(1);
            let p = if let Ok(i) = target.parse::<usize>() {
                if i >= w.n() {
                    println!("position {i} is past the end of the log.");
                    return Ok(());
                }
                i
            } else {
                let (hits, how) = w.resolve(&target);
                let Some(&first) = hits.first() else {
                    println!("'{target}': nothing to copy — no link and no string match.");
                    return Ok(());
                };
                println!("resolved by {how} ({} positions, copying the first)", hits.len());
                first
            };
            let o = &w.obs[p];
            let clip = Clip {
                text: w.texts[p].clone(),
                source: o.source.clone(),
                subject: o.subject.clone(),
                seq: o.seq,
                at: o.at.to_rfc3339(),
                links: w.ents[p].clone(),
                copied_at: chrono::Utc::now().to_rfc3339(),
            };
            let envelope = serde_json::to_string_pretty(&clip).unwrap();
            let sink = clipboard_write(&envelope);
            println!("copied position {p} → {sink}");
            println!("{}", w.line(p));
            println!("  carries: seq {} · source {} · links {:?}", o.seq, o.source, w.ents[p]);
            // The copy is itself an observation. A world that records what it
            // was asked about but not what it handed over would have a hole in
            // exactly the place a user later asks "where did this come from".
            let mut ev = vec![physis_core::observe::Observation::new("clipboard", o.subject.clone())
                .with_body(format!("copied from seq {} ({})", o.seq, o.source))
                .by("physis-world copy")];
            let _ = physis_core::observe::append(&physis_core::observe::log_path(), &mut ev);
        }
        "paste" => {
            let Some((body, from)) = clipboard_read() else {
                println!("clipboard is empty.");
                return Ok(());
            };
            let clip: Option<Clip> = serde_json::from_str(&body).ok();
            let (text, prov) = match &clip {
                Some(c) => (
                    c.text.clone(),
                    format!("seq {} · {} · {} · links {:?}", c.seq, c.source, c.at, c.links),
                ),
                None => (body.clone(), "no physis envelope — plain text".to_string()),
            };
            let into = args
                .iter()
                .position(|a| a == "--into")
                .and_then(|i| args.get(i + 1).cloned());
            match into {
                Some(path) => {
                    // The only verb in this shell that writes to a file the user
                    // named. It appends, it never truncates, and it stamps the
                    // provenance beside the text so the paste stays traceable.
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)?;
                    writeln!(f, "{text}")?;
                    writeln!(f, "<!-- physis: {prov} -->")?;
                    println!("appended to {path} (from {from})");
                    println!("  {prov}");
                    let mut ev = vec![physis_core::observe::Observation::new("clipboard", path.clone())
                        .with_body(format!("pasted: {prov}"))
                        .by("physis-world paste")];
                    let _ = physis_core::observe::append(&physis_core::observe::log_path(), &mut ev);
                }
                None => {
                    println!("from {from} — {prov}\n");
                    println!("{text}");
                    println!("\n(nothing was written; pass --into <file> to append there)");
                }
            }
        }
        other => println!("unknown verb '{other}' — try `physis-world help`"),
    }
    Ok(())
}
