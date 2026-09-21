use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

pub const DEFAULT_EXCLUDE_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "__pycache__",
    "node_modules",
    ".venv",
    "venv",
    ".tox",
    ".eggs",
    "eggs",
    "build",
    "dist",
    "target",
    ".gradle",
    "out",
    ".index",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".opencode",
    ".claude",
    ".cursor",
    ".agents",
    "aura_venv",
    ".terraform",
    ".serverless",
    // Physis's own state dir (sled store, nav index, transition log,
    // transcripts). It is the observer, not the observed: scanning it counts
    // our own bookkeeping as repository content and makes every diff see the
    // log entry the last command just wrote.
    ".physis",
];

pub const BINARY_EXTENSIONS: &[&str] = &[
    ".apk",
    ".aab",
    ".jar",
    ".aar",
    ".dex",
    ".class",
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".bmp",
    ".webp",
    ".ico",
    ".svg",
    ".mp3",
    ".mp4",
    ".avi",
    ".mov",
    ".wav",
    ".flac",
    ".ogg",
    ".m4a",
    ".pdf",
    ".doc",
    ".docx",
    ".xls",
    ".xlsx",
    ".ppt",
    ".pptx",
    ".zip",
    ".tar",
    ".gz",
    ".bz2",
    ".xz",
    ".7z",
    ".rar",
    ".so",
    ".dylib",
    ".dll",
    ".o",
    ".a",
    ".lib",
    ".woff",
    ".woff2",
    ".ttf",
    ".eot",
    ".pyc",
    ".pyo",
    ".pyd",
    ".whl",
    ".egg",
    ".deb",
    ".rpm",
    ".keystore",
    ".jks",
    ".p12",
    ".pfx",
    ".min.js",
    ".min.css",
];

pub const TEXT_EXTENSIONS: &[&str] = &[
    ".md",
    ".mdx",
    ".txt",
    ".rst",
    ".adoc",
    ".asciidoc",
    ".org",
    ".py",
    ".pyi",
    ".pyx",
    ".kt",
    ".kts",
    ".java",
    ".rs",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".mjs",
    ".cjs",
    ".vue",
    ".svelte",
    ".html",
    ".htm",
    ".xhtml",
    ".xml",
    ".css",
    ".scss",
    ".sass",
    ".less",
    ".stylus",
    ".go",
    ".rb",
    ".erb",
    ".c",
    ".h",
    ".cpp",
    ".hpp",
    ".cc",
    ".hh",
    ".cxx",
    ".hxx",
    ".swift",
    ".json",
    ".yaml",
    ".yml",
    ".toml",
    ".ini",
    ".cfg",
    ".conf",
    ".env",
    ".properties",
    ".gradle",
    ".gradle.kts",
    ".sh",
    ".bash",
    ".zsh",
    ".fish",
    ".sql",
    ".graphql",
    ".prisma",
    ".csv",
    ".tsv",
    ".log",
    ".lock",
    ".patch",
    ".proto",
    ".thrift",
    ".hcl",
];

pub const NAMED_FILES: &[&str] = &[
    "Dockerfile",
    "Makefile",
    "CMakeLists.txt",
    ".gitignore",
    ".dockerignore",
    "docker-compose.yml",
    "docker-compose.yaml",
];

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub abs_path: String,
    pub ext: String,
    pub size: u64,
    pub mtime: f64,
    pub hash: String,
    pub structural_lines: Vec<String>,
}

impl FileInfo {
    pub fn structural_path(&self) -> String {
        self.path.replace('/', " → ")
    }

    /// Browsable content labels for this file: one per structural line
    /// (`path → fn/struct`) plus the file's structural path. Callers register
    /// these as labeled core nodes so scanned/watch-ingested files AND their
    /// inner structures are recallable by content in the wiki.
    pub fn labels(&self) -> Vec<String> {
        let p = self.structural_path();
        let mut labels: Vec<String> = self
            .structural_lines
            .iter()
            .map(|line| format!("{} → {}", p, line))
            .collect();
        labels.push(p);
        labels
    }
}

#[allow(clippy::type_complexity)]
static EXTRACTORS: Lazy<Vec<(String, Vec<(Regex, String)>)>> = Lazy::new(build_extractors);

