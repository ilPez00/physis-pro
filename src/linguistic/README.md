# Linguistic Routing Middleware

A native multi-modal middleware that intercepts systemic data streams, logs, and ontological metrics, transforming them through three distinct linguistic filters.

## Architecture

```
raw_data
  │
  ├── WenyanFilter   → long-term uberwiki archival (compression)
  ├── PirahaFilter   → silent internal sub-process logging (hidden from user)
  └── SanskritEngine → background "dreaming" (generative semantic expansion)
```

### `LinguisticRouter`

The main entry point. Routes data to all three filters simultaneously:

```rust
let router = LinguisticRouter::new();
let result = router.route("morning run 5k", LinguisticLense::Wenyan);
let all = router.route_all("morning run 5k");
// all[Wenyan]  → "晨/跑/5k"
// all[Piraha]  → "morning run 5k"
// all[Sanskrit] → "~ dreaming of morning ~ ..."
```

## Filters

### Wenyan (Classical Chinese)

**Purpose**: Long-term structural compression for uberwiki archival.

**How it works**:
- Maps ~80 English keywords to single CJK characters via a static dictionary
- Transliterates unknown words to 4-char lowercase abbreviations
- Formats output as path hierarchies: `身健/晨/跑·75%`
- Domain prefix matching uses longest-key priority to resolve compound domains (e.g. "Body & Fitness" → "身健")

**Key methods**:
- `compress(text)` — basic compression
- `compress_domain(text)` — domain name → CJK prefix
- `compress_goal(name, domain, progress)` — structured goal archival: `{domain}/{name}·{pct}%`
- `compress_experience(action, grade, rationale)` — experience archival: `行:{action}·評{grade}·由:{rationale}`

### Pirahã (Caveman-Ultra)

**Purpose**: Real-time silent internal logging. Output is strictly hidden from the user.

**How it works**:
- Strips ~160 function words across 9 categories: articles, prepositions, auxiliary verbs, pronouns, conjunctions, quantifiers, filler words, linking words
- Splits on path separators (`→`, `/`) for trie-based logs
- Preserves numeric values with smart formatting (integers stay integer, floats to 2 decimals)
- Multi-line input is joined with ` | ` separator

**Key methods**:
- `filter(text)` — strip function words, return compressed text
- `log_event(type, data, grade)` → `[SCAN] found file G75`
- `log_goal(name, progress)` → `GOAL morning run P50`
- `log_experience(action, grade, rationale)` → `ACT ran 5k G80 REASON good run`

### Sanskrit (Dreaming)

**Purpose**: Low-priority background thread generative expansion. "Dreams" on data to produce semantic associations.

**How it works**:
- Maintains a 35-concept semantic net mapping keywords to 4-5 associative terms
- Each seed word appears only once per dream (no duplicate expansions)
- Unknown words are echoed in a `⋮ word · word ⋮` fallback format
- Supports multi-entry synthesis for holistic reports

**Key methods**:
- `dream(text)` → `~ dreaming of run ~\n  run blossoms into sprint, jog, move, flow, circulate\n  ~ thus the seed 'run' yearns for form ~`
- `dream_on_domain(domain, progress)` — add progress context
- `dream_on_experience(action, grade, rationale)` — experience dreaming
- `synthesize(&[(domain, progress)])` — multi-domain synthesis report

## Integration

### With PDCActor
```rust
let router = LinguisticRouter::new();
let piraha_log = router.route(
    &format!("{} {}", action, rationale),
    LinguisticLense::Piraha,
);
eprintln!("[PHYSIS_INTERNAL] {}", piraha_log);
```

### With DreamEngine
```rust
let router = LinguisticRouter::new();
for dream in engine.ungraded_dreams() {
    let wenyan = router.route(&dream.description, LinguisticLense::Wenyan);
    // archive to uberwiki
    let sanskrit = router.route(&dream.description, LinguisticLense::Sanskrit);
    // feed into background dreaming thread
}
```

### With CLI / logging
```rust
// Silent internal bus — goes to stderr, never stdout
let piraha = router.route(data, LinguisticLense::Piraha);
eprintln!("[PIRAHA] {}", piraha);
```
