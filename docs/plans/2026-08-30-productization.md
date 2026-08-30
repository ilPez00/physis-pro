# Productization — Monetization v2 · Integration · UI/UX · Feature Testing

> 2026-08-30. Consolidates the findings from a full re-inspection of the codebase
> against the saleable product. This is the "turn the substrate into a business"
> plan. It supersedes the tiered-SaaS pricing in `BUSINESS.md` and the per-job
> billing in `PLAN.md`'s current-state table — both describe a product this repo
> is not: a generic API platform with metered jobs. What ships here is a
> service-led industrial offer plus a distribution funnel for the MIT core.

---

## 0. What this is, stated plainly (the grounding)

Physis Pro is a Rust runtime that reduces any input to one vector space,
classifies it against a 70-cell semiotic grid (~630 ontology entries), tracks
each thing's **coherence** (+1/0/−1), and replays failures in a background
"dream" loop to propose fixes. On top sits an industrial-backoffice layer:
OEE, predictive maintenance/RUL, SMA-30 inventory, document/mail/vault/history
ingest, a quality-feedback loop, and a space·time·cause causal ledger — exposed
as CLI + HTTP (98 routes, multi-tenant) + MCP + TUI + voice, license-gated by an
offline Ed25519 key.

The honest framing (already written in `docs/business/PRODUCT_RETHINK.md`): this
is a **brilliant substrate wearing the wrong coat**. The sellable product is
*adaptive industrial intelligence that runs air-gapped*, not "a coherence
engine" and not "a pay-per-job API".

---

## 1. Monetization v2

### 1.1 The two-product reality

The repo has been trying to be three things at once. Split them; do not merge:

| Product | License | Buyer | Channel | Goal |
|---------|---------|-------|---------|------|
| **Core** (`physis-core`, MIT) | MIT | developers / agents / MCP directories | crates.io, MCP registry, Smithery, llms.txt, CrewAI/LangGraph recipes | adoption + long-tail Pro leads |
| **Pro** (`physis_pro`, EULA) | commercial, Ed25519 offline key | Italian SME manufacturers first, then regulated | **service-led pilots** → license/subscription | first euros + recurring |

Core adoption is the *free* distribution funnel into Pro. Pro is the revenue.
Do not gate Core features behind Pro to "create urgency" — the funnel works
because Core is genuinely useful, not because it is crippled.

### 1.2 Offer ladder (first-euro sequence — from `docs/business/BUSINESS_PLAN.md`)

| Step | Offer | Price | What it proves |
|------|-------|-------|----------------|
| 0 | Free sample: one week of their CSV → shift report | €0 | engine produces output; email captured |
| 1 | **Shift Loss Review** (5 days, 1 week CSV → IT report + Pareto + anomaly review + 3 actions + 30-min call) | €1,500–2,500 | first euros; capture baseline metrics (review hours, downtime, scrap) |
| 2 | Second review cycle | €1,500–2,500 | repeat value |
| 3 | **Industriale** recurring (auto shift report, OEE+anomaly watch, 5 users) | €1,500/mo | recurring revenue |
| 4 | **Enterprise** (dedicated instance, custom ontologies, training, SLA) | €5,000/mo | margin + lock-in |
| 5 | Channel/OEM for MES vendors | per-seat | scale without services load |

Pricing discipline (from `GO_TO_MARKET_AND_EMAIL.md`): state the price without
attenuants. Discount the *first* reference customer for speed, but never make a
pilot free without a named reference + usable data + a case-study agreement.

### 1.3 What is dropped

- The 4-tier SaaS subscription ($0/$10/$50/Enterprise, per-job metering in
  `BUSINESS.md` and the current `pricing.html`). No RBAC, no billing backend, no
  hosted-service agreement — the SaaS model is premature and its pricing page is
  actively wrong.
- "Pay-per-job" as the lead message. Jobs are not the unit of value; avoided
  loss and review-hours-saved are.

### 1.4 Saleability gates — what must exist before each price tier is honest

