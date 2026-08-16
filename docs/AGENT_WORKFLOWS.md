# Physis as a context provider for coding agents

Physis packs a fixed token budget from a local corpus without calling an LLM.
That makes it useful to every agent that has a context window and a bill — which
is all of them. This document specifies how Physis attaches to each agent
runtime, what it exposes, and the workflows that justify the attachment.

Status legend: **built** — works today; **gap** — specified here, not yet
implemented.

---

## 1. Transport and tool surface

### 1.1 stdio transport — **built**

`physis-pro serve --stdio` serves MCP over stdin/stdout, which is what every
mainstream client speaks:

| Transport | Used by | State |
|---|---|---|
| stdio (child process, framed over stdin/stdout) | Claude Code, Codex, OpenCode, Zed, Cursor | **built** |
| Streamable HTTP / SSE | remote and hosted servers | gap |
| bare TCP `127.0.0.1:9876` | nothing | legacy, retained for the hosted deploy |

Verified by handshake: `initialize` → `tools/list` (19 tools) → `tools/call`.

**stdout belongs to the protocol.** Every diagnostic on this path — the licence
banner, the embedder provenance line, warnings — goes to stderr. A single
`println!` reachable from `serve_stdio` corrupts the stream, and the client
reports the server as broken with no usable diagnostic.

### 1.2 Tool coverage

| Capability | Module | Exposed over MCP? |
|---|---|---|
| Token-fixed packing with provenance | `src/pack.rs` | **yes — `physis_pack`** |
| Ontology classification | `src/classify.rs` | yes (`physis_classify`) |
| Directory scan | `src/scanner.rs` | yes (`physis_scan`) |
| Structural similarity | `src/structsim.rs` | gap — `physis_similar` |
| Quality/proof obligations | `src/quality.rs` | gap — `physis_check`, `physis_record` |

### 1.3 The embedder defect this uncovered

The MCP server previously constructed `RandomProjectionEmbedder` directly, so
every tool it served — classification included — answered from random
projections rather than semantics. It failed silently: a random projection
returns plausible-looking cosine scores for any query, so the output looked
like retrieval and was noise.

All entry points now resolve the embedder through
`crate::embed::select_embedder`, the self-test-gated cascade
(BGE → BGE-M3 → CLIP → MiniLM → random projection). If you add another entry
point, route it through that function.

---

## 2. The tool surface agents actually need

Five tools, named for what an agent wants rather than what the module is called.

### `physis_pack` — the flagship — **built**

Pack the highest-value context for a query into a hard token budget.

Root paths arrive from the agent, and an agent following a poisoned instruction
will ask for `/etc` or `../../..`. Set `PHYSIS_SCAN_ROOT` to confine packing;
`pack::resolve_root` canonicalises and rejects anything outside it.

```jsonc
// request
{
  "query": "how does tenant isolation work on the reconstruct path",
  "budget": 4000,          // hard ceiling, never exceeded
  "root": "./src",         // corpus scope
  "diversity": 0.3         // 0 = pure relevance, 1 = maximum spread
}

// response
{
  "context": "…assembled text…",
  "tokens": 3948,          // <= budget, always
  "chunks": [
    {"path": "src/web.rs", "lines": "2624-2701", "score": 0.81, "tokens": 612},
    {"path": "src/core.rs", "lines": "1101-1140", "score": 0.77, "tokens": 388}
  ],
  "dropped": 41            // candidates that did not fit
}
```

The `dropped` count is the sales pitch rendered as an integer: it is the context
the agent would otherwise have pulled in blind.

### `physis_map`

Return the ontology map of a directory — which cells the files fall into, which
cells are empty. This is the "show me the shape of this codebase" call an agent
makes once at session start.

### `physis_similar`

Given a file, a JSON blob, or a schema, return structurally similar ones via
`structsim`. The call an agent makes before writing a new module: *what does the
existing shape look like?*

### `physis_check`

Run the quality/proof-obligation checkers over a proposed change and return
unmet obligations. The PDCA "Check" phase, exposed as a tool so the agent can
self-verify before returning to the user.

### `physis_record`

Record an outcome (`success` / `failure` plus the cell) so `QualityTracker`
reweights future retrieval. This closes the PDCA loop across sessions and is
what makes Physis improve where a stateless retriever cannot.

---

## 3. Per-runtime attachment

### 3.1 Claude Code — MCP stdio (first-class)

`.mcp.json` in the project root:

```json
{
  "mcpServers": {
    "physis": {
      "command": "physis-pro",
      "args": ["serve", "--stdio"],
      "env": {
        "PHYSIS_LICENSE_FILE": "~/.physis/license.key",
        "PHYSIS_SCAN_ROOT": "."
      }
    }
  }
}
```

Best fit of the five: Claude Code already has a context-management problem worth
money, and MCP is native. This is the reference integration — build it first and
demo against it.

### 3.2 Codex CLI — MCP stdio

`~/.codex/config.toml`:

```toml
[mcp_servers.physis]
command = "physis-pro"
args = ["serve", "--stdio"]
env = { PHYSIS_LICENSE_FILE = "~/.physis/license.key" }
```

Same server, same tools. No Physis-side work beyond stdio.

### 3.3 OpenCode — MCP stdio

`opencode.json`:

```json
{
  "mcp": {
    "physis": {
      "type": "local",
      "command": ["physis-pro", "serve", "--stdio"],
      "enabled": true
    }
  }
}
```

Verify the exact key names against the OpenCode version in use before shipping
docs — its config schema has moved more than the others'.

### 3.4 Grok (xAI) — not MCP, needs an adapter

Grok exposes an OpenAI-compatible function-calling API, not an MCP client. The
attachment is an adapter that translates the tool surface into function
definitions and runs the loop:

```
physis-pro agent-bridge --provider xai --model grok-<version>
```

The bridge already has most of what it needs in `src/ai/provider.rs` and
`src/ai/tools.rs`. Work is mapping the five tools above into function schemas
and handling the tool-call round trip.

**Note:** sending code to the xAI API contradicts the air-gapped pitch. Position
this adapter for the cost-reduction customer, never for the defence or pharma
customer. Do not put it in the same datasheet.

### 3.5 Hermes — a hosted model, not a client

Hermes is a tool-calling model you run yourself (llama.cpp, vLLM, Ollama). The
integration is the same adapter as Grok, pointed at a local OpenAI-compatible
endpoint:

```
physis-pro agent-bridge --provider openai-compatible \
    --base-url http://localhost:8000/v1 --model hermes-<version>
```

This is the **fully local stack**: Hermes for generation, Physis for retrieval,
nothing leaving the host. It is the configuration to demo at Evotec and Leonardo,
because it is the only one that survives their network policy.

| Runtime | Protocol | Physis work needed | Air-gap safe |
|---|---|---|---|
| Claude Code | MCP stdio | stdio transport | no (model is remote) |
| Codex | MCP stdio | none beyond stdio | no |
| OpenCode | MCP stdio | none beyond stdio | depends on model |
| Grok | xAI function calling | adapter | **no** |
| Hermes (self-hosted) | OpenAI-compatible | adapter | **yes** |

---

## 4. Mock workflows

These are the demos. Each one should become a recorded terminal session before
any sales call.

### W1 — Cold-start orientation

*An agent joins a repository it has never seen.*

1. Agent calls `physis_map` on the repo root.
2. Physis returns ontology cells with file counts and names the empty cells.
3. Agent calls `physis_pack` with the user's task and a 6k budget.
4. Agent begins work with a bounded, relevant context.

**Baseline it replaces:** the agent greps around for several turns, pulling
whole files, spending 30–60k tokens to reach the same understanding.

**Metric to record:** tokens to first correct edit, Physis vs. baseline.

### W2 — Budgeted implementation

*A task spanning several modules, under a hard context ceiling.*

1. `physis_pack` with `budget: 8000`.
2. Agent edits.
3. `physis_similar` on the new file to check it matches existing shape.
4. `physis_check` for unmet obligations.
5. `physis_record` with the outcome.

**Point demonstrated:** the context never exceeds budget, so cost per task is
predictable in advance. For a customer running hundreds of agent tasks a day,
predictability is worth more than the savings.

### W3 — Air-gapped review (the enterprise demo)

*Hermes local + Physis local, network cable physically unplugged.*

1. Disconnect the network on camera.
2. `physis_pack` over the internal corpus.
3. Hermes generates a review from the packed context.
4. `physis_check` verifies obligations.

**Point demonstrated:** the entire loop runs with no egress. This is the demo
that closes Evotec and Leonardo, and no competitor in the comparison table can
run it.

### W4 — Cross-repository structural transfer

*"We implemented this in repo A; do the same in repo B."*

1. `physis_similar` with the reference schema from repo A.
2. Physis returns node-level correspondences in repo B, including the fields
   that have no counterpart.
3. Agent implements against the gap list rather than guessing.

**Point demonstrated:** `structsim` does something no keyword retriever can. The
unmatched-node list is the deliverable.

### W5 — Learning loop across sessions

*The same class of task, three weeks apart.*

1. Week 1: task fails, `physis_record` logs the failure and cell.
2. `QualityTracker` down-weights that cell.
3. Week 3: `physis_pack` for a similar query returns a different, better set.

**Point demonstrated:** retrieval improves with use, without retraining and
without sending anything to a vendor. Requires a persisted tracker across
sessions — verify `QualityTracker::save`/`load` is wired into the MCP path.

---

## 5. Build order

1. ~~stdio transport~~ — **done**.
2. ~~`physis_pack`~~ — **done**. Still the benchmark subject.
3. **W1 + W2 recorded** against Claude Code. ← next
4. **`physis_map`, `physis_similar`** — the differentiating pair.
5. **Hermes adapter + W3** — the enterprise demo.
6. **`physis_check`, `physis_record`, W5** — the retention story.
7. Codex and OpenCode config docs — near-zero work once stdio exists.
8. Grok adapter last: it serves the segment that pays least and contradicts the
   positioning that pays most.

---

## 6. Licensing interaction

Each MCP tool call runs inside a licensed process; the gate in `src/license.rs`
runs once at startup. Feature slugs map to tools:

| Tool | Required feature |
|---|---|
| `physis_pack`, `physis_map` | `rag` |
| `physis_similar` | `rag` |
| `physis_check`, `physis_record` | `quality` |
| all MCP access | `mcp` |

An unlicensed host fails at spawn, so the agent reports the server as
unavailable rather than silently returning degraded context. That is the correct
failure mode: silent degradation would be blamed on retrieval quality, not on
the licence.