static HEADING_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(#{1,6})\s+(.+)").unwrap());

static EQ_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([A-Za-z_][A-Za-z0-9_.]*)\s*=").unwrap());

static BLOCK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\w+)\s*\{").unwrap());

fn build_extractors() -> Vec<(String, Vec<(Regex, String)>)> {
    let raw: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            ".py",
            vec![
                (r"^class\s+(\w+)", "class {name}"),
                (r"^(?:async\s+)?def\s+(\w+)", "fn {name}"),
            ],
        ),
        (
            ".kt",
            vec![
                (
                    r"^(?:class|object|interface|enum class|data class|sealed class|abstract class)\s+(\w+)",
                    "class {name}",
                ),
                (r"^fun\s+(\w+)", "fun {name}"),
                (r"^val\s+(\w+)", "val {name}"),
                (r"^var\s+(\w+)", "var {name}"),
            ],
        ),
        (
            ".java",
            vec![(
                r"^(?:public|private|protected)?\s*(?:class|interface|enum|@interface|record)\s+(\w+)",
                "class {name}",
            )],
        ),
        (
            ".rs",
            vec![
                // Functions: all qualifier combinations this repo actually uses.
                // Anchored at ^ after trimming, so comment-interior lines (which
                // start with `//` or `*` after trim) never match.
                // Order: most-qualified first so the longest prefix is captured.
                (
                    r"^pub\s*\(crate\)\s+async\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r"^pub\s*\(crate\)\s+unsafe\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r"^pub\s*\(crate\)\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r"^pub\s+async\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r"^pub\s+unsafe\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r#"^pub\s+extern\s+"C"\s+fn\s+(\w+)"#,
                    "fn {name}",
                ),
                (
                    r"^pub\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r"^async\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r"^unsafe\s+fn\s+(\w+)",
                    "fn {name}",
                ),
                (
                    r#"^extern\s+"C"\s+fn\s+(\w+)"#,
                    "fn {name}",
                ),
                (r"^fn\s+(\w+)", "fn {name}"),
                // Structs, enums, traits.
                (r"^pub\s*\([^)]+\)\s+struct\s+(\w+)", "struct {name}"),
                (r"^pub\s+struct\s+(\w+)", "struct {name}"),
                (r"^struct\s+(\w+)", "struct {name}"),
                (r"^pub\s*\([^)]+\)\s+enum\s+(\w+)", "enum {name}"),
                (r"^pub\s+enum\s+(\w+)", "enum {name}"),
                (r"^enum\s+(\w+)", "enum {name}"),
                (r"^pub\s*\([^)]+\)\s+trait\s+(\w+)", "trait {name}"),
                (r"^pub\s+trait\s+(\w+)", "trait {name}"),
                (r"^trait\s+(\w+)", "trait {name}"),
                // impl blocks: capture the implementing type name (after optional
                // generics and optional `Trait for`).
                (
                    r"^impl(?:\s*<[^>]*>)?\s+\w+\s+for\s+(\w+)",
                    "impl {name}",
                ),
                (
                    r"^impl(?:\s*<[^>]*>)?\s+(\w+)",
                    "impl {name}",
                ),
                // Modules, type aliases, constants.
                (r"^pub\s*\([^)]+\)\s+mod\s+(\w+)", "mod {name}"),
                (r"^pub\s+mod\s+(\w+)", "mod {name}"),
                (r"^mod\s+(\w+)", "mod {name}"),
                (r"^pub\s*\([^)]+\)\s+type\s+(\w+)", "type {name}"),
                (r"^pub\s+type\s+(\w+)", "type {name}"),
                (r"^type\s+(\w+)", "type {name}"),
                (r"^pub\s*\([^)]+\)\s+const\s+(\w+)", "const {name}"),
                (r"^pub\s+const\s+(\w+)", "const {name}"),
                (r"^const\s+(\w+)", "const {name}"),
            ],
        ),
        (
            ".ts",
            vec![
                (
                    r"^(?:export\s+)?(?:class|interface|type|enum)\s+(\w+)",
                    "class {name}",
                ),
                (r"^(?:export\s+)?function\s+(\w+)", "fn {name}"),
            ],
        ),
        (
            ".js",
            vec![
                (r"^(?:class|function)\s+(\w+)", "class {name}"),
                (r"^(?:const|let|var)\s+(\w+)\s*=", "const {name}"),
                (
                    r"^(?:export\s+)?(?:default\s+)?(?:function|class)\s+(\w+)",
                    "fn {name}",
                ),
            ],
        ),
        (
            ".go",
            vec![
                (r"^func\s+(\w+)", "fn {name}"),
                (r"^type\s+(\w+)\s+(?:struct|interface)", "type {name}"),
            ],
        ),
        (
            ".rb",
            vec![
                (r"^(?:class|module)\s+(\w+)", "class {name}"),
                (r"^def\s+(\w+)", "fn {name}"),
            ],
        ),
        (
            ".swift",
            vec![
                (
                    r"^(?:class|struct|enum|protocol|extension)\s+(\w+)",
                    "class {name}",
                ),
                (r"^func\s+(\w+)", "fn {name}"),
                (r"^var\s+(\w+)", "var {name}"),
            ],
        ),
        (".c", vec![(r"^struct\s+(\w+)", "struct {name}")]),
        (".h", vec![(r"^#define\s+(\w+)", "define {name}")]),
    ];

    raw.into_iter()
        .map(|(ext, patterns)| {
            let compiled: Vec<(Regex, String)> = patterns
                .into_iter()
                .map(|(re, fmt)| (Regex::new(re).unwrap(), fmt.to_string()))
                .collect();
            (ext.to_string(), compiled)
        })
        .collect()
}

