//! techne demo — run the codebase machine on a slice of physis_pro's own module
//! layout: cluster files into capabilities, build the dependency graph, and trace
//! impact ("what breaks if the embedder changes").
//!
//!   cargo run --example techne_demo

use physis_pro::machines::techne::{
    classify_capabilities, dependency_edges, impact_of, SourceFile,
};

fn f(path: &str, lines: &[&str]) -> SourceFile {
    SourceFile {
        path: path.to_string(),
        lines: lines.iter().map(|s| s.to_string()).collect(),
    }
}

fn main() {
    // A representative slice of the engine's modules + their imports.
    let files = vec![
        f("src/embed/mod.rs", &[]),
        f("src/embed/clip.rs", &["use crate::embed::VectorEmbed;"]),
        f("src/classify.rs", &["use crate::embed::VectorEmbed;"]),
        f(
            "src/core.rs",
            &[
                "use crate::trie::Trie;",
                "use crate::graph::HolonicGraph;",
                "use crate::embed::VectorEmbed;",
            ],
        ),
        f("src/trie.rs", &[]),
        f("src/graph.rs", &[]),
        f(
            "src/machines/chronos.rs",
            &[
                "use crate::core::PhysisCore;",
                "use crate::embed::VectorEmbed;",
            ],
        ),
        f("src/machines/techne.rs", &["use crate::core::PhysisCore;"]),
        f(
            "src/bin/web_server.rs",
            &[
                "use crate::core::PhysisCore;",
                "use crate::classify::CellClassifier;",
            ],
        ),
        f("src/spanner.rs", &["use crate::core::PhysisCore;"]),
        f("src/auth_guard.rs", &[]),
    ];

    println!("techne — codebase machine");
    println!("perceived {} files\n", files.len());

    println!("CAPABILITIES (files clustered by purpose):");
    for c in classify_capabilities(&files) {
        println!("  • {:<16} {}", c.name, c.files.join(", "));
    }

    let edges = dependency_edges(&files);
    println!("\nDEPENDENCY EDGES (module → module):");
    for (from, to) in &edges {
        println!("  {} → {}", from, to);
    }

    println!("\nIMPACT — what breaks if `embed` changes:");
    for m in impact_of(&edges, "embed") {
        println!("  ⚠ {}", m);
    }

    println!("\nIMPACT — what breaks if `core` changes:");
    for m in impact_of(&edges, "core") {
        println!("  ⚠ {}", m);
    }
}
