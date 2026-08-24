# Physis Pro — Per-Client Pitches & Pipeline

> Companion to `BUSINESS_PLAN.md`. One row per prospect in `PROSPECT_LIST_EXPANDED.csv`.
> Status values: not_contacted → sent → sample_delivered → review_quoted → pilot → recurring / dead.
> Email confidence: `confirmed` = verbatim in public snippet (still verify the PERSON before send);
> `domain-guess` = inferred, verify on site before sending.
> Update this table after every send. Max 1 follow-up after 5 business days of silence.

## Pipeline summary

| Status | Count | Companies |
|---|---|---|
| proposal_in_progress | 1 | Generalmeccanica |
| not_contacted | 28 | all below |

## Master pipeline table

| Company | Tier | Vertical | Email | Confidence | Particular needs / tuning | Status |
|---|---|---|---|---|---|---|
| Generalmeccanica | ? | Process facilitator | *(recover from proposal recipient — none saved)* | n/a | Multi-client facilitation; needs Polis team-alignment + ticket triage (MAINTAIN/MOVE/PLAN); scale their service, don't replace it | proposal_in_progress |
| Moro Srl | A | Torneria CNC conto terzi, Padova | info@morosrltornerie.com | confirmed | Single-site turning shop; standard OEE/shift-report pitch; no MES — CSV export path only. Draft ready (outreach_moro_cbmec.md) | not_contacted |
| MG Tecno Meccanica | A | Officina meccanica precisione, Veneto | info@mgtecnomeccanica.com | confirmed | Precision subcontracting; likely multi-machine small batches → micro-fermate & setup-loss Pareto angle | not_contacted |
| Meccanica Marcato | A | Tornitura/fresatura, Padova | info@meccanicamarcato.it | confirmed | Turning+milling mix; pitch loss-decomposition across two workstreams; tribal-knowledge capture via quality loop | not_contacted |
| Officine Cioetto | A | CNC 33 machines double-shift | info@officinecioetto.it | confirmed | Scale case: 33 CNC × 2 shifts = report volume stress-test; pitch per-reparto comparison ("quale macchina ha perso più tempo"). Validation-partner framing (free for feedback) | not_contacted |
| BMech | A | CNC precision machining (fresatura+tornitura) | info@bmech.it | confirmed | Smaller workshop; automation components; keep pitch minimal — one-week CSV free report. Draft ready (GO_TO_MARKET §drafts) | not_contacted |
| FILA Srl | A | CNC/casting prod+quality+admin | commerciale@filasrl.it | confirmed | Cross-functional (production+quality+admin) → office ontology + doc ingest angle on top of shift reports; quality-loop for audit trail | not_contacted |
| CB Meccanica | B | Aeronautics & defence parts | info@cbmeccanica.it | confirmed | Regulated traceability: air-gap story + provenance/audit trail; quality feedback loop as AS9100/ISO evidence support. NO cloud claims. Draft ready (outreach_moro_cbmec.md) | not_contacted |
| DFM | B | CNC aerospace/defence MES-integrated | info@dfm.srl | confirmed | HAS MES already → "sopra i vostri export MES, non un altro cruscotto"; classification layer over MES data; do NOT pitch data collection | not_contacted |
| Steel Lavorazioni Meccaniche | B | Aerospace precision machining | info@steel-lm.it | domain-guess | Same regulated angle as CB; verify email on site first | not_contacted |
| Aerosystems | B | CNC machining + assembly aerospace | info@aerosystems.it | domain-guess | Assembly+aerospace: mixed production/assembly classification (CONSTRUCT vs FABRICATE) is the differentiator; verify contact | not_contacted |
| SOPHIA High Tech | B | Aerospace/defence prototypes | info@sophiahightech.com | domain-guess | Prototypes = high-mix/low-volume; standard OEE weak → pitch anomaly detection + RUL on prototype cells instead; verify contact | not_contacted |
| OFI (Officina Farmaceutica) | C | Contract mfg medical devices | info@ofi.it | domain-guess | Pharma-regulated: air-gapped deployment + document ingest (batch records); longest sales cycle — deprioritize until 2 pilots exist | not_contacted |
| Euleria Srl | C | Medical/health manufacturing | info@euleria.health | domain-guess | Medical device mfg; quality-system documentation angle; verify fit + contact | not_contacted |
| FarmaImpesa Group | C | Medical devices/integrators | info@farmaimpresa.com | domain-guess | Integrator potential (channel) more than end user — probe both angles; verify contact | not_contacted |
| Italpres | D | Alluminio pressofuso + officina, Brescia | italpres@italpres.com | confirmed | Foundry: scrap/yield angle FIRST (pressofusione margins die on scrap), OEE second; thermal-anomaly (4.4σ) demo maps directly to furnace/die temp monitoring | not_contacted |
| Almeco Group | D | Lavorazione alluminio conto terzi, Bergamo | info@almecogroup.com | domain-guess | Aluminum subcontractor group; scrap/yield + multi-impianto comparison; verify contact | not_contacted |
| Fonderia SL | D | Pressofusione alluminio | info@fonderiasl.it | domain-guess | Foundry pitch template (scrap-first); verify contact | not_contacted |
| Fonderia FAR | D | Pressofuso multi-settore | info@fonderia-far.com | domain-guess | Foundry pitch template; multi-sector = varied scrap causes → loss-Pareto hook; verify contact | not_contacted |
| Fonderie Pandolfo | D | Billette alluminio quality sys | info@fonderiepandolfo.com | domain-guess | Already has quality system → position as upgrade feeding their existing QMS, not replacement; SPC/yield ontology fit | not_contacted |
| OLMA | E | CNC Industry4.0/OEE reseller | contact form only | form | CHANNEL partner: they sell to our ICP; pitch white-label/license embedding (license-gated engine), not direct use | not_contacted |
| rsAutomazione (rsMES) | E | MES reseller CNC data | info@rsautomazione.com | domain-guess* | Channel: white-label reporting layer on top of rsMES installs; *fix typo'd email (domain-guest→guess) after verification | not_contacted |
| ISIPC | E | MES software manufacturing | info@isipc.it | confirmed | Channel/OEM: license the engine as ISIPC-branded module; needs seat-enforcement hardening first (BUSINESS.md #1) | not_contacted |
| Campi SRL | E | CNC-MES interconnection SI, Piemonte | campi@campisrl.net | confirmed | Channel: they interconnect CNC↔MES → natural embedder of reporting/classification engine; Piemonte = outside core region, channel-only | not_contacted |
| AEC Soluzioni (jpiano MES) | E | MES vendor | info@aecsoluzioni.it | domain-guess | Channel: OEM licensing of physis_pro as add-on module to jpiano MES; verify contact | not_contacted |
| Mic-Mec Srl | A | Officina conto terzi, Treviso | info@micmecsrl.it | domain-guess | Standard Tier-A pitch; VERIFY EMAIL on site before send | not_contacted |
| CMS Meccanica | A | CNC progettazione, Treviso/PD/VE | info@cmsmeccanica.it | domain-guess | Design+CNC mix → office-ontology (progettazione flows) + production reports; VERIFY EMAIL | not_contacted |
| Mec-Point Srl | A | Lavorazioni meccaniche precisione | info@mec-point.srl | domain-guess | Standard Tier-A pitch; VERIFY EMAIL | not_contacted |
| Unispecial | A | Torneria automatica conto terzi, Padova | info@unispecial.it | domain-guess | Automatic lathes = high-volume small parts → cycle-time & micro-stoppage analytics lead; 30+ yr firm, conservative buyer; VERIFY EMAIL | not_contacted |

\* rsAutomazione email listed as `domain-guest` in CSV — typo; treat as unverified.

---

## Pitch variants by need (tuning map)

### V1 — Standard machine shop (Tier A default): **"Rapporto di Turno"**
Hook: send one week of CSV → Italian report with OEE, top losses, fermi, anomalies,
next-shift actions. Free sample, €1,500–2,500 paid review after.
Tuning needed: column mapping to their export format (1–2h in onboarding).
Targets: Moro, BMech, MG Tecno, Marcato, Mic-Mec, Mec-Point, CMS.

### V2 — High-volume/high-mix (Cioetto, Unispecial): **micro-fermate & cycle time**
Standard OEE hides repeated 2-minute stops. Lead with micro-stoppage clustering +
cycle-time drift per machine/part family. Tuning: part-family grouping logic;
possibly batch-tracking ontology fields.

### V3 — Regulated/aerospace-defence (CB, DFM, Steel, Aerosystems, SOPHIA):
**air-gap + audit trail**
Air-gapped deployment, provenance-preserving semiotic editing, quality-failure
persistence as audit evidence. NEVER claim certification compliance — position as
"supporta la vostra evidenza documentale". DFM variant: sits ON TOP of existing MES
exports (no new dashboard). SOPHIA variant: prototype mode — anomaly/RUL lead, OEE secondary.
Tuning: document ingest for travelers/route cards; possibly custom defence ontology terms.

### V4 — Foundries/aluminum (Italpres, Almeco, SL, FAR, Pandolfo): **scrap-first**
Lead with yield/scrap cost, not downtime: pressofusione economics die on scrap rate.
Thermal-anomaly demo (4.4σ bearing temp) maps to furnace/die temperature monitoring.
Pandolfo variant: feeds their EXISTING quality system (SPC ontology), never replaces it.
Tuning: scrap-reason categories mapped to yield&scrap ontology; SMA-30 inventory angle
for alloy ingot supply.

### V5 — Cross-functional/admin-heavy (FILA): **office + production**
Production reports PLUS document/mail triage via office ontology (75 domains) —
one engine for backoffice and reparto. Tuning: email/document connectors; watch-mode
on their shared folders.

### V6 — Channel/OEM (OLMA, rsAutomazione, ISIPC, Campi, AEC, FarmaImpesa):
**white-label engine license**
They own the customer; we license the license-gated engine (Ed25519 keys, seat counts)
as a branded reporting/analytics module. Prereq: seat enforcement + key revocation
hardening (BUSINESS.md items #1/#2). Price: per-seat wholesale, negotiable.
Do not pitch these until ≥1 direct pilot exists (reference needed).

### V7 — Process facilitator (Generalmeccanica): **force-multiplier**
Already proposed (proposta-generalmecannica.md): Physis lets one facilitator handle 5×
more clients. Follow-up drafted; force a decision, max 1 sollecito, then close.

---

## Send-order recommendation (next 3 weeks)

| Week | Sends | Why |
|---|---|---|
| W1 | Moro Srl, CB Meccanica (drafts ready) + BMech draft | Confirmed emails, drafts exist, Tier A/B spread |
| W1 | Generalmeccanica follow-up (text ready) | Oldest warm lead; force decision |
| W2 | Officine Cioetto, FILA, MG Tecno, Marcato | Confirmed emails, Tier A |
| W3 | Italpres (V4 test of foundry angle), DFM (V3-on-MES test) | Validate V3/V4 variants before wider blast |
| W4+ | Domain-guess rows ONLY after on-site email verification | Don't burn domain reputation on guesses |

## Verification checklist (before ANY send)

1. Email exists on company site (contact page / Imprint / Privacy policy footer).
2. Named production/Azienda decision-maker if findable in <5 min (LinkedIn/company site).
3. Sender identity: gio@praxisweb.xyz live (Zoho setup, GO_TO_MARKET §business email).
4. Sample link praxisweb.xyz/physis working.
5. Log send in this file's status column.
