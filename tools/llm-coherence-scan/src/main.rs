use llm_coherence_scan::baseline;
use llm_coherence_scan::capture::RunRecord;
use llm_coherence_scan::drift::{self, ReferenceCorpus};
use llm_coherence_scan::embed;
use llm_coherence_scan::engine;
use llm_coherence_scan::overlay;
use llm_coherence_scan::refine;
use llm_coherence_scan::report;
use llm_coherence_scan::risk;
use llm_coherence_scan::workflow;
use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "physis-scan", about = "LLM I/O coherence + drift + refinement")]
struct Cli {
    /// Directory for physis-scan data (salience, proposals, overlays, history sled).
    #[arg(long, global = true, default_value = "./.physis-scan")]
    data_dir: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Score a single LLM run and update the refinement loop.
    Run {
        /// Path to RunRecord JSON.
        #[arg(long)]
        in_: PathBuf,
        /// Output directory for summary.txt, report.json, workflow.mmd.
        #[arg(long, default_value = "./out")]
        out: PathBuf,
        /// Skip the TUI dashboard even on a TTY.
        #[arg(long)]
        no_tui: bool,
        /// Optional reference corpus JSON for the reference drift component.
        #[arg(long)]
        ref_corpus: Option<PathBuf>,
        /// Additional asserted-failure labels (e.g. "hallucination", "wrong-file").
        #[arg(long, value_delimiter = ',')]
        asserted_failures: Vec<String>,
    },
    /// Manage a reference corpus of "known-good" runs.
    Baseline {
        #[command(subcommand)]
        cmd: BaselineCmd,
    },
    /// Drive the refinement loop: list / accept / reject discovery proposals.
    Refine {
        /// Accept a pending proposal by id (land it in the salience store).
        #[arg(long)]
        accept: Option<String>,
        /// Reject a pending proposal by id.
        #[arg(long)]
        reject: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum BaselineCmd {
    /// Add one or more run files to a named reference corpus.
    Add {
        name: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        to: PathBuf,
    },
    List {
        #[arg(long)]
        from: PathBuf,
    },
    Remove {
        name: String,
        #[arg(long)]
        from: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let now = Utc::now();
    std::fs::create_dir_all(&cli.data_dir).context("create data dir")?;
    let db = baseline::open_db(&cli.data_dir.join("history"))?;
    match cli.cmd {
        Cmd::Run { in_, out, no_tui: _no_tui, ref_corpus, asserted_failures } => {
            let rec = RunRecord::load(&in_)?;
            let signals = engine::score(&rec);
            let risk = risk::coherence_risk(&signals);
            let corpus = match ref_corpus.as_ref() {
                Some(p) => Some(ReferenceCorpus::load(p)?),
                None => None,
            };
            let drift = drift::model_drift(&rec, &db, corpus.as_ref())?;
            baseline::record(&db, &rec)?;

            let mut salience = refine::SalienceStore::load(&refine::salience_path(&cli.data_dir))?;
            refine::update_outcome_salience(&rec, &mut salience, now)?;
            salience.save(&refine::salience_path(&cli.data_dir))?;

            let dream = refine::run_dream_pass(&rec, risk.total, &asserted_failures);
            let new_proposals = refine::discover_proposals(&rec, &salience, now);
            let mut prop_store =
                refine::ProposalStore::load(&refine::proposals_path(&cli.data_dir))?;
            prop_store.pending.extend(new_proposals.iter().cloned());
            prop_store.save(&refine::proposals_path(&cli.data_dir))?;

            let overlays = overlay::load(&cli.data_dir)?;
            let hits = overlay::check(&rec, &overlays);

            let prompt_hash = {
                use sha2::{Digest, Sha256};
                let mut s = String::new();
                for b in Sha256::digest(rec.input.prompt.as_bytes()) {
                    s.push_str(&format!("{:02x}", b));
                }
                s
            };
            let full = report::build(&rec, risk, drift, dream, new_proposals, hits, prompt_hash);
            std::fs::create_dir_all(&out)?;
            std::fs::write(out.join("summary.txt"), report::one_line(&full))?;
            std::fs::write(
                out.join("report.json"),
                serde_json::to_string_pretty(&full)?,
            )?;
            std::fs::write(
                out.join("workflow.mmd"),
                workflow::render_mermaid(&rec, &risk, &full.drift),
            )?;
            println!("{}", report::one_line(&full));
        }
        Cmd::Baseline { cmd } => match cmd {
            BaselineCmd::Add { name, files, to } => {
                let mut corpus: ReferenceCorpus = std::fs::read_to_string(&to)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or(ReferenceCorpus { name: name.clone(), entries: vec![] });
                corpus.name = name.clone();
                for f in files {
                    let r = RunRecord::load(&f)?;
                    let text = embed::combine(
                        &r.input.prompt,
                        &r.input
                            .rag_files
                            .iter()
                            .map(|x| (x.path.clone(), x.body.clone()))
                            .collect::<Vec<_>>(),
                    );
                    let out_text = embed::combine(
                        &r.output.text,
                        &r.output
                            .edited_files
                            .iter()
                            .map(|x| (x.path.clone(), x.body.clone()))
                            .collect::<Vec<_>>(),
                    );
                    let emb = embed::hash_embed(&format!("{}\n{}", text, out_text)).to_vec();
                    corpus.entries.push(crate::drift::ReferenceEntry {
                        label: format!("{}:{}", r.model, r.task),
                        embedding: emb,
                    });
                }
                std::fs::write(&to, serde_json::to_string_pretty(&corpus)?)?;
                println!("wrote {} entries to {}", corpus.entries.len(), to.display());
            }
            BaselineCmd::List { from } => {
                let corpus: ReferenceCorpus = serde_json::from_str(&std::fs::read_to_string(&from)?)?;
                for e in corpus.entries {
                    println!("{}", e.label);
                }
            }
            BaselineCmd::Remove { name: _, from } => {
                let _ = from;
                anyhow::bail!("baseline remove not implemented in v1");
            }
        },
        Cmd::Refine { accept, reject } => {
            let mut store = refine::ProposalStore::load(&refine::proposals_path(&cli.data_dir))?;
            if let Some(id) = accept {
                store.pending.retain(|p| p.id != id);
                store.save(&refine::proposals_path(&cli.data_dir))?;
                println!("accepted {id}");
            } else if let Some(id) = reject {
                store.pending.retain(|p| p.id != id);
                store.save(&refine::proposals_path(&cli.data_dir))?;
                println!("rejected {id}");
            } else {
                if store.pending.is_empty() {
                    println!("(no pending proposals)");
                }
                for p in &store.pending {
                    println!(
                        "{} -> cell={} evidence={} excerpt={:?}",
                        p.id, p.proposed_cell, p.evidence_path, p.evidence_excerpt
                    );
                }
            }
        }
    }
    Ok(())
}
