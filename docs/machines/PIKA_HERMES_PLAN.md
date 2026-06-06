# Implementing the Philosophical Machines in pika-hermes

How the machines (`docs/machines/MACHINES_BUILD_PLAN.md`) get a runtime + an
operator inside **pika-hermes-rust** — the agent framework that already embeds a
physis client. The agent's LLM invokes machines as **tools**; physis_pro stays
the engine.

## What pika-hermes already gives us

(`pika-hermes-rust2/src/`)

| Piece | File | Role for machines |
|-------|------|-------------------|
| `BaseTool` trait | `tools/types.rs` | `name / description / parameters / execute → ToolResult` — each machine surfaces as one or more tools |
| `ToolRegistry` | `tools/registry.rs` | register machine tools so the agent can call them |
| `PhysisClient` | `physis/client.rs` | HTTP client (today: `ingest`, `query`) — extend with the machine endpoints |
| `MemoryManager` / `store` | `memory/` | per-session memory; pairs with physis tenants |
| `knowledge` (kb/vault) | `knowledge/` | document corpus → a natural source for `lex`/`techne` reducers |
| agent loop + LLM providers | `agent/` | the operator that decides *when* to run a machine |

## Architecture

```
LLM agent (pika-hermes)
   │  invokes tools
   ▼
Machine tools  (tools/logic/chronos.rs, techne.rs, …)
   │  call
   ▼
PhysisClient (extended)  ──HTTP──▶  physis-pro-web
   classify · reconstruct · coherence/register · holarchy/edge · dream
```

- **Pure reducers stay in physis_pro** (`src/machines/*`, e.g. `chronos::detect_rhythms`).
  pika-hermes either calls them via HTTP results or, for CPU-only reducers, depends
  on `physis_pro` as a crate (it's a sibling path dep candidate). Default: HTTP, so
  the agent stays thin and language-agnostic.
- **State lives in physis** (one shared graph via the id bridge), keyed by the
  agent's session id → `X-Physis-User` tenant. Two agents never cross state.

## Step 1 — extend `PhysisClient`

Add the endpoints the machines need (mirror the engine's REST API + Bearer +
`X-Physis-User`):

```rust
impl PhysisClient {
    async fn classify(&self, text: &str) -> Result<Classification, String>;
    async fn reconstruct(&self, input: &str) -> Result<Vec<Neighbor>, String>;
    async fn register(&self, input: &str, user: &str) -> Result<String, String>; // node id
    async fn add_edge(&self, src: &str, dst: &str, weight: f32, user: &str) -> Result<(), String>;
    async fn dream(&self, user: &str) -> Result<Vec<Dream>, String>;
}
```
Auth: read `PHYSIS_API_TOKEN` (Bearer) + thread the session id as `X-Physis-User`.

## Step 2 — a `MachineTool` pattern

Each machine = one `BaseTool` (or a small family). Template:

```rust
// tools/logic/chronos.rs
pub struct ChronosTool { physis: Arc<PhysisClient> }
impl BaseTool for ChronosTool {
    fn name(&self) -> &str { "chronos" }
    fn description(&self) -> &str {
        "Find recurring rhythms in timestamped events; report what's due or slipping."
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("events", "array", "label+timestamp pairs", true),
             ToolParameter::new("horizon_days", "number", "lookahead window", false)]
    }
    fn execute(&self, params) -> ToolResult {
        // 1. reduce (detect_rhythms — local crate or a /machines/chronos endpoint)
        // 2. register each rhythm + edges via PhysisClient (shared graph)
        // 3. return upcoming/slipping as the tool result for the agent to act on
    }
}
```
Register in `ToolRegistry`. The agent now *decides* to run chronos ("am I slipping
on anything?") and acts on the result (e.g. via the existing
`google_calendar`/`messages`/`notifications` tools — a slipping rhythm → a reminder).

> Note: `BaseTool::execute` is sync in pika-hermes; the PhysisClient calls are
> async. Bridge with a small `block_on` helper or make the registry async — pick
> one convention up front (recommend: a shared Tokio handle + `block_on` in the
> tool, matching the existing client usage).

## Step 3 — roster → tools (priority)

| Machine | Tool(s) | Source in pika-hermes | Notes |
|---------|---------|------------------------|-------|
| **chronos** | `chronos` | events from memory/calendar tools | start here — reducer already in physis_pro, tested |
| **techne** | `techne_scan`, `techne_impact` | `knowledge` vault / a repo path | reuses `/scan` + capability classify |
| **soma** | `soma_log`, `soma_review` | tracker/wearable input | behavioural-vector coherence model exists |
| **lex** | `lex_obligations` | `knowledge` documents | sits on physis `ingest` |
| **elenchus** | `elenchus_argue` | conversation/memory | claim graph + ConstructiveRefutation |
| **harmony / polis** | later | audio / social feeds | reuse voice / social scoring |

## Step 4 — the operator (what makes it an agent, not a script)

The LLM agent already chooses tools. Give it a **machines system-prompt fragment**
describing each machine's job + when to invoke it, and let the agent run the
perceive→reduce→reflect loop opportunistically (e.g. on a daily tick: run chronos
+ dream, surface slips + echoes). The agent's memory + the physis tenant together
are the machine's "remember" beat.

## Dependencies / order

1. Extend `PhysisClient` (classify/reconstruct/register/add_edge/dream). **Blocks all.**
2. `MachineTool` async-bridge convention + `chronos` tool (vertical slice).
3. Wire chronos into the registry + a system-prompt fragment → first agentic machine.
4. techne / soma / lex as the corpus + tracker sources land.
5. Durable shared graph once Spanner 7C leaves scaffold (PREMORTEM P7).

## Open questions
- **Reducers: HTTP vs crate dep?** HTTP keeps pika-hermes thin; crate dep avoids a
  round-trip for pure CPU reducers. Recommend HTTP first, crate-dep only if latency bites.
- **Async tools.** Decide the sync/async bridge once (above) before adding tools.
- **Session ↔ tenant mapping.** One agent session = one `X-Physis-User`; persist the
  mapping in `memory/store` so a returning session re-attaches to its graph.
