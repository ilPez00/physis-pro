# Physis Pro — Business Plan (2026)

> Canonical plan. Supersedes the strategy sections of `/BUSINESS.md` (kept for the
> original feature/pricing analysis). Doctrine sources: `GO_TO_MARKET_AND_EMAIL.md`
> (sell a measurable result before a platform), WEALTH_PLAN (first-euro doctrine),
> `PROSPECT_LIST_EXPANDED.csv` + `CLIENT_PITCHES.md` (pipeline).

---

## 1. Executive summary

Physis Pro is an offline-first industrial intelligence engine written in Rust: it
turns production exports (CSV/JSON logs, machine readings, documents) into Italian
shift reports with OEE decomposition, anomaly detection (σ-distance), remaining-useful-life
projection, and concrete countermeasures — plus ontology-driven classification of any
text/document against a 370+-domain grid, with a persistent quality-feedback loop.

**Business model:** service-led entry (paid 5-day Shift Loss Reviews) converting to a
license/subscription (Industriale €1,500/mo · Enterprise €5,000/mo), dual-licensed
(MIT crate + commercial EULA). Air-gap capability is the moat vs SaaS MES/dashboards.

**12-month goal:** 3 paid pilots → 2 recurring Industriale clients (~€36k ARR run-rate)
+ €6–10k one-time review revenue. Verona/NE-Italy SME manufacturing first.

## 2. Product & proof

| Asset | Status |
|---|---|
| Engine (Rust): OEE, anomalies, RUL, countermeasures, classify, quality loop | DONE — covered by revenue-critical industrial demo test |
| Sample shift report IT (HTML/PDF + JSON input) | DONE (`sample-shift-report.md/.html`, `demo-shift.json`) |
| Live API (Railway) + web console praxisweb.xyz | DONE |
| Public upload preview funnel (email-captured) | NOT BUILT — do not advertise uploads until retention/isolation live |
| crates.io publication (physis-core → physis_pro) | BLOCKED on physis-core publish |
| Real customer baseline metrics | NOT YET — capture during first pilot |

Honesty rule (from GO_TO_MARKET): the demo proves the engine produces the output;
it does not prove monetary value until a pilot measures review hours saved,
downtime avoided, scrap reduced.

## 3. Market & ICP

**Primary ICP:** Italian SME machine shops & foundries (10–200 employees), NE Italy
(Veneto/Trentino/Lombardia), CNC or pressofusione, double-shift, no dedicated MES or
MES-fatigued, owner/production-lead decides software purchases < €25k/yr.

**Secondary:** regulated manufacturers (aerospace/defence suppliers, medical device
contract mfg, pharma) where air-gapped operation + audit-trail quality loops beat cloud SaaS.

**Channel:** MES integrators/resellers (white-label embedding of the license-gated engine).

Why now: EU manufacturing cost pressure + Italian Industry-4.0 transition plans;
competitors are either web-only SaaS (disqualified in air-gapped/regulatory settings)
or heavyweight workflow engines (overkill for SMEs).

## 4. Offer ladder (first-euro sequence)

| Step | Offer | Price | Conversion goal |
|---|---|---|---|
| 0 | Free sample package (praxisweb.xyz/physis/sample) + free report on 1 week of data | €0 | Trust; email captured |
| 1 | **5-day Shift Loss Review** (1 week CSV → IT report, Pareto, anomaly review, 3 actions, 30-min call) | €1,500–2,500 upfront | First euros; measure baseline metrics |
| 2 | Second review cycle (another week/scope) | €1,500–2,500 | Proof of repeat value |
| 3 | **Piano Industriale** recurring monitoring (auto shift report email/PDF, OEE+anomaly watch, 5 users) | €1,500/mo | Recurring revenue |
| 4 | **Enterprise**: dedicated instance, custom ontologies, training, SLA | €5,000/mo | After 3+ months of value |
| 5 | Channel/OEM for MES vendors | per-seat license, negotiable | Scale without services load |

Price discipline: never apologize for price; state it without attenuants
(followup-generalmecannica.md rule).

## 5. Revenue model & projections

Recurring: Industriale €1,500/mo/client · Enterprise €5,000/mo/client.
Services: reviews €1.5–2.5k each; custom ontology workshop €1–3k one-time.
Crate licensing (crates.io MIT + EULA gate) is long-tail discovery, not near-term revenue.