pub fn extract_structural_lines(file_path: &Path, ext: &str) -> Vec<String> {
    let text = match std::fs::read_to_string(file_path) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut lines: Vec<String> = Vec::new();

    match ext {
        ".md" => {
            for line in text.lines() {
                if let Some(caps) = HEADING_RE.captures(line) {
                    let level = caps.get(1).unwrap().as_str().len();
                    let heading = caps.get(2).unwrap().as_str().trim();
                    if level <= 3 {
                        lines.push(format!("h{}:{}", level, heading));
                    }
                }
            }
            for para in text.split("\n\n") {
                let para = para.trim();
                if para.len() > 20 && para.len() < 200 {
                    if let Some(first_line) = para.lines().next() {
                        let fl = first_line.trim();
                        if !fl.starts_with('#')
                            && !fl.starts_with('-')
                            && !fl.starts_with('*')
                            && !fl.starts_with('>')
                            && fl.split_whitespace().count() <= 15
                        {
                            lines.push(fl.to_string());
                        }
                    }
                }
            }
        }
        ".json" => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(obj) = val.as_object() {
                    for (i, key) in obj.keys().enumerate() {
                        if i >= 30 {
                            break;
                        }
                        lines.push(format!("key:{key}"));
                    }
                }
            }
        }
        ".yaml" | ".yml" | ".toml" | ".ini" | ".cfg" | ".conf" | ".env" | ".properties" => {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(caps) = EQ_RE.captures(line) {
                    lines.push(format!("config:{}", caps.get(1).unwrap().as_str()));
                }
            }
        }
        ".gradle" | ".gradle.kts" => {
            let blocks: HashSet<&str> = [
                "plugins",
                "android",
                "dependencies",
                "defaultConfig",
                "buildTypes",
                "repositories",
                "signingConfigs",
            ]
            .into();
            for line in text.lines() {
                let line = line.trim();
                if let Some(caps) = BLOCK_RE.captures(line) {
                    let name = caps.get(1).unwrap().as_str();
                    if blocks.contains(name) {
                        lines.push(format!("block:{name}"));
                    }
                }
            }
        }
        ".csv" | ".tsv" => {
            if let Some(first_line) = text.lines().next() {
                let sep = if ext == ".csv" { "," } else { "\t" };
                for (i, h) in first_line.split(sep).enumerate() {
                    if i >= 20 {
                        break;
                    }
                    let h = h.trim();
                    if !h.is_empty() {
                        lines.push(format!("col:{h}"));
                    }
                }
            }
        }
        _ => {}
    }

    let all_extractors: Vec<(Regex, String)> = EXTRACTORS
        .iter()
        .find(|(e, _)| e == ext)
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    if !all_extractors.is_empty() {
        let mut seen = HashSet::new();
        for line in text.lines() {
            let trimmed = line.trim();
            for (re, fmt) in &all_extractors {
                if let Some(caps) = re.captures(trimmed) {
                    let name = caps.get(1).map_or("", |m| m.as_str());
                    let extracted = fmt.replace("{name}", name);
                    if seen.insert(extracted.clone()) {
                        lines.push(extracted);
                    }
                }
            }
        }
    }

    if lines.is_empty() && ext.is_empty() {
        if let Some(first) = text.lines().next() {
            if let Some(rest) = first.strip_prefix("#!") {
                lines.push(format!("shebang:{}", rest.trim()));
            }
        }
    }

    lines.truncate(50);
    lines
}