| Gate | Blocks | Already done? |
|------|--------|---------------|
| SKU → feature-slug entitlement table | selling any tier | ✅ Done (`Tier::sku/from_sku/ALL` + `ENTITLEMENTS` in `src/license.rs`; `physis-license list-skus`; SKU surfaced in `/settings` + `physis-pro license`) |
| License `inspect`/`info` + trial-key generator | free → paid conversion | ✅ Done (`physis-pro license [--json]`, instant; vendor `sign --trial` mints 14-day TRIAL ids) |
| Key rotation + revocation | enterprise sales (lost keys, terminations) | ✅ Done for revocation (drop-in `revocations.json`, enforced in `load()`, vendor `revoke --id`). Rotation documented out of scope — rotating the vendor key reissues every licence |
| Real authn/RBAC (not just header token) | multi-customer hosted / Enterprise | partial (`src/rbac.rs` TokenTable + viewer/editor/admin roles exist; no hosted SaaS motion yet) |
| Append-only temporal event log | audit claims, "trust" layer | partial (causal ledger; not node/link mutation history) |
| On-prem single-binary distro + installer | air-gapped claim at scale | partial (Docker; no `install-physis` UX test) |
| One measurable customer baseline | every marketing number | open (first pilot pending) |

Order of work: the entitlement table + revocation are days of work and unblock
Enterprise pricing; the temporal log is the differentiator that justifies the
price; RBAC is a larger build that only becomes blocking once multi-customer
SaaS is attempted (which the service-led model deliberately avoids first).

### 1.5 The metric that matters

**First paid review → baseline metrics → recurring Industriale conversion.**
Track: contacts/week (≥5), reply rate (>10%), sample ≤48h, review conversion
(>25%), recurring conversion after 2 cycles (>50%), churn (<5%/mo).

---

## 2. Integration plan

### 2.1 One normalized event stream (the architectural step)

`docs/business/PRODUCT_RETHINK.md` calls it; `src/moonraker.rs` already does it.
Every source — Moonraker, MQTT/Modbus, mailbox, watch, scan, history, praxis —
must normalize to one typed, timestamped **`OperationalEvent`** with provenance,
rather than each being a bespoke special case inside the engine.

| Step | Work | Notes |
|------|------|-------|
| I1 | Define the canonical `OperationalEvent` (connector, mechanism, actor, timestamps, payload) | already partially in `ledger.rs` + `industrial.rs` |
| I2 | Adapters become `MachineAdapter`/`SourceAdapter` implementations emitting `OperationalEvent` | Moonraker done; MQTT/Modbus done; mail/scan/watch partially |
| I3 | One ingestion path into the holarchy | single `post_event` — the Phase 8b read-back contract |
| I4 | Provenance survives every projection | timeline, ledger, shift report, lineage all read the same event |

Success criterion: a new adapter (e.g. OPC-UA) is ~100 lines and inherits
dedup, anomaly detection, ledger recording, and timeline rendering for free.

### 2.2 Typed ontology (Palantir parity, additively)

Promote the substrate to a real typed property graph **without breaking** the
embed/coherence/dream engine (it only reads `.coherence` and `.embedding`):

| Step | Work |
|------|------|
| I5 | `CoherenceNode` gains `type: String`, `properties: Json` (default empty → back-compat) |
| I6 | `Edge { semantic_type: u32 }` becomes a named, schema'd link type + optional props + inverse traversal |
| I7 | 70-cell grid → object *types* (Machine, WorkOrder, SensorReading, Invoice), not just classification buckets |
| I8 | Dream outputs bind to typed actions ("open WorkOrder X", "schedule maintenance Y") instead of free text |

### 2.3 Temporal / event log

| Step | Work |
|------|------|
| I9 | Append-only log of node/link/verdict mutations (id, op, timestamp, actor) |
| I10 | Time-travel + audit view ("what did the graph look like when OEE dropped") |

This is the trust layer an industrial buyer pays for; without it the "audit
trail" claims in the pitch are not yet backed by a queryable log.

### 2.4 Outbound integrations (the recurring-value + lock-in layer)