Conservative Year-1 target (from today, not calendar):

| Quarter | Reviews sold | Recurring clients | Quarterly revenue |
|---|---|---|---|
| Q1 | 2 | 0 | €3–5k |
| Q2 | 2 | 1 | €4–6.5k |
| Q3 | 1 | 2 | €4.5–7k |
| Q4 | 1 | 3 (+1 Enterprise) | €9–13k |

Year-1 exit: ~€40–60k ARR-equivalent run-rate. Matches BUSINESS.md's conservative
path (Yr-1 $121k total incl. services is the optimistic bound; treat as stretch).

## 6. Sales process

1. **List work** — CLIENT_PITCHES.md (29 targets, tiered A–E, emails flagged
   confirmed/domain-guess; verify person/role before send).
2. **First touch** — Italian result-led email (template: prospect-research skill /
   outreach_moro_cbmec.md). Offer free report on one week of THEIR data.
   No attachments of customer files; sample link only.
3. **Sample delivery** — 48h turnaround on received CSV; limitation statement included.
4. **Review call** — 30 min; capture baseline metrics table (GO_TO_MARKET §metrics).
5. **Paid review quote** — €1,500–2,500, stated plainly.
6. **Convert to recurring** after two useful cycles.

Cadence rules: follow up once after 5 business days of silence; max 1 sollecito,
then close and move on. 5 new contacts/week minimum (WEALTH_PLAN P0).
Pipeline tracked in CLIENT_PITCHES.md status column.

## 7. Competition

| Competitor class | Weakness vs Physis |
|---|---|
| Cloud MES/SaaS dashboards | Web-only → disqualified air-gapped/regulatory; heavy rollout |
| Camunda/Flowable workflow engines | Overkill, network-dependent, no ontology/quality loop |
| Generic ML kits (sklearn/ONNX demos) | No domain grid, no shift-report artifact, no quality feedback persistence |
| Spreadsheet culture (the real incumbent) | Manual, inconsistent, tribal — this is who we actually replace first |

Positioning sentence: *"Non un altro cruscotto: un rapporto di turno leggibile generato
sui dati che già avete — senza installare nulla."*

## 8. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Data-format heterogeneity across shops | Free-sample step doubles as format discovery; per-client column mapping in onboarding workshop |
| Italian classification accuracy (86.7%) framed as weakness | Position as active workstream w/ regression suite; onboard-time ontology tuning raises precision on client lexicon |
| Long SME sales cycles | Service-first entry (small ticket, fast yes); max-1-followup rule keeps pipeline moving |
| Solo-founder delivery bandwidth | Cap at 2 concurrent pilots; reviews are 5-day bounded engagements |
| Upload-funnel abuse before controls | Do not advertise uploads until retention/isolation/email-capture live (already in GO_TO_MARKET) |
| physis-core publish dependency | Publish core first; all launch comms gated on it |

## 9. Milestones

- [ ] M0: physis-core → crates.io (unblocks everything public)
- [ ] M1: 20 verified contacts + first 10 sends (CLIENT_PITCHES.md)
- [ ] M2: Generalmeccanica decision forced (follow-up sent; 1 sollecito max)
- [ ] M3: FIRST PAID REVIEW (€1,500+) — first-euro milestone
- [ ] M4: Baseline metrics captured from real pilot (review hours, downtime, scrap)
- [ ] M5: First recurring Industriale conversion
- [ ] M6: Public launch wave (r/rust + Show HN + TWiR + Ben's Bites — DISTRIBUTION_LISTS.md)
- [ ] M7: One channel-partner LOI (Tier E list)

## 10. KPIs

Contacts contacted/wk (target 5) · reply rate (>10%) · samples delivered (≤48h SLA)
· review conversion from sample (>25%) · recurring conversion after 2 cycles (>50%)
· churn (<5%/mo) · MRR trend vs §5 table.

## 11. Immediate next actions

1. Set up gio@praxisweb.xyz mailbox (Zoho path, GO_TO_MARKET §business email).
2. Send Moro Srl + CB Meccanica drafts (already written) after contact verification.
3. Force Generalmeccanica decision (follow-up text ready).
4. Work Tier-A list top-down (CLIENT_PITCHES.md order).
