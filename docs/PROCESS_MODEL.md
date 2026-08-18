# Process Semantics & The Industrial Reasoning Loop

## 1. Motivation

In industrial, manufacturing, and operational settings, intelligence cannot operate purely on static text or disconnected knowledge graphs.

Intelligence must understand **processes**:
- What were we trying to accomplish? (**Goal / Plan**)
- What steps were executed? (**Tasks / Transitions**)
- What physical measurements occurred? (**Observations / Measurements**)
- Where did execution diverge from expectations? (**Deviations / Anomalies**)
- What corrective action was taken? (**Interventions**)
- What was the final result? (**Outcomes**)

Physis integrates process semantics directly underneath its semantic reasoning and hypothesis evaluation engine.

---

## 2. The Operational Reasoning Loop

```
                         ┌──────────────┐
                         │     PLAN     │ (Goals, Tasks, Constraints)
                         └──────────────┘
                                │
                                ▼
                         ┌──────────────┐
                         │    ACTION    │ (Execution, Resource allocation)
                         └──────────────┘
                                │
                                ▼
                         ┌──────────────┐
                         │ OBSERVATION  │ (Telemetry, Measurements, Sensor logs)
                         └──────────────┘
                                │
                                ▼
                         ┌──────────────┐
                         │  DEVIATION   │ (OEE drop, scrap, threshold breach)
                         └──────────────┘
                                │
                                ▼
                         ┌──────────────┐
                         │ INTERVENTION │ (Operator fix, parameter tweak, emergency stop)
                         └──────────────┘
                                │
                                ▼
                         ┌──────────────┐
                         │   OUTCOME    │ (Quality verdict, yield, recovery)
                         └──────────────┘
```

---

## 3. Core Process Entities (`process.rs`)

### 1. Goal & Plan
- **`ProcessGoal`**: Target outcome (e.g. "Maintain OEE > 85%", "Produce 500 units without scrap").
- **`ProcessPlan`**: Graph of tasks and operational constraints.

### 2. Tasks & States
- **`ProcessTask`**: Individual workflow unit with state (`Pending`, `InProgress`, `Completed`, `Blocked`, `Failed`, `Intervened`).
- **`ProcessState`**: Discrete machine/system state parameter snapshot.
- **`StateTransition`**: Triggered change from state $A$ to state $B$.

### 3. Resources & Constraints
- **`ProcessResource`**: Machine, operator, material lot, or tool with capacity and current utilization.
- **`ProcessConstraint`**: Hard or soft operating boundaries (e.g. "Extruder Temp $\le 240^\circ\text{C}$").

### 4. Telemetry & Deviations
- **`ProcessMeasurement`**: Time-stamped physical measurement from machine adapters (Modbus, MQTT, serial, REST).
- **`ProcessDeviation`**: Detected difference between expected nominal value and measured reality.

### 5. Interventions & Outcomes
- **`ProcessIntervention`**: Human or automated action taken to correct a deviation.
- **`ProcessOutcome`**: Final recorded verdict, scrap count, and duration.
- **`ProcessCycle`**: Full execution record linking Plan $\rightarrow$ Action $\rightarrow$ Measurement $\rightarrow$ Deviation $\rightarrow$ Intervention $\rightarrow$ Outcome.

---

## 4. Bridging Industrial Frameworks to Epistemic Reasoning

Physis acts as the unified semantic foundation beneath multiple industrial paradigms:

| Framework | Process Entity Mapping in Physis | Epistemic Role |
| :--- | :--- | :--- |
| **PDCA (Deming Cycle)** | Plan $\rightarrow$ Do (Action) $\rightarrow$ Check (Measurement/Deviation) $\rightarrow$ Act (Intervention) | Drives the continuous hypothesis revision and dream loop. |
| **Lean / Six Sigma** | Defects, scrap counts, and tolerance deviations | Forms negative quality feedback and contradiction triggers. |
| **Gantt / Project Scheduling** | Task dependencies (`DependsOn`, `Requires`, `Precedes`) | Provides temporal and procedural coherence constraints. |
| **OEE / Industrial Telemetry** | Availability, Performance, and Quality metrics | Ground truth empirical measurements for hypothesis verification. |

By grounding processes in this epistemic architecture, Physis does not merely record what happened: it explains **why** deviations occurred and learns **which** interventions work.