pub fn is_text_file(file_path: &Path) -> bool {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if ["png", "jpg", "jpeg"].contains(&ext.as_str()) {
        return true; // We handle these specifically in deep scan
    }
    match std::fs::read(file_path) {
        Ok(bytes) => {
            let head = &bytes[..bytes.len().min(512)];
            !head.contains(&0x00)
        }
        Err(_) => false,
    }
}

pub fn should_include_file(file_path: &Path, exclude_dirs: &HashSet<String>, root: &Path) -> bool {
    let rel = file_path.strip_prefix(root).unwrap_or(file_path);
    for parent in rel.parent().into_iter().flat_map(|p| p.ancestors()) {
        if let Some(name) = parent.file_name() {
            if exclude_dirs.contains(&name.to_string_lossy().to_string()) {
                return false;
            }
        }
    }

    if let Some(name) = file_path.file_name() {
        let name_str = name.to_string_lossy();
        if NAMED_FILES.contains(&name_str.as_ref()) {
            return true;
        }
    }

    let ext = file_path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();

    if BINARY_EXTENSIONS.contains(&ext.as_str()) {
        return false;
    }
    if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        return true;
    }

    if ext.is_empty() {
        if let Ok(mut f) = std::fs::File::open(file_path) {
            let mut buf = [0u8; 128];
            if std::io::Read::read_exact(&mut f, &mut buf).is_ok() && buf.starts_with(b"#!") {
                return true;
            }
        }
    }

    false
}

