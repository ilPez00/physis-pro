//! Demo: build a realistic two-machine shift, score an anomaly, project RUL,
//! and render the Italian shift report (INDUSTRIAL_VERTICAL_PLAN.md step 2).
//!
//! Run: `cargo run --example shift_report_demo`
//! Pass `--json` to print the `ShiftReportInput` JSON instead (the same payload
//! `POST /api/v1/report/shift` and `physis-pro report --shift <file>` accept).

use physis_pro::maintenance::{HealthBaseline, Sample, project_time_to_limit};
use physis_pro::oee::{OeeShift, StopReason};
use physis_pro::report::{render_shift_report_it, RulEntry, ShiftReportInput};

fn main() {
    // ── Tornio CNC: one breakdown, one changeover, some scrap ───────────────
    let mut tornio = OeeShift::new("tornio-cnc-01", 8.0 * 3600.0, 30.0);
    tornio.record_stop(StopReason::Breakdown, 2700.0, "surriscaldamento cuscinetto mandrino");
    tornio.record_stop(StopReason::SetupAdjustment, 1200.0, "cambio utensile");
    tornio.record_stop(StopReason::PlannedStop, 1800.0, "pausa pranzo");
    tornio.record_production(610, 17);

    // ── Fresa: smooth shift, minor stops only ────────────────────────────────
    let mut fresa = OeeShift::new("fresa-02", 8.0 * 3600.0, 45.0);
    fresa.record_stop(StopReason::MinorStop, 600.0, "inceppamento alimentatore");
    fresa.record_stop(StopReason::PlannedStop, 1800.0, "pausa pranzo");
    fresa.record_production(540, 3);

    // ── Anomaly: bearing temperature 4σ above the healthy baseline ──────────
    let mut baseline = HealthBaseline::new("tornio-cnc-01");
    for i in 0..40 {
        baseline.observe(&reading(58.0 + (i % 8) as f32 * 1.5)); // healthy 58-68.5°C
    }
    let anomaly = baseline.score(&reading(78.5), 3.0);

    // ── RUL: temperature trending toward the 85°C limit ─────────────────────
    let history: Vec<Sample> = (0..12)
        .map(|i| Sample { t: i as f64 * 1800.0, value: 62.0 + i as f64 * 1.5 })
        .collect();
    let rul = project_time_to_limit(&history, 85.0).expect("rising trend projects");

    let input = ShiftReportInput {
        oee: vec![tornio.compute(), fresa.compute()],
        plant: None, // computed from oee
        anomalies: vec![anomaly],
        rul: vec![RulEntry {
            machine_id: "tornio-cnc-01".into(),
            metric: "temperature".into(),
            estimate: rul,
        }],
        countermeasures: vec![
            "Ispezionare e sostituire il cuscinetto mandrino del tornio-cnc-01 entro 48h".into(),
            "Anticipare il cambio utensile a fine turno per ridurre il fermo di setup".into(),
            "Verificare la taratura dell'alimentatore della fresa-02".into(),
        ],
        shift_label: Some("Turno 1 — officina meccanica".into()),
    };

    if std::env::args().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&input).expect("serializes"));
    } else {
        println!("{}", render_shift_report_it(&input));
    }
}

fn reading(temp: f32) -> physis_pro::industrial::MachineReading {
    physis_pro::industrial::MachineReading {
        machine_id: "tornio-cnc-01".into(),
        timestamp: chrono::Utc::now(),
        temperature: Some(temp),
        pressure: None,
        vibration: None,
        rpm: None,
        status: physis_pro::industrial::MachineStatus::Online,
        raw: Default::default(),
    }
}
