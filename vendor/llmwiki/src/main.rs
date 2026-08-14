mod indexer;
mod inverted;
mod mcp;
mod parser;
mod search;
mod semantic;
mod watcher;
mod writer;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "llmwiki — local-first wiki search and indexing")]
struct Cli {
    /// Path to the wiki root directory (defaults to current directory)
    #[arg(long, global = true)]
    wiki_root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the .index/ directory structure in the wiki root
    Init,

    /// Index wiki files and build the search index
    Index {
        /// Only re-index files whose content has changed
        #[arg(long)]
        incremental: bool,
    },

    /// List indexed wiki pages, optionally filtered by path prefix or tag
    List {
        /// Filter by path prefix
        #[arg(long)]
        path: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Search the wiki (default: hybrid keyword + semantic)
    Search {
        /// Search query
        query: String,

        /// Use keyword-only search (BM25, no semantic)
        #[arg(long)]
        keyword: bool,

        /// Use semantic-only vector search
        #[arg(long)]
        semantic: bool,
    },

    /// Read a wiki page
    Read {
        /// Path to the page (relative to wiki root)
        path: String,

        /// Extract only the named section by heading
        #[arg(long)]
        section: Option<String>,
    },

    /// Start the MCP server (Phase 05)
    Mcp,

    /// Watch for file changes and auto-index
    Watch,

    /// Start the HTTP server (Phase 07)
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,
    },

    /// Write a wiki page from a file (CLI testing only)
    #[command(hide = true)]
    Write {
        /// Relative path within the wiki (must end with .md)
        path: String,
        /// Path to a local file containing the page content
        content_file: String,
    },