pub fn compute_file_hash(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

pub fn process_file(path: &Path, root: &Path) -> Option<FileInfo> {
    let rel_path = path.strip_prefix(root).unwrap_or(path);
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();

    let hash = compute_file_hash(path)?;
    let meta = std::fs::metadata(path).ok()?;
    let structural_lines = extract_structural_lines(path, &ext);

    Some(FileInfo {
        path: rel_path.to_string_lossy().to_string(),
        abs_path: path.to_string_lossy().to_string(),
        ext,
        size: meta.len(),
        mtime: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
        hash,
        structural_lines,
    })
}

pub fn scan_project(
    root_dir: &Path,
    extra_exclude_dirs: Option<&HashSet<String>>,
) -> Vec<FileInfo> {
    let root = root_dir
        .canonicalize()
        .unwrap_or_else(|_| root_dir.to_path_buf());
    let exclude: HashSet<String> = {
        let mut s: HashSet<String> = DEFAULT_EXCLUDE_DIRS.iter().map(|d| d.to_string()).collect();
        if let Some(extra) = extra_exclude_dirs {
            s.extend(extra.iter().cloned());
        }
        s
    };

    if !root.is_dir() {
        return vec![];
    }

    let file_paths: Vec<_> = WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !exclude.contains(name.as_ref())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    let mut files: Vec<FileInfo> = file_paths
        .par_iter()
        .filter_map(|path| {
            if !should_include_file(path, &exclude, &root) {
                return None;
            }
            if !is_text_file(path) {
                return None;
            }
            process_file(path, &root)
        })
        .collect();

    files.par_sort_by(|a, b| a.path.cmp(&b.path));
    files
}

pub fn load_hash_cache(cache_path: &Path) -> std::collections::HashMap<String, String> {
    if cache_path.exists() {
        std::fs::read_to_string(cache_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    }
}

pub fn save_hash_cache(
    cache_path: &Path,
    hashes: &std::collections::HashMap<String, String>,
) -> anyhow::Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(hashes)?;
    std::fs::write(cache_path, json)?;
    Ok(())
}

#[derive(Debug)]
pub struct ScanDiff {
    pub new: Vec<FileInfo>,
    pub changed: Vec<FileInfo>,
    pub deleted: Vec<String>,
    pub unchanged: Vec<FileInfo>,
    pub summary: ScanDiffSummary,
}

#[derive(Debug)]
pub struct ScanDiffSummary {
    pub new: usize,
    pub changed: usize,
    pub deleted: usize,
    pub unchanged: usize,
    pub total: usize,
}

pub fn compute_diff(
    current_files: &[FileInfo],
    cached_hashes: &std::collections::HashMap<String, String>,
) -> ScanDiff {
    let current: std::collections::HashMap<&str, &str> = current_files
        .iter()
        .map(|f| (f.path.as_str(), f.hash.as_str()))
        .collect();

    let new: Vec<FileInfo> = current_files
        .iter()
        .filter(|f| !cached_hashes.contains_key(&f.path))
        .cloned()
        .collect();

    let changed: Vec<FileInfo> = current_files
        .iter()
        .filter(|f| cached_hashes.get(&f.path).is_some_and(|h| h != &f.hash))
        .cloned()
        .collect();

    let deleted: Vec<String> = cached_hashes
        .keys()
        .filter(|p| !current.contains_key(p.as_str()))
        .cloned()
        .collect();

    let unchanged: Vec<FileInfo> = current_files
        .iter()
        .filter(|f| cached_hashes.get(&f.path) == Some(&f.hash))
        .cloned()
        .collect();

    let summary = ScanDiffSummary {
        new: new.len(),
        changed: changed.len(),
        deleted: deleted.len(),
        unchanged: unchanged.len(),
        total: current_files.len(),
    };

    ScanDiff {
        new,
        changed,
        deleted,
        unchanged,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rs_structural() {
        let dir = std::env::temp_dir().join("physis_test_scan");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("test.rs");
        std::fs::write(
            &file_path,
            "fn hello() {}\nstruct World {}\nfn another() {}",
        )
        .unwrap();
        let lines = extract_structural_lines(&file_path, ".rs");
        assert!(lines.contains(&"fn hello".to_string()));
        assert!(lines.contains(&"struct World".to_string()));
        assert!(lines.contains(&"fn another".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// All qualifier shapes measured in this repository: pub fn, pub async fn,
    /// pub(crate) fn, async fn, unsafe fn, pub struct, pub enum, pub trait,
    /// impl, impl Trait for Type. These must all produce structural lines.
    #[test]
    fn rust_qualified_declarations_are_extracted() {
        let dir = std::env::temp_dir().join("physis_test_scan_qualified");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("qualified.rs");
        std::fs::write(
            &file_path,
            concat!(
                "pub fn public_fn() {}\n",
                "pub async fn public_async_fn() {}\n",
                "pub(crate) fn crate_fn() {}\n",
                "pub(crate) async fn crate_async_fn() {}\n",
                "async fn bare_async_fn() {}\n",
                "unsafe fn bare_unsafe_fn() {}\n",
                "pub unsafe fn public_unsafe_fn() {}\n",
                "pub struct PublicStruct;\n",
                "pub enum PublicEnum {}\n",
                "pub trait PublicTrait {}\n",
                "pub mod pub_mod {}\n",
                "pub type PubType = u32;\n",
                "pub const PUB_CONST: u32 = 0;\n",
                "impl Engine {}\n",
                "impl<T> Container<T> {}\n",
                "impl Runnable for Engine {}\n",
            ),
        )
        .unwrap();
        let lines = extract_structural_lines(&file_path, ".rs");
        assert!(
            lines.contains(&"fn public_fn".to_string()),
            "pub fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"fn public_async_fn".to_string()),
            "pub async fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"fn crate_fn".to_string()),
            "pub(crate) fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"fn crate_async_fn".to_string()),
            "pub(crate) async fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"fn bare_async_fn".to_string()),
            "async fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"fn bare_unsafe_fn".to_string()),
            "unsafe fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"fn public_unsafe_fn".to_string()),
            "pub unsafe fn not found in {lines:?}"
        );
        assert!(
            lines.contains(&"struct PublicStruct".to_string()),
            "pub struct not found in {lines:?}"
        );
        assert!(
            lines.contains(&"enum PublicEnum".to_string()),
            "pub enum not found in {lines:?}"
        );
        assert!(
            lines.contains(&"trait PublicTrait".to_string()),
            "pub trait not found in {lines:?}"
        );
        assert!(
            lines.contains(&"mod pub_mod".to_string()),
            "pub mod not found in {lines:?}"
        );
        assert!(
            lines.contains(&"type PubType".to_string()),
            "pub type not found in {lines:?}"
        );
        assert!(
            lines.contains(&"const PUB_CONST".to_string()),
            "pub const not found in {lines:?}"
        );
        assert!(
            lines.contains(&"impl Engine".to_string()),
            "impl not found in {lines:?}"
        );
        assert!(
            lines.contains(&"impl Container".to_string()),
            "impl<T> not found in {lines:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Comment and string content must never become structural symbols.
    /// These are the exact false-positive forms the packet specifies.
    #[test]
    fn comments_and_strings_do_not_become_symbols() {
        let dir = std::env::temp_dir().join("physis_test_scan_negatives");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("negatives.rs");
        // The content has real declarations (so the extractor runs) and then
        // several comment/string forms that must not appear as symbols.
        // Note: string literal contents are on *indented* lines after trim,
        // but the content itself doesn't start with a valid qualifier prefix —
        // the surrounding `let _x = "..."` is what appears at top level.
        // We test the actual trimmed-line behaviour the extractor uses.
        std::fs::write(
            &file_path,
            concat!(
                "// pub fn fake_comment() {}\n",
                "/* pub struct FakeBlock; */\n",
                "pub fn real_fn() {}\n",
                "fn body() {\n",
                "    let _x = \"pub fn string_fn() {}\";\n",
                "    let _y = \"pub struct StringStruct;\";\n",
                "}\n",
            ),
        )
        .unwrap();
        let lines = extract_structural_lines(&file_path, ".rs");
        // Real declarations must appear.
        assert!(
            lines.contains(&"fn real_fn".to_string()),
            "real fn missing from {lines:?}"
        );
        assert!(
            lines.contains(&"fn body".to_string()),
            "real body fn missing from {lines:?}"
        );
        // Comment forms must NOT appear.
        // Line comment: `// pub fn fake_comment()` — after trim starts with `//`
        // so `^pub` does not match.
        assert!(
            !lines.contains(&"fn fake_comment".to_string()),
            "line-comment fn leaked into {lines:?}"
        );
        // Block comment opener: `/* pub struct FakeBlock; */` — starts with `/*`
        // so `^pub` does not match.
        assert!(
            !lines.contains(&"struct FakeBlock".to_string()),
            "block-comment struct leaked into {lines:?}"
        );
        // String literals are on indented lines; after trim the content of the
        // string does NOT become a line in isolation — the outer `let` is the
        // line. The extractor never sees `pub fn string_fn()` as a bare line.
        assert!(
            !lines.contains(&"fn string_fn".to_string()),
            "string-literal fn leaked into {lines:?}"
        );
        assert!(
            !lines.contains(&"struct StringStruct".to_string()),
            "string-literal struct leaked into {lines:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_labels_cover_path_and_lines() {
        let f = FileInfo {
            path: "src/main.rs".to_string(),
            abs_path: "/abs/src/main.rs".to_string(),
            ext: ".rs".to_string(),
            size: 10,
            mtime: 0.0,
            hash: "abc".to_string(),
            structural_lines: vec!["fn main".to_string(), "struct Foo".to_string()],
        };
        let labels = f.labels();
        assert!(
            labels.contains(&"src → main.rs".to_string()),
            "structural path is a label"
        );
        assert!(
            labels.contains(&"src → main.rs → fn main".to_string()),
            "per-line label"
        );
        assert!(
            labels.contains(&"src → main.rs → struct Foo".to_string()),
            "per-line label 2"
        );
        assert_eq!(labels.len(), 3);
    }

    #[test]
    fn test_is_text_file() {
        let dir = std::env::temp_dir().join("physis_test_text");
        let _ = std::fs::create_dir_all(&dir);
        let text_path = dir.join("text.txt");
        std::fs::write(&text_path, "hello").unwrap();
        assert!(is_text_file(&text_path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_diff() {
        let mut cache = std::collections::HashMap::new();
        cache.insert("a.rs".to_string(), "hash1".to_string());
        cache.insert("b.rs".to_string(), "hash2".to_string());

        let files = vec![
            FileInfo {
                path: "a.rs".to_string(),
                abs_path: "/a.rs".to_string(),
                ext: ".rs".to_string(),
                size: 10,
                mtime: 0.0,
                hash: "hash1".to_string(),
                structural_lines: vec![],
            },
            FileInfo {
                path: "c.rs".to_string(),
                abs_path: "/c.rs".to_string(),
                ext: ".rs".to_string(),
                size: 20,
                mtime: 0.0,
                hash: "hash3".to_string(),
                structural_lines: vec![],
            },
        ];

        let diff = compute_diff(&files, &cache);
        assert_eq!(diff.summary.new, 1);
        assert_eq!(diff.summary.unchanged, 1);
        assert_eq!(diff.summary.deleted, 1);
        assert_eq!(diff.new[0].path, "c.rs");
        assert_eq!(diff.deleted[0], "b.rs");
    }
}
