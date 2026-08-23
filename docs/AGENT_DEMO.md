# Agent Demo — Inside vs Outside the Harness

The agent surface is one ReAct loop (`src/ai/agent.rs::run_agent_stream`)
served two ways:

| Path | Entry | Trace |
|------|-------|-------|
| **Inside** the harness | `POST /api/v1/agent/run` (SSE), dashboard `/agent` | live event stream, answer judged by the coherence gate before display |
| **Outside** the harness | library call `physis_pro::ai::agent::run_agent_stream` with a hand-built `ToolRegistry` | `EventSink` callback, no HTTP |

Both demonstrations are executable tests in
`tests/http_surface.rs` (`agent_demo_*`). They need no API keys: a scripted
OpenAI-compatible mock is served on 127.0.0.1 and registered as the priority-0
provider via `EP_ORCHESTRATOR`.

Reproduce the traces:

```
cargo test --test http_surface agent_demo -- --nocapture
```

## Demo 1 — inside the harness (SSE)

Request (tenant-scoped; runs are isolated per `X-Physis-User`):

```json
POST /api/v1/agent/run
{"prompt": "Classify this very prompt against the semiotic grid.",
 "tools": ["classify"], "mode": "ANALYZE"}
```

Captured SSE trace (abridged; verdict JSON shortened):

```
event=tool_call   classify → {"text":"Classify this very prompt against the semiotic grid."}
event=tool_result HEAL × SENSE : 1.000
                  FABRICATE × MOVE : 0.995
                  FABRICATE × BRAINSTORM : 0.994
                  CONSTRUCT × CREATE : 0.990
                  HEAL × GUIDE : 0.988
event=plan        The prompt lands in the HUMAN × OPERATE cell: it directs an agent to act
                  on the semiotic grid, which is precisely what the classify result above recorded.
event=final       The prompt lands in the HUMAN × OPERATE cell: …
event=verdict     {"verdict":"APPROVED","composite_fitness":0.72,"top_cell":"HEAL × GUIDE",
                   "human_readable_summary":"Physis evaluated output against cell [HEAL × GUIDE]
                   with composite fitness 0.72 (threshold 0.60). Verdict: APPROVED."}
event=done        run-3c608480-02bd-47d1-baa8-e0c845dd6971

gate verdict=APPROVED fitness=0.720
```

What this demonstrates beyond "the loop works":

- The tool executed against **real engine state**: those cells come from the
  loaded ontology classifier, not from the mock.
- The **coherence gate ran after the loop**: the final answer was embedded,
  placed on the semiotic grid (top cell `HEAL × GUIDE`), scored for fitness and
  contradictions, and emitted as its own `verdict` frame the dashboard renders
  beside the answer.
- Ordering is contractual: `tool_call < tool_result < final < done`, asserted
  by the test.

## Demo 2 — outside the harness (direct library call)

Same loop, no router, no SSE — a consumer embedding physis-pro as a crate
would drive it:

```rust
let mut tools = ToolRegistry::new();
tools.clear(); // drop network/fs builtins
tools.register("get_time", "Returns the current UTC timestamp.", …schema…,
    Box::new(|_| Box::pin(async { Ok(chrono::Utc::now().to_rfc3339()) })));

let sink: EventSink = Arc::new(move |ev| println!("[r{}] {} {}", ev.round, ev.kind, ev.text));

let out = run_agent_stream(&cascade, &tools, &AgentConfig::default(), &[],
    "What time is it right now? Use the get_time tool.",
    None, "", None, &sink).await?;
// AgentOutput { provider: "orchestrator", tool_rounds: 1, tokens_used: 64, … }
```

Captured sink trace:

```
r1 tool_call   get_time → {}
r1 tool_result 2026-08-23T17:36:05.090496923+00:00
r1 plan        The current UTC timestamp is the one the get_time tool returned.
r1 final       The current UTC timestamp is the one the get_time tool returned.
summary: provider=orchestrator rounds=1 tokens=64
```

## Notes for operators / integrators

- **Provider injection**: both paths take providers from the cascade built by
  `ProviderCascade::from_env()`. `EP_ORCHESTRATOR=<base_url>` registers an
  OpenAI-compatible endpoint as priority-0 (no key required) — that is how the
  demos stay offline, and how a self-hosted orchestrator plugs in.
- **Tool whitelist**: `"tools": [...]` restricts the registry per request;
  empty means all curated tools (`graph_search`, `node_search`, `classify`,
  `coherence_register`, `coherence_edit`, `dream`, plus `opencode` when its
  binary is configured).
- **OpenCode delegation**: the `opencode` tool shells out to
  `agent.opencode_bin` as a subprocess, sandboxed to the scan root
  (`resolve_workdir` refuses `..`, symlinks pointing outward, and missing
  dirs); abort kills the child.
- **Abort**: `POST /api/v1/agent/abort {"run_id": …}` — only the tenant that
  started a run may abort it.
