//! PH-012: physis-core public API snapshot.
//!
//! The core is the piece a customer embeds (Apache-2.0); nothing else pins
//! its public surface, so a refactor could silently break it. This test
//! regenerates the API inventory from source on every run and diffs it
//! against `tests/snapshots/physis_core_api.txt`. Any diff fails with the
//! diff itself plus bless instructions.
//!
//! Method (packet-sanctioned: `cargo public-api` is not installable here):
//! doc-grep over `physis-core/src/**/*.rs` tracking `pub mod` paths,
//! recording `pub` items with their full signatures (multi-line joined,
//! whitespace-normalized). Limitations, by design: macro-generated items
//! are recorded at their `macro_rules!`/declaration site only; `pub(crate)`
//! is NOT public surface and is skipped; `#[doc(hidden)]` items ARE recorded
//! (with a marker) so hiding stays deliberate and visible.
//!
//! To bless an intended change (human only, never CI):
//!   BLESS=1 cargo test --test api_stability

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SNAPSHOT: &str = "tests/snapshots/physis_core_api.txt";
const CORE_SRC: &str = "physis-core/src";

/// One public item: `path::to::Item — kind signature`.
fn inventory() -> BTreeSet<String> {
    let mut items = BTreeSet::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_rs(Path::new(CORE_SRC), &mut files);
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        // Module path from file location: src/foo/bar.rs → foo::bar
        // (src/lib.rs → crate root).
        let rel = f
            .strip_prefix(CORE_SRC)
            .unwrap()
            .with_extension("")
            .to_string_lossy()
            .replace(['/', '\\'], "::");
        let base: Vec<String> = if rel == "lib" {
            vec![]
        } else {
            // foo/mod → foo (mod.rs files live in their parent dir).
            let parts: Vec<&str> = rel.split("::").collect();
            if parts.last() == Some(&"mod") {
                parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect()
            } else {
                parts.iter().map(|s| s.to_string()).collect()
            }
        };
        scan_file(&text, &base, &mut items);
    }
    items
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// Scan one file, tracking `pub mod` nesting plus struct/enum/trait/impl
/// bodies (fields, variants, trait methods and inherent methods belong to
/// the type path, e.g. `core::PhysisCore::register_node_vec_labeled`).
fn scan_file(text: &str, base: &[String], items: &mut BTreeSet<String>) {
    // Stack of (name, brace_depth_inside, context).
    #[derive(Clone, Copy, PartialEq)]
    enum Ctx {
        Mod,
        Struct,
        Enum,
        Trait,
        Impl,
    }
    let mut stack: Vec<(String, i64, Ctx)> = Vec::new();
    let mut depth: i64 = 0;
    let mut pending_attr: Option<String> = None;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    // Current full path from base + named contexts (impl blocks contribute
    // their type name, not "impl").
    let cur_path = |base: &[String], stack: &[(String, i64, Ctx)]| -> Vec<String> {
        let mut path = base.to_vec();
        for (name, _, ctx) in stack {
            if *ctx == Ctx::Impl {
                // `impl Trait for Type` recorded as just Type below; the
                // stack holds the bare type name already.
                path.push(name.clone());
            } else {
                path.push(name.clone());
            }
        }
        path
    };
    while i < lines.len() {
        let t = lines[i].trim();
        // Attributes attach to the next item.
        if t.starts_with("#[doc(hidden)]") {
            pending_attr = Some("doc(hidden)".to_string());
            i += 1;
            continue;
        }
        if t.starts_with("#[non_exhaustive]") {
            let prev = pending_attr.take();
            pending_attr = Some(match prev {
                Some(p) => format!("{p}+non_exhaustive"),
                None => "non_exhaustive".to_string(),
            });
            i += 1;
            continue;
        }
        if t.starts_with("#[") {
            // Other attributes (derive, allow, cfg…) don't affect the surface.
            if t.ends_with(']') {
                i += 1;
                continue;
            }
            // Multi-line attribute: skip until balanced `]`.
            let mut open = t.bytes().filter(|&b| b == b'[').count();
            open -= t.bytes().filter(|&b| b == b']').count();
            i += 1;
            while open > 0 && i < lines.len() {
                open += lines[i].bytes().filter(|&b| b == b'[').count();
                open -= lines[i].bytes().filter(|&b| b == b']').count();
                i += 1;
            }
            continue;
        }
        // Pop closed contexts.
        while stack.last().is_some_and(|(_, d, _)| depth < *d) {
            stack.pop();
        }
        let in_struct = stack.last().is_some_and(|(_, _, c)| *c == Ctx::Struct);
        let in_enum = stack.last().is_some_and(|(_, _, c)| *c == Ctx::Enum);
        let in_trait = stack.last().is_some_and(|(_, _, c)| *c == Ctx::Trait);
        // Struct fields: `pub name: Ty,` inside a struct body.
        if in_struct && t.starts_with("pub ") && !t.starts_with("pub(") && !t.starts_with("pub fn") {
            let field = t.trim_end_matches(',').trim().to_string();
            let mut path = cur_path(base, &stack);
            let fname: String = field
                .strip_prefix("pub ")
                .unwrap_or(&field)
                .split([':', ' '])
                .next()
                .unwrap_or("?")
                .to_string();
            path.push(fname);
            let attr = pending_attr.take().map(|a| format!(" [{a}]")).unwrap_or_default();
            items.insert(format!("{} — field{attr} {}", path.join("::"), normalize(&field)));
            depth = (depth + count_braces_signed(t)).max(0);
            i += 1;
            continue;
        }
        // Enum variants: bare `Name`, `Name(..)`, `Name{..}`, `Name = 1`.
        if in_enum
            && !t.is_empty()
            && !t.starts_with("//")
            && !t.starts_with("pub ")
            && !t.starts_with("fn ")
            && !t.starts_with("impl ")
            && !t.starts_with("type ")
            && !t.starts_with("const ")
        {
            let vline = t.trim_end_matches(',').trim().to_string();
            if !vline.is_empty() && !vline.starts_with('{') && !vline.starts_with('}') && !vline.ends_with('{') {
                let vname: String = vline
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !vname.is_empty() && vname.chars().next().is_some_and(|c| c.is_uppercase()) {
                    let mut path = cur_path(base, &stack);
                    path.push(vname);
                    let attr = pending_attr.take().map(|a| format!(" [{a}]")).unwrap_or_default();
                    items.insert(format!("{} — variant{attr} {}", path.join("::"), normalize(&vline)));
                    // A struct-like variant body `{ .. }` spans lines — skip the
                    // opener but keep brace tracking so depth stays honest.
                    depth = (depth + count_braces_signed(t)).max(0);
                    i += 1;
                    continue;
                }
            }
            // Struct-variant bodies and anything else: brace-track only.
            pending_attr = None;
            depth = (depth + count_braces_signed(t)).max(0);
            i += 1;
            continue;
        }
        // Trait methods: `fn name(..)` without pub inside a trait body.
        if in_trait && (t.starts_with("fn ") || t.starts_with("unsafe fn ")) {
            let (sig, next) = join_signature(&lines, i);
            depth = (depth + count_braces_signed(&sig)).max(0);
            i = next;
            let mut path = cur_path(base, &stack);
            let name = item_name(&format!("pub {sig}")).unwrap_or_else(|| "?".to_string());
            path.push(name);
            let attr = pending_attr.take().map(|a| format!(" [{a}]")).unwrap_or_default();
            items.insert(format!("{} — method{attr} {}", path.join("::"), normalize(&sig)));
            continue;
        }
        // `impl [..] Type [..] [for Trait] {` — track the type for methods.
        if t.starts_with("impl ") && t.ends_with('{') {
            if let Some(ty) = parse_impl_type(t) {
                stack.push((ty, depth + 1, Ctx::Impl));
            }
            pending_attr = None;
            depth = (depth + count_braces_signed(t)).max(0);
            i += 1;
            continue;
        }
        if let Some(name) = parse_pub_mod(t) {
            if t.ends_with('{') {
                stack.push((name.clone(), depth + 1, Ctx::Mod));
            }
            let mut path = cur_path(base, &stack);
            path.push(name.clone());
            let attr = pending_attr.take().map(|a| format!(" [{a}]")).unwrap_or_default();
            items.insert(format!("{} — mod{attr}", path.join("::")));
            depth = (depth + count_braces_signed(t)).max(0);
            i += 1;
            continue;
        }
        if t.starts_with("pub ") && !t.starts_with("pub(") {
            let (sig, next) = join_signature(&lines, i);
            depth = (depth + count_braces_signed(&sig)).max(0);
            i = next;
            let kind = item_kind(&sig);
            let name = item_name(&sig).unwrap_or_else(|| "?".to_string());
            // Push struct/enum/trait bodies so members resolve under them.
            // A `{`-terminated struct/enum/trait opens member context;
            // functions end at their body brace (no push — body skipped by
            // join_signature stopping at the opening `{`).
            let opens_members = sig.trim_end().ends_with('{')
                && matches!(kind, "struct" | "enum" | "trait");
            if opens_members {
                let ctx = match kind {
                    "struct" => Ctx::Struct,
                    "enum" => Ctx::Enum,
                    _ => Ctx::Trait,
                };
                stack.push((name.clone(), depth, ctx));
            }
            let mut path = cur_path(base, &stack);
            // For member-opening items the name is already the pushed ctx.
            if !opens_members {
                path.push(name);
            }
            let attr = pending_attr.take().map(|a| format!(" [{a}]")).unwrap_or_default();
            items.insert(format!("{} — {kind}{attr} {}", path.join("::"), normalize(&sig)));
            continue;
        }
        pending_attr = None;
        depth = (depth + count_braces_signed(t)).max(0);
        i += 1;
    }
}

