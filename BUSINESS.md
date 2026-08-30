# Physis-Pro Business Plan

> **Superseded (2026-08-30).** The 4-tier SaaS subscription and per-job
> metering below describe a generic API platform, not the product. The canonical
> commercial model is now `docs/COMMERCIAL_MODEL.md` (service-led pilots →
> Industriale/Enterprise licensing) and `docs/plans/2026-08-30-productization.md`
> (integration/UI-UX/testing + the Pro/Core split). This file is kept only for
> the original feature-analysis tables.

## Vision
Physis-Pro: Industrial backoffice intelligence that runs air-gapped. Knowledge management, process monitoring, and quality feedback for regulated environments where network connectivity is a liability, not an asset.

## Problem
Regulated industries (defense, pharma, banking, manufacturing) have three interlocking problems:
1. **No network access** — air-gapped environments disqualify SaaS, web-based tools
2. **Knowledge fragmentation** — scattered docs, tribal knowledge, no centralized ontology
3. **Quality blindness** — failures are recorded informally, if at all; no feedback loop

Existing tools are either:
- **Web-only** (can't run offline)
- **Generic ML kits** (no ontology, no quality loop, no domain specificity)
- **Heavyweight workflow engines** (Camunda/Flowable — overkill, network-dependent)

There is no offline-first, ontology-driven, quality-feedback loop tool for industrial backoffice work.

## Problem Worth Solving (Quantified)
| Vertical | Pain Cost | Why physis-Pro |
|---|---|---|
| Defense/Intel | Classified data cannot leave facility; classification accuracy matters | Air-gapped, ontology-driven, no network |
| Pharma/Regulated | FDA audit failures cost $5M+ per incident; quality documentation is manual | Quality feedback loop, persistent embeddings, audit trail |
| Banking/Finance | Compliance violations: $10M+ avg; document processing is manual | Document ingest, quality tracking, ontological grid |
| Manufacturing | OEE losses 5-20%; RUL unknown; inventory mismatches | Process monitoring, SMA-30 forecast, inventory tracking |
| Legal/Compliance | Regulatory document mismatches; statute of limitations risk | Knowledge vault, semiotic editing, provenance tracking |

## Solution
Physis-Pro is an offline-first industrial backoffice engine with:
- **Ontology-driven classification** — 370+ domains (54 human + 107 machine + 298 custom)
- **Quality feedback loop** — `report_failure()`/`report_success()` persist 384-dim embeddings to `~/.physis/quality_failures.json`
- **File theme graph** — BM25 + ontology domains → interactive browsing & wiki export
- **Semiotic editing** — `node-edit`/`node-delete` preserve structure (id/verdict/edges/provenance)
- **Multi-tenancy** — per-user isolated state via `X-Physis-User` header; shared infra global
- **License-gated** — Ed25519 offline verification; no network required
- **Watch / auto-ingest** — `PHYSIS_WATCH_DIR` background thread registers changed files
- **Vault / History / Praxis import** — Obsidian/markdown, browser bookmarks/history, chat JSONL → labeled core nodes
- **SMA-30 inventory forecasting** — with cooldown-regulated order suggestions
- **Facet queries** — filter by lifecycle, agency, scale, abstraction, sub-domain, sub-mode

## Monetization Strategy

### Recommended: Dual Licensing + Subscription Tiers

#### Tier 1: Base (Free / Evaluation)
- Features: `cli, voice, web, tui`
- Price: $0
- Target: Individual users, hobbyists, evaluation
- Conversion goal: 2-5% → paid tier

#### Tier 2: Industrial ($49/user/mo)
- Adds: `industrial` (MQTT/Modbus process monitoring, OEE, RUL)
- Target: Manufacturing, logistics, field services
- Differentiator: Process monitoring that works offline

#### Tier 3: Ingest ($29/user/mo)
- Adds: `ingest` (PDF text extraction, email parsing, wiki-embedded)
- Target: Document-heavy organizations, legal, compliance
- Differentiator: Real PDF/text extraction offline

#### Tier 4: RAG ($19/user/mo)
- Adds: `rag` (token-fixed context retrieval, budget-bounded search)
- Target: Knowledge workers, AI-assisted research
- Differentiator: Deterministic, overflow-proof context

#### Tier 4: Full ($149/user/mo)
- Features: `["*"]` all features enabled
- Target: Enterprises, large-scale deployment
- Differentiator: Bundle discount vs. à la carte

#### Phase 3+: Enterprise Support
- Support plans: $500/mo (email) → $3K/mo (24/7 hotline)
- Implementation services: $25K-100K one-time
- Custom feature development: $150/hr

### Revenue Projections (3-Year)
| Metric | Year 1 | Year 2 | Year 3 |
|---|---|---|---|
| Free users | 5,000 | 15,000 | 40,000 |
| Paid subscriptions | 120 | 350 | 900 |
| MRR | $5,900 | $17,900 | $54,900 |
| ARR | $71K | $215K | $659K |
| Services revenue | $50K | $80K | $120K |
| **Total revenue** | **$121K** | **$295K** | **$779K** |

### Key Success Metrics
| Metric | Target | Measurement |
|---|---|---|
| Free-to-paid conversion | 2-5% | Trials → paid |
| Monthly churn | <5% | Subscription cancellations |
| CAC | <$200 | Marketing spend / new customers |
| LTV:CAC ratio | >5x | Lifetime value / CAC |
| Feature adoption | >30% | % of paid users using each feature |
| NPS | >40 | Customer surveys |
| Trial-to-paid | 15-25% | Trials → paid |

## How to Make physis-Pro Maximally Valuable

### 1. Hard Seat Enforcement (IN PROGRESS)
- Track concurrent seat usage via `~/.physis/.seats_counter`
- If `seats_used >= license.seats`, exit with code 2 + message
- Currently best-effort (file-based), can be upgraded to sled DB + license server later
- **Impact:** Turns license from "nagware" into genuine access control

### 2. Key Rotation & Revocation
- Add revocation list stored in sled database
- `physis-license revoke --id <id>` to mark licenses invalid
- Allow key rotation by accepting `PHYSIS_LICENSE_PUBKEY` overrides at runtime
- **Impact:** Critical for enterprise sales — lost/stolen keys, contract terminations, compliance

### 3. SKU Feature Matrix & Pricing (IN PROGRESS)
- Define concrete SKUs with clear feature boundaries
- Base/Industrial/Ingest/RAG/Full pricing tiers
- Clear upgrade paths with ΔPrice
- **Impact:** Monetization clarity, customers know what they're buying

### 3. ONNX Model Management
- Add `physis-pro download-models --model bge-base` (with fallback to random-projection)
- Add model version pinning and checksum verification
- Add `physis-pro models status` endpoint
- **Impact:** Production deployments don't require manual model placement

### 4. TUI Polish & Net Workspace
- Add per-tenant color schemes in TUI
- Net workspace integration with live status age indicators
- Keyboard shortcuts handbook
- **Impact:** Usability for power users who spend all day in the TUI

### 5. Vault/History/Praxis Import Quality
- Add `physis-pro validate-import` to scan imported nodes for ontological consistency
- Add confidence scores for imported data
- Add "conflict resolution" UI for duplicate labels
- **Impact:** Higher data quality, reduces "garbage in" problems

### 6. Quality Feedback Loop Analytics
- Add `physis-pro quality stats` — top failure modes, most penalized cells, trend over time
- Add web endpoint `GET /api/v1/quality/stats`
- Add exported JSON for BI tools
- **Impact:** Actionable intelligence for customers — "why are our scores dropping?"

### 6. Docker/OCI Optimized Build
- Add Dockerfile with multi-stage build
- Add `.dockerignore`
- Add `physis-pro docker-build` script
- **Impact:** Easy deployment, CI/CD integration

### 7. Multi-Language Documentation
- Add i18n to README (Spanish, German for industrial markets)
- Add Chinese/Japanese technical docs for Asian OEM partnerships
- **Impact:** Market expansion, particularly in non-English industrial sectors

### 7. Quality Feedback Loop → Revenue Loop
- Customers see direct ROI: fewer failures, better compliance, lower audit costs
- Add ROI calculator on website: "How much could you save with quality tracking?"
- Add case studies with real numbers: "PharmaCo reduced audit prep time by 70%"
- **Impact:** Clear business case, easier sales conversations

### 8. Partnerships & ISV Program
- Partner with ontology vendors (PoolParty, OntologyHub)
- Partner with AI model providers (swap embedders)
- Partner with integration consultants (MQTT/Modbus shops)
- ISV program: 10% referral commission on first-year revenue
- **Impact:** Channel revenue, ecosystem growth, faster market penetration

### 9. Compliance & Certifications
- SOC 2 Type II readiness documentation
- FedRAMP baseline (defense pipeline)
- ISO 27001 documentation
- FDCC (Federal Desktop Core Configuration) compliance
- **Impact:** Opens government/defense contracts, enterprise buyers require it

### 8. The "Killer Feature" Differentiators
What makes physis-Pro uniquely valuable vs. alternatives:

| Differentiator | Why It Matters |
|---|---|
| **Air-gapped by design** | Defense/pharma/banke will *never* use web-only tools |
| **Ontology-driven** (370+ domains) | Generic ML can't match domain specificity out-of-the-box |
| **Quality feedback loop** | Most tools track nothing; physis persistence changes behavior |
| **Semiotic editing** | Preserve structure through edits — no other tool does this |
| **Watch/auto-ingest** | Background thread that auto-registers changed files |
| **Vault/history/praxis import** | Three knowledge ingestion paths, not just one |
| **SMA-30 inventory** | Forecast + cooldown-regulated orders, unique to this codebase |

### Implementation Priority (Next 90 Days)

| Priority | Feature | Effort | Revenue Impact |
|---|---|---|---|
| **1-Hard** | Hard seat enforcement (completed) | 1 day | Prevents over-usage |
| **2-High** | Key rotation & revocation | 3-4 days | Enterprise sales enabler |
| **3-High** | SKU pricing page + Stripe billing | 2-3 days | Immediate revenue |
| **4-Med** | Trial/demo license generator | 2 days | Free-to-paid conversion |
| **5-Med** | Quality stats endpoint | 3 days | Upsell/expansion |
| **6-Med** | Docker/OCI build | 1 day | Deployment ease |
| **7-Low** | Multi-language docs | 1-2 weeks | Market expansion |
| **8-Low** | Partnerships & ISV program | 2-3 weeks | Channel revenue |

### The "North Star" Metric
**Paid active users × Feature adoption rate × Net promoter score**

This single metric captures:
- How many paying users you have
- How much of the product they use
- Whether they'd recommend it to others

Track this weekly, and all other business metrics should trend positively.

## Immediate Next Actions (This Week)

1. Publish SKU pricing page on website
2. Add `physis-pro license inspect` + `physis-pro license info` endpoints
3. Create 30-day trial license generator
4. Set up Stripe billing account (Business tier)
5. Create 3 case studies (manufacturing, legal, pharma)
6. Add ROI calculator mockup on website
7. Add partner program setup (10% referral commission)
8. Add SOC 2 documentation skeleton
9. Add Dockerfile for OCI build
10. Add keyboard shortcuts documentation for TUI

## Long-Term Vision (3-5 Years)
- 1,000+ paying customers across 5 verticals
- $2M+ ARR
- Category creator: "the offline ontology engine for regulated industries"
- Profitable without venture capital
- Platform for vertical-specific extensions (healthcare, energy, education)

The key insight: physis-Pro's value comes from its **constraints** — air-gapped, ontology-driven, quality-aware, semiotic editing. These are not bugs to be "fixed away" but features that define the market position. The business model should lean into these constraints, not try to make physis-Pro "like everything else."