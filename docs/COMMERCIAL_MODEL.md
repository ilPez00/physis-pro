# Commercial model and licensing

This document describes the proposed packaging. `LICENSE.txt` remains the
controlling legal agreement; pricing and packaging are commercial terms in the
signed order form.

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

## Commercial MVP backlog

- [ ] SKU-to-feature entitlement table and renewal checklist.
- [ ] Evaluation-key issuance and a documented 14-day pilot flow.
- [ ] Customer/plant/host identity in report metadata.
- [ ] Durable shift history before multi-replica hosted deployment.
- [ ] Invoice/checkout process outside the engine first; automate only after
      three paying customers establish the pricing and tax workflow.
- [ ] Track pilot-to-recurring conversion, time-to-first-report, exception
      precision, and customer decision changes; these become pricing evidence.