/// `impl ... Type ... {` → the bare type name (generics/where stripped).
/// `impl Trait for Type` → Type. Returns None when unparseable.
fn parse_impl_type(t: &str) -> Option<String> {
    let rest = t.strip_prefix("impl ")?;
    // Drop leading generic params `<...>`.
    let mut rest = rest.trim();
    if let Some(r) = rest.strip_prefix('<') {
        let mut bal = 0i32;
        let mut idx = None;
        for (n, c) in r.char_indices() {
            match c {
                '<' => bal += 1,
                '>' => {
                    if bal == 0 {
                        idx = Some(n);
                        break;
                    }
                    bal -= 1;
                }
                _ => {}
            }
        }
        rest = r[idx? + 1..].trim();
    }
    // `Trait for Type` → take after `for`.
    if let Some(pos) = rest.find(" for ") {
        rest = rest[pos + 5..].trim();
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    let name = name.split("::").last().unwrap_or("").to_string();
    if name.is_empty() { None } else { Some(name) }
}

/// `pub mod name;` or `pub mod name {` → Some(name).
fn parse_pub_mod(t: &str) -> Option<String> {
    let rest = t.strip_prefix("pub mod ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

fn item_kind(sig: &str) -> &'static str {
    let s = sig.strip_prefix("pub ").unwrap_or(sig);
    for kind in ["fn", "struct", "enum", "trait", "mod", "const", "static", "type", "use", "impl", "macro"] {
        if s == kind || s.starts_with(&format!("{kind} ")) || s.starts_with(&format!("{kind}<")) {
            return kind;
        }
    }
    "item"
}

fn item_name(sig: &str) -> Option<String> {
    let s = sig.strip_prefix("pub ")?;
    let s = s.strip_prefix("unsafe ").unwrap_or(s);
    for prefix in ["fn ", "struct ", "enum ", "trait ", "type "] {
        if let Some(r) = s.strip_prefix(prefix) {
            let name: String = r
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            return if name.is_empty() { None } else { Some(name) };
        }
    }
    if let Some(r) = s.strip_prefix("const ").or_else(|| s.strip_prefix("static ")) {
        // `const NAME: Ty` — name before `:`.
        return r.split(':').next().map(|n| n.trim().to_string());
    }
    if let Some(r) = s.strip_prefix("use ") {
        return Some(r.trim_end_matches(';').trim().to_string());
    }
    None
}

/// Join a possibly multi-line signature. Stops at the first line ending
/// with `;` (use/type/const/static, unit structs) or `{` (fn bodies and
/// struct/enum/trait member blocks open here — bodies and members are
/// recorded separately, never inside the signature). Stops before doc
/// comments and attributes (cap 12 lines).
fn join_signature(lines: &[&str], start: usize) -> (String, usize) {
    let mut acc = String::new();
    let mut i = start;
    while i < lines.len() && i < start + 12 {
        let t = lines[i].trim();
        if t.starts_with("///") || t.starts_with("//") || t.starts_with("#[") {
            break;
        }
        if !acc.is_empty() {
            acc.push(' ');
        }
        // `pub static` / `pub const` initializers are not API — keep the
        // declaration (`pub static NAME: Ty`), drop the value.
        if acc.is_empty() && (t.starts_with("pub static ") || t.starts_with("pub const ")) {
            if let Some(eq) = t.find('=') {
                acc.push_str(t[..eq].trim_end());
                acc.push(';');
                i += 1;
                break;
            }
        }
        acc.push_str(t);
        i += 1;
        if t.ends_with(';') || t.ends_with('{') {
            break;
        }
    }
    (acc, i)
}

/// Signed brace delta for depth tracking (the scanner stores depth as i64
/// and pops contexts when it drops below their open depth; strings can
/// unbalance the count, which the depth check repairs on the next close).
fn count_braces_signed(t: &str) -> i64 {
    // Strings can unbalance the count; struct literals in const initializers
    // are the main risk — under-counting only delays a pop, which the depth
    // check repairs on the next real close.
    let open = t.bytes().filter(|&b| b == b'{').count() as i64;
    let close = t.bytes().filter(|&b| b == b'}').count() as i64;
    open - close
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn core_public_api_matches_snapshot() {
    let items = inventory();
    let rendered = items.iter().cloned().collect::<Vec<_>>().join("\n") + "\n";
    if std::env::var("BLESS").is_ok() {
        std::fs::create_dir_all("tests/snapshots").unwrap();
        std::fs::write(SNAPSHOT, &rendered).unwrap();
        eprintln!("blessed {} public items to {SNAPSHOT}", items.len());
        return;
    }
    let expected = std::fs::read_to_string(SNAPSHOT)
        .unwrap_or_else(|_| panic!("missing {SNAPSHOT}: run BLESS=1 once to create it (human only)"));
    if rendered != expected {
        let exp: BTreeSet<&str> = expected.lines().collect();
        let got: BTreeSet<&str> = rendered.lines().collect();
        let mut diff = String::from("public API drift (run BLESS=1 only to bless a deliberate, human-approved change):\n");
        for line in exp.difference(&got) {
            diff.push_str(&format!("- {line}\n"));
        }
        for line in got.difference(&exp) {
            diff.push_str(&format!("+ {line}\n"));
        }
        panic!("{diff}");
    }
}
