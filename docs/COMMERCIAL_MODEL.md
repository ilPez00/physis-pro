# Commercial model and licensing

> Scope update, 2026-09-18: this document describes the industrial offer.
> The proposed agentic-workspace market, packaging experiments and sequencing
> live in the [workspace programme](plans/2026-09-18-agentic-workspace.md).
> Its prices are hypotheses, not published offers or changes to licence terms.

This document describes the proposed packaging. `LICENSE.txt` remains the
controlling legal agreement; pricing and packaging are commercial terms in the
signed order form.

> **Supersedes** the 4-tier SaaS / per-job metering in `BUSINESS.md` and the
> `pricing.html` page. Those describe a generic API platform this repo is not.
> This document (and `docs/plans/2026-08-30-productization.md`) is the canonical
> commercial model.

## Two products, one funnel

| Product | License | Buyer | Channel |
|---------|---------|-------|---------|
| **Core** (`physis-core`, Apache-2.0 per current manifest) | Apache-2.0 | developers / agents / MCP directories | crates.io, MCP registry, Smithery, llms.txt, agent-framework recipes |
| **Pro** (`physis_pro`, EULA) | commercial, offline Ed25519 key | SME manufacturers first, then regulated | **service-led pilots** → license/subscription |

Core is the free distribution funnel into Pro. Pro is the revenue. Core must
stay genuinely useful (not crippled) for the funnel to work — its "adoption"
goal is measured in agent-directory presence and long-tail Pro leads, not MRR.
See `docs/business/ADOPTION.md` for the zero-budget Core distribution plan.

## Revenue model

## 2026 monetary re-evaluation