    /// Register llmwiki as an MCP server for Claude Desktop and/or Claude Code
    SetupClaude {
        /// Configure for Claude Desktop only
        #[arg(long)]
        desktop: bool,

        /// Configure for Claude Code only
        #[arg(long)]
        code: bool,

        /// (Claude Code) Add to global ~/.claude/settings.json instead of local .mcp.json
        #[arg(long)]
        global: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let wiki_root = cli
        .wiki_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    if let Err(e) = run(cli.command, wiki_root).await {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

async fn run(command: Commands, wiki_root: PathBuf) -> Result<()> {
    match command {
        Commands::Init => {
            indexer::ensure_index_dirs(&wiki_root)?;
            println!("Initialized .index/ in {}", wiki_root.display());

            // Add .index/ reminder to .gitignore if not already present
            let gitignore_path = wiki_root.join(".gitignore");
            let existing = if gitignore_path.exists() {
                std::fs::read_to_string(&gitignore_path)?
            } else {
                String::new()
            };
            if !existing.contains(".index/") {
                let mut content = existing;
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(".index/\n");
                std::fs::write(&gitignore_path, content)?;
                println!("Added .index/ to .gitignore");
            }
        }

        Commands::Index { incremental } => {
            let start = Instant::now();
            // Force full rebuild if search index files are missing
            let tantivy_meta = wiki_root.join(".index").join("tantivy").join("meta.json");
            #[cfg(feature = "semantic")]
            let index_complete =
                tantivy_meta.exists() && wiki_root.join(".index").join("vectors.usearch").exists();
            #[cfg(not(feature = "semantic"))]
            let index_complete = tantivy_meta.exists();
            let effective_incremental = incremental && index_complete;
            let stats = indexer::index_root(&wiki_root, effective_incremental)?;
            let elapsed = start.elapsed().as_secs_f64();
            let total = stats.added + stats.updated + stats.skipped;
            println!(
                "Indexed {} files ({} added, {} updated, {} skipped, {} removed) in {:.2}s",
                total, stats.added, stats.updated, stats.skipped, stats.removed, elapsed
            );
        }

        Commands::List { path, tag } => {
            let metas = indexer::load_meta(&wiki_root)?;
            let filtered: Vec<_> = metas
                .iter()
                .filter(|m| {
                    if let Some(ref p) = path {
                        if !m.path.starts_with(p.as_str()) {
                            return false;
                        }
                    }
                    if let Some(ref t) = tag {
                        if !m.tags.iter().any(|tag| tag == t) {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if filtered.is_empty() {
                println!("No pages found.");
                return Ok(());
            }

            for meta in filtered {
                println!("{}", meta.path);
                println!("  {}", meta.description);
                let tags_str = meta.tags.join(", ");
                let mut meta_parts = vec![format!("tags: [{}]", tags_str)];
                if let Some(conf) = meta.confidence {
                    meta_parts.push(format!("confidence: {:.2}", conf));
                }
                if let Some(ref upd) = meta.updated {
                    meta_parts.push(format!("updated: {}", upd));
                }
                println!("  {}", meta_parts.join(" | "));
            }
        }

        Commands::Search {
            query,
            keyword,
            semantic,
        } => {
            let all_meta = indexer::load_meta(&wiki_root)?;
            let meta_map: HashMap<String, indexer::FileMeta> =
                all_meta.into_iter().map(|m| (m.path.clone(), m)).collect();

            let raw: Vec<(String, f32)> = if keyword {
                // Keyword-only (BM25)
                inverted::keyword_search(&wiki_root, &query, 10)?
            } else if semantic {
                // Semantic-only (vector)
                crate::semantic::semantic_search(&wiki_root, &query, 10)?
            } else {
                // Hybrid (default): RRF fusion of keyword + semantic
                let kw_raw = inverted::keyword_search(&wiki_root, &query, 10)
                    .unwrap_or_default();
                let sem_raw = crate::semantic::semantic_search(&wiki_root, &query, 10)
                    .unwrap_or_default();
                search::rrf_merge(&kw_raw, &sem_raw, 60.0)
            };

            if raw.is_empty() {
                println!("No results found for \"{}\".", query);
                return Ok(());
            }

            let results: Vec<search::SearchResult> = raw
                .into_iter()
                .enumerate()
                .map(|(i, (path, score))| {
                    let description = meta_map
                        .get(&path)
                        .map(|m| m.description.clone())
                        .unwrap_or_default();
                    let match_locations = if !semantic {
                        inverted::get_match_locations(&wiki_root, &path, &query)
                    } else {
                        vec![]
                    };
                    search::SearchResult {
                        rank: i + 1,
                        path,
                        score,
                        description,
                        match_locations,
                    }
                })
                .collect();

            println!("{}", search::format_results(&results));
        }

        Commands::Read { path, section } => {
            let abs_path = wiki_root.join(&path);

            if !abs_path.exists() {
                anyhow::bail!("File not found: {}", path);
            }

            if abs_path.extension().and_then(|e| e.to_str()) != Some("md") {
                anyhow::bail!("Not a markdown file: {}", path);
            }

            let content = std::fs::read_to_string(&abs_path)
                .with_context(|| format!("Failed to read {}", path))?;

            if let Some(heading) = section {
                match parser::extract_section(&content, &heading) {
                    Some(text) => println!("{}", text),
                    None => println!("Section not found: \"{}\"", heading),
                }
            } else {
                print!("{}", content);
            }
        }

        Commands::Mcp => {
            mcp::run_mcp_server(wiki_root).await?;
        }

        Commands::Watch => {
            watcher::watch(&wiki_root)?;
        }

        Commands::Write { path, content_file } => {
            let content = std::fs::read_to_string(&content_file)
                .with_context(|| format!("Failed to read content file: {}", content_file))?;
            let result = writer::write_page(&wiki_root, &path, &content)?;
            let action = if result.created { "created" } else { "updated" };
            println!("{}: {} (reindexed: {})", action, result.path, result.reindexed);
        }

        Commands::Serve { .. } => {
            let wiki_root_watcher = wiki_root.clone();
            std::thread::spawn(move || {
                if let Err(e) = watcher::watch(&wiki_root_watcher) {
                    eprintln!("Watcher error: {:#}", e);
                }
            });
            mcp::run_mcp_server(wiki_root).await?;
        }

        Commands::SetupClaude {
            desktop,
            code,
            global,
        } => {
            // If neither --desktop nor --code specified, do both
            let do_desktop = desktop || !code;
            let do_code = code || !desktop;

            if do_desktop {
                setup_claude_desktop(&wiki_root)?;
            }
            if do_code {
                setup_claude_code(&wiki_root, global)?;
            }
        }
    }

    Ok(())
}

fn claude_desktop_config_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").context("APPDATA not set")?;
        Ok(PathBuf::from(appdata)
            .join("Claude")
            .join("claude_desktop_config.json"))
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("Claude")
            .join("claude_desktop_config.json"))
    }
}

fn resolve_binary_and_wiki(wiki_root: &PathBuf) -> Result<(PathBuf, PathBuf)> {
    let binary_path = std::env::current_exe()
        .context("Failed to determine binary path")?
        .canonicalize()
        .context("Failed to canonicalize binary path")?;
    let wiki_root_abs = std::fs::canonicalize(wiki_root)
        .with_context(|| format!("Wiki root not found: {}", wiki_root.display()))?;
    Ok((binary_path, wiki_root_abs))
}

fn mcp_server_entry(binary_path: &PathBuf, wiki_root_abs: &PathBuf) -> serde_json::Value {
    serde_json::json!({
        "command": binary_path.to_string_lossy(),
        "args": ["serve", "--wiki-root", wiki_root_abs.to_string_lossy()]
    })
}

fn upsert_mcp_server(
    config_path: &PathBuf,
    server_name: &str,
    entry: serde_json::Value,
) -> Result<bool> {
    let mut config: serde_json::Value = if config_path.exists() {
        let data = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        serde_json::from_str(&data)
            .with_context(|| format!("Invalid JSON in {}", config_path.display()))?
    } else {
        serde_json::json!({})
    };

    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }

    let servers = config["mcpServers"]
        .as_object_mut()
        .ok_or_else(|| anyhow!("mcpServers is not an object"))?;

    let already_exists = servers.contains_key(server_name);
    servers.insert(server_name.to_string(), entry);

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let formatted = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, formatted)
        .with_context(|| format!("Failed to write {}", config_path.display()))?;

    Ok(already_exists)
}

fn setup_claude_code(wiki_root: &PathBuf, global: bool) -> Result<()> {
    let (binary_path, wiki_root_abs) = resolve_binary_and_wiki(wiki_root)?;
    let entry = mcp_server_entry(&binary_path, &wiki_root_abs);

    // Derive a unique server name from wiki folder name
    let wiki_name = wiki_root_abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("llmwiki");
    let server_name = format!("llmwiki-{}", wiki_name);

    if global {
        // Write to ~/.claude/settings.json
        let home = std::env::var("HOME").context("HOME not set")?;
        let config_path = PathBuf::from(home)
            .join(".claude")
            .join("settings.json");

        let already_exists = upsert_mcp_server(&config_path, &server_name, entry)?;

        let action = if already_exists { "Updated" } else { "Added" };
        println!("{} {} in {}", action, server_name, config_path.display());
    } else {
        // Write .mcp.json in the wiki root
        let config_path = wiki_root_abs.join(".mcp.json");

        let already_exists = upsert_mcp_server(&config_path, "llmwiki", entry)?;

        let action = if already_exists { "Updated" } else { "Created" };
        println!("{} {}", action, config_path.display());
    }

    println!("  command: {}", binary_path.display());
    println!("  wiki-root: {}", wiki_root_abs.display());
    Ok(())
}

fn setup_claude_desktop(wiki_root: &PathBuf) -> Result<()> {
    let config_path = claude_desktop_config_path()?;
    let (binary_path, wiki_root_abs) = resolve_binary_and_wiki(wiki_root)?;
    let entry = mcp_server_entry(&binary_path, &wiki_root_abs);

    let already_exists = upsert_mcp_server(&config_path, "llmwiki", entry)?;

    let action = if already_exists { "Updated" } else { "Added" };
    println!("{} llmwiki in {}", action, config_path.display());
    println!("  command: {}", binary_path.display());
    println!("  wiki-root: {}", wiki_root_abs.display());
    println!("\nRestart Claude Desktop to pick up the changes.");

    Ok(())
}
