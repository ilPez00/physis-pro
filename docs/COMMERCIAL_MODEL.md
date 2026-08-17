# Commercial model and licensing

This document describes the proposed packaging. `LICENSE.txt` remains the
controlling legal agreement; pricing and packaging are commercial terms in the
signed order form.

## Revenue model

### 1. Shift Loss Pilot

- €1,500–€3,000 one-time
- one plant, one week of data, up to two machines for the initial scope
- Italian HTML shift reports, loss review, and baseline/anomaly demonstration
- no promise of safety, guaranteed savings, or predictive accuracy without
  sufficient historical data

### 2. Plant Monitor

- €500–€1,500 per plant/month, priced by machine count and support level
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
