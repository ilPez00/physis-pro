# Physis Pro → Lean / Industry 4.0 Management Tool

Turn physis_pro from an ontological reasoning engine into a shop-floor +
backoffice management tool for lean production and Industry 4.0. The coherence
model (+1 / 0 / −1) already *is* the muda taxonomy (value-add / waste / defect)
and PDCA + dream already *is* the kaizen loop. We add the missing
metric/telemetry/visualization primitives on top.

## Differentiator

MES dashboards (SAP, Ignition) *display* OEE and defects. Physis *closes the
loop*: anomaly → −1 coherence node → dream proposes a countermeasure → PDCA
grades whether the fix actually moved the metric. A self-improving lean system,
not a dashboard.

## Roadmap (priority order)

| Phase | Deliverable | Why first |
|------|-------------|-----------|
| **P0** | `oee.rs` (OEE + six-big-losses) + real MQTT `poll()` + `/api/v1/oee/*` | Industry-4.0 table-stakes KPI; Quality factor falls out of coherence we already track. Telemetry is currently a stub (`poll()` bails). |
| P1 | Embedding-based anomaly + predictive maintenance; feed anomalies as −1 nodes → dream | Unique selling point: auto-kaizen. Reuses the vector space. |
| P2 | Time-series historian (sled) + SPC control charts (`/api/v1/spc/*`) | Unlocks every trend KPI. |
| P3 | Real Gantt: true critical path + takt + schedule adherence | Rip the dummy `gantt.rs`. |
| P4 | A3 problem-solving object (5-whys + countermeasures + verify) | The artifact lean managers actually use. |
| P5 | Management dashboard: OEE board, Pareto over semiotic grid, Andon wall + push | Manager-readable surface. |
| P6 | Inventory → real JIT: safety stock from lead-time variance, EOQ, supplier scorecard | Uses the currently-unused `lead_time_days`. |

## P0 detail (this change)

### `src/oee.rs` (new, pure — no feature gate)
- `StopReason` → six-big-losses bucket (Availability / Performance loss).
- `OeeShift` accumulator: `record_stop`, `record_production(good, scrap)`,
  `compute() -> OeeReport`.
- Standard formulas:
  - Planned Production Time = shift − planned stops
  - Availability = Run Time / Planned Production Time
  - Performance   = (Ideal Cycle × Total Count) / Run Time  (capped 1.0)
  - Quality       = Good Count / Total Count
  - OEE           = A × P × Q
- `plant_oee(&[OeeReport])` — planned-time-weighted rollup.
- Six-big-losses breakdown for Pareto (P5).
- Fully unit-tested; div-by-zero → 0.0.

### `src/industrial.rs`
- Implement real `MqttAdapter::poll()`: drive the event loop, parse incoming
  JSON payload into `MachineReading` (was `bail!("not implemented")`).

### `src/bin/web_server.rs`
- Plant-wide `Mutex<OeeBoard>` in `AppCtx` (machines are shared infra, not
  per-tenant).
- Routes:
  - `POST /api/v1/oee/machine` — upsert machine config (ideal cycle, shift).
  - `POST /api/v1/oee/event` — record stop / production count.
  - `GET  /api/v1/oee/:machine` — single-machine OEE report.
  - `GET  /api/v1/oee/plant` — weighted plant rollup + six-big-losses Pareto.

### Wiring to coherence (closes the loop, P0→P1 seam)
- Each scrap count registers a −1 node; each good count a +1 node — so OEE
  Quality and the global coherence index agree, and the dream loop can already
  see defects.
</content>
</invoke>