| Step | Work |
|------|------|
| I11 | Dream countermeasures → webhook/CSV export (lowest build, immediate demo value) |
| I12 | ERP/MES/ticketing bindings (Jira, Slack, email) behind a stable adapter trait |
| I13 | Report delivery: email/PDF auto-send on shift close (Industriale tier) |

Order: I11 first (one endpoint, sells the "it tells you what to do next" story),
I12 only when a named customer names their system ("edge adapters wait for
customer" — existing key decision).

### 2.5 Agent surface (Phase 19)

**Thesis:** Physis is not "LangChain + Semantic Router" — it is the layer both
are missing: routing that learns, budget-bounded grounded retrieval, and memory
that carries verdicts. **Harness = the loop (out of repo); Physis = the
semantics, over MCP + HTTP.** This is the "device perceives, engine means"
doctrine (Phase 14 / 8b) extended to agents. Shipped P1–P5:

| Piece | What | Contract |
|-------|------|----------|
| P1 Route table | `AgentRoute { name, cell, action, examples }` as JSON data; classify → cell → bound action | `GET/POST/DELETE /api/v1/routes`, `POST /api/v1/routes/match`, MCP `physis_route`. Misroutes feed `QualityTracker` — Semantic Router cannot learn; this table can |
| P2 Structured output | schema-constrained slot extraction over the cascade, tools-first + JSON-prompt fallback, explicit `note` | `POST /api/v1/extract {text, schema}` |
| P3 Streaming + tools | `POST /api/v1/agent/run` → SSE `AgentEvent`s; `complete()` takes `ToolDefinition[]` | verified already shipped |
| P4 Thin SDK | zero-dep `clients/physis_client.ts` mirroring the MCP surface | fetch over the same endpoints |
| P5 Trace | quality-failure upstream trail joined to the causal ledger (±1h, tenant-scoped) | `GET /api/v1/trace?failure_id=` |

**Hard non-goals:** no LCEL/chains, no integration catalog, no vector-store
zoo, no prompt DSL. Composition stays data (the notebook-registry rule). A
DeepSeek-style harness should live in a separate repo with MCP as its contract,
so the air-gap story survives and the substrate stays clean.

---

## 3. UI/UX plan

### 3.1 The live surface is good; the customer surface is broken

The 10 product pages (Pulse, Notebook, Search, Atlas, Files, Studio, Console,
Agent, Study, Demo) share the Bioluminal Forge shell (`ui-shell.js` +
`physis.css`) and are in strong shape. The **customer-facing pages are orphans**:
`index.html`, `pricing.html`, `about.html`, `customer_dashboard.html`,
`api_tester.html` are not routed, use CSS classes that no longer exist
(`.hero`, `.pricing-grid`, `.sidebar`, `.nav-button`, …), and describe a
"pay-per-job API platform" that is not the product. `customer_dashboard.html`
has broken JS (`showSection` references an undefined `btn`) and calls endpoints
that do not exist (`/api/v1/my/account`, `/api/v1/my/usage`).

### 3.2 Refinement backlog

| # | Item | Notes |
|---|------|-------|
| U1 | Rewrite landing (`/`), pricing, about to the real model + design system | ship in this change |
| U2 | Delete or gut `customer_dashboard.html` — no billing backend exists to back it | convert to a license/deployment status page reading only real endpoints (`/health`, `/api/v1/embedder`) |
| U3 | Kill `api_tester.html` or fold it into the existing Agent/Studio | it duplicates the Notebook + Search affordances |
| U4 | Route the landing/pricing/about publicly; keep `/pulse` as the product home | `PRODUCT_PAGES` in `ui-shell.js` already treats Pulse as home |
| U5 | One 10-minute demo: `prospect.csv → shift report + ontology studio` in one command | the wedge that makes data legible in week 1 |
| U6 | A `--screenshot` mode or per-release capture pipeline | feeds the adoption track (visual distribution) |
| U7 | Audit each product page against the WCAG/offline rules already encoded in `physis.css` | the design system already documents its own floor; extend, don't regress |

Doctrine (from `physis.css` header): **density + glanceability beat decoration**.
The customer pages must obey the same rule — no hero banners, no marketing
framing, type and color carry hierarchy.

---

## 4. Feature testing plan

The engine is heavily unit-tested (759 lib tests, per-phase e2e). What is
missing is **cross-surface, funnel-shaped testing** — the things a customer
actually does across the whole product.

### 4.1 Coverage gaps

| Gap | Why it matters |
|-----|----------------|
| No test walks scan → classify → quality → dream → report in one flow | each phase tests in isolation; the customer walks the whole chain |
| No test for the customer-facing pages (they are not even routed) | the first impression is untested and broken |
| No integration test for a *new* adapter beyond MQTT/Modbus mocks | I4's "100 lines" claim is unverified |
| No test for temporal/audit (feature does not exist yet) | I9–I10 are greenfield |
| `tests/output_quality.rs` is `#[ignore]`d | the accuracy probe exists but is not in CI |

### 4.2 Test matrix to add

| # | Test | Layer | Gates |
|---|------|-------|-------|
| T1 | **End-to-end funnel**: scan dir → shift report → quality register → dream → `where` query → lineage | e2e | one test, the whole customer journey, tenant isolation throughout |
| T2 | **Adapter-contract test**: a stub adapter emitting `OperationalEvent` inherits dedup + anomaly + ledger | unit/e2e | verifies I4 |
| T3 | **Typed-object round-trip**: node with type+props survives persist/load/edit/delete | unit | verifies I5–I6 back-compat |
| T4 | **Temporal-log test**: mutation log replays to any past state; audit view correct | unit | verifies I9–I10 |
| T5 | **Marketing-page smoke**: `/`, `/pricing`, `/about` return 200 + contain expected copy, no external assets | http_surface | verifies U1/U4 |
| T6 | **Entitlement matrix**: each SKU enables exactly its feature slugs, cross-tier denial | unit | extends Phase 4.1 (already 9 tests) |
| T7 | **Offline/air-gapped test**: no model weights, no network → lexical path degrades honestly (no 500, no CDN fetch) | integration | the core moat, must be proven in CI |
| T8 | **Un-ignore `output_quality`** in a scheduled (not per-push) job | CI | keeps the accuracy floor from silently regressing |
| T9 | **Route-table contract** (P1): CRUD round-trip, validation-all-at-once, match→cell→action, tenant isolation | unit + e2e | 6 unit tests shipped; e2e pending |
| T10 | **Extract degradation** (P2): schema conformance with a provider; explicit `note` (never 500) without one | unit + e2e | 4 unit tests shipped; e2e pending |
| T11 | **Trace join** (P5): failure + ledger trail within window; unknown id → 404 | e2e | wired; test pending |

### 4.3 CI policy

- Per-push: `cargo check --features full`, clippy `-D warnings`, lib + e2e tests
  (already the gate; add T5/T6/T7 to it).
- Scheduled/daily: `output_quality` + accuracy floor under the real BGE embedder.
- Release: a fresh `PHYSIS_LICENSE` issued by the vendor key must gate the binary
  (already the `predeploy.sh` contract).

---

## 5. Sequencing (the 90-day order)

1. **Monetization unblocks** (entitlement table, revocation, license inspect,
   trial keygen) — days, unblocks every enterprise conversation.
2. **UI/UX honesty** (U1–U4) — the customer surface stops lying about the
   product; the landing/pricing/about tell the service-led story.
3. **Integration spine** (I1–I4 normalized event stream) — prerequisite for
   temporal + outbound; makes the demo feel like one engine.
4. **Temporal log** (I9–I10) — the audit/trust differentiator.
5. **Feature testing** (T1–T8) — lands alongside each of the above, not after.
6. **Typed ontology + outbound integrations** (I5–I8, I11–I13) — only once a
   named customer asks; "no adapter guessing" (existing key decision).

The rule throughout: **productization, not more research.** The engine is good;
the work is focus, a first paid pilot, and closing the gap between what the
code can do and what a prospect sees in the first five minutes.