Physis is worth more as a workflow product than as a generic AI API. Current
market references put OEE software broadly around $99–$800+ per machine/month,
with dedicated platforms often priced at $500–$5,000 per production line/year
before integration and hardware. Document-automation deployments can require
five-figure setup work even when ongoing software costs are hundreds per month.
See [Fabrico's 2026 OEE pricing guide](https://www.fabrico.io/blog/oee-software-pricing-guide-2026/),
[TeepTrak's OEE pricing/ROI guide](https://teeptrak.com/en/oee-software-price-roi/),
and [USmartTec's SMB document-automation guide](https://www.usmarttec.com/guides/ai-document-automation).

These are comparison points, not a claim that Physis has equivalent market
coverage today. The revised value model is:

| Product lane | First paid engagement | Recurring target | Value proof |
|---|---:|---:|---|
| Shift Loss Pilot | €2,500–€5,000 | €750–€2,000/plant/month | avoided loss, faster shift review, quantified scrap/downtime |
| Admin Review Pilot | €2,000–€6,000 | €500–€2,000/team/month | backlog hours removed, exceptions found, evidence delivered |
| Decision Planner | €5,000–€15,000 discovery | €1,000–€4,000/month add-on | avoided bad taxonomy/process change, controlled rollout |
| Private Edge / integrator | €10,000–€40,000 setup | €8,000–€30,000/year maintenance | data control, integration, SLA, multi-site reuse |

Use the lower end for the first reference customer and the upper end only when
Physis owns a recurring review process, has data coverage, and carries support
responsibility. The economic ceiling is the cost of recurring operational
mistakes and the staff time spent finding them—not the cost of an embedding.

### 1. Shift Loss Pilot

- €2,500–€5,000 one-time
- one plant, one week of data, up to two machines for the initial scope
- Italian HTML shift reports, loss review, and baseline/anomaly demonstration
- no promise of safety, guaranteed savings, or predictive accuracy without
  sufficient historical data

### 2. Plant Monitor

- €750–€2,000 per plant/month, priced by machine count and support level
- recurring reports, persisted history, anomaly/RUL monitoring, and operator
  support
- CSV/JSON ingestion first; MQTT/Modbus/OPC-UA are paid integration work until
  a common adapter is validated

### 3. Private Edge / Enterprise

- annual quote, typically a setup fee plus annual host/seat maintenance
- offline or on-premise deployment, signed licence keys, named hosts/seats,
  customer-controlled data, and optional integrator support
- custom ontology, ERP/MES integration, SLA, and audit exports are add-ons

These numbers are hypotheses for selling the first pilots. Discount the first
reference customer for speed, but do not make the pilot free unless the buyer
provides a named reference, usable data, and a time-boxed case study agreement.

## Licence tiers

| Tier | Intended use | Key entitlement |
|---|---|---|
| Evaluation | internal proof of value | time-limited, non-production, one host |
| Plant | one customer's internal plant operations | one plant, agreed hosts/seats, OEE/report/maintenance features |
| Enterprise Edge | multiple plants or air-gapped deployment | negotiated host/seat count, offline keys, support/SLA option |
| Integrator | implementation partner | written reseller/implementation addendum; no unapproved redistribution |

The current implementation already supports signed offline keys, expiry,
maintenance dates, seat counts, and feature slugs in `src/license.rs`. The next
commercial hardening is an order-form mapping from SKU to feature slugs and a
small vendor tool/process for issuing, renewing, and revoking keys.

## Hosted service position

The current EULA excludes operating Physis as a hosted service for third
parties. Keep that restriction for distributed binaries. If the product is
later offered as a managed service, use a separate hosted-service agreement
covering data processing, availability, subprocessors, retention, and support;
do not silently rely on the binary EULA.

## Non-negotiable trust language

Physis is decision support. It may identify statistical drift and project a
trend; it does not certify equipment safety, replace a qualified technician,
or guarantee remaining useful life. Reports must show data coverage and
assumptions, and the customer remains responsible for operational decisions.

## Saleability gates

Each gate must close before the tier above it is an honest claim. Order matters:
the entitlement/revocation work is days and unblocks Enterprise; the temporal
log is the audit differentiator that justifies the price; RBAC only becomes
blocking once multi-customer hosted service is attempted, which the service-led
model deliberately avoids first.

| Gate | Blocks | Status |
|------|--------|--------|
| SKU → feature-slug entitlement table + order form | any tier | ✅ Done (`ENTITLEMENTS` in `src/license.rs`; `physis-license list-skus`) |
| License `inspect`/`info` + trial-key generator | free → paid conversion | ✅ Done (`physis-pro license [--json]`; vendor `sign --trial`) |
| Key rotation + revocation | enterprise sales (lost keys, terminations) | ✅ Done (drop-in `revocations.json` enforced in `load()`; vendor `revoke --id`. Rotation out of scope — reissues every licence) |
| Real authn/RBAC (beyond the header token) | multi-customer hosted / Enterprise | partial (`src/rbac.rs` roles exist) |
| Append-only temporal/audit log | audit claims in the pitch | partial (causal ledger only) |
| On-prem single-binary distro + installer UX | air-gapped claim at scale | partial (Docker; installer untested end-to-end) |
| One measurable customer baseline | every marketing number | open (first pilot pending) |

## Commercial MVP backlog

- [x] SKU-to-feature entitlement table and renewal checklist.
- [x] License `inspect` / `info` and a 14-day trial-key generator.
- [x] Key rotation + revocation list, tied to the vendor tool.
- [ ] Evaluation-key issuance and a documented pilot flow.
- [ ] Customer/plant/host identity in report metadata.
- [ ] Durable shift history before multi-replica hosted deployment.
- [x] Rewrite `pricing.html` to the offer ladder here (Pilot → Industriale →
      Enterprise), not the SaaS tiers; route it with `/` and `/about`.
- [ ] Invoice/checkout process outside the engine first; automate only after
      three paying customers establish the pricing and tax workflow.
- [ ] Track pilot-to-recurring conversion, time-to-first-report, exception
      precision, and customer decision changes; these become pricing evidence.
