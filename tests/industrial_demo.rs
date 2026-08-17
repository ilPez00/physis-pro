//! Revenue-critical demo contract.
//!
//! This is deliberately independent of the web server and external models. It
//! proves the shortest sales path: production facts become OEE, a degrading
//! machine becomes an actionable anomaly/RUL signal, and management receives a
//! readable Italian shift report.

use physis_pro::industrial::{MachineReading, MachineStatus};
use physis_pro::maintenance::{project_time_to_limit, HealthBaseline, Sample};
use physis_pro::oee::{OeeShift, StopReason};
use physis_pro::report::{render_shift_report_it, RulEntry, ShiftReportInput};

fn reading(machine_id: &str, temperature: f32) -> MachineReading {
    MachineReading {
        machine_id: machine_id.into(),
        timestamp: chrono::DateTime::parse_from_rfc3339("2026-08-17T08:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        temperature: Some(temperature),
        pressure: None,
        vibration: None,
        rpm: None,
        status: MachineStatus::Online,
        raw: Default::default(),
    }
}

#[test]
fn paid_pilot_demo_produces_oee_anomaly_rul_and_italian_report() {
    let mut lathe = OeeShift::new("tornio-cnc-01", 8.0 * 3600.0, 30.0);
    lathe.record_stop(StopReason::Breakdown, 2700.0, "surriscaldamento cuscinetto");
    lathe.record_stop(StopReason::SetupAdjustment, 1200.0, "cambio utensile");
    lathe.record_stop(StopReason::PlannedStop, 1800.0, "pausa pranzo");
    lathe.record_production(610, 17);

    let mut mill = OeeShift::new("fresa-02", 8.0 * 3600.0, 45.0);
    mill.record_stop(StopReason::MinorStop, 600.0, "inceppamento alimentatore");
    mill.record_stop(StopReason::PlannedStop, 1800.0, "pausa pranzo");
    mill.record_production(540, 3);

    let lathe_report = lathe.compute();
    let mill_report = mill.compute();
    assert!(
        lathe_report.oee < 0.70,
        "the demo must expose a real loss: {}",
        lathe_report.oee
    );
    assert!(mill_report.oee > lathe_report.oee);
    assert!(lathe_report
        .losses
        .iter()
        .any(|loss| loss.label == "Defects / Rework"));

    let mut baseline = HealthBaseline::new("tornio-cnc-01");
    for i in 0..40 {
        baseline.observe(&reading("tornio-cnc-01", 58.0 + (i % 8) as f32 * 1.5));
    }
    let anomaly = baseline.score(&reading("tornio-cnc-01", 78.5), 3.0);
    assert!(anomaly.baseline_ready);
    assert!(anomaly.is_anomaly);
    assert!(anomaly.max_abs_z >= 3.0);

    let history: Vec<Sample> = (0..12)
        .map(|i| Sample {
            t: i as f64 * 1800.0,
            value: 62.0 + i as f64 * 1.5,
        })
        .collect();
    let rul = project_time_to_limit(&history, 85.0).expect("rising trend must project");
    assert!(rul.seconds_to_limit > 7_000.0 && rul.seconds_to_limit < 8_000.0);

    let report = render_shift_report_it(&ShiftReportInput {
        oee: vec![lathe_report, mill_report],
        anomalies: vec![anomaly],
        rul: vec![RulEntry {
            machine_id: "tornio-cnc-01".into(),
            metric: "temperature".into(),
            estimate: rul,
        }],
        countermeasures: vec!["Ispezionare il cuscinetto entro 48h".into()],
        shift_label: Some("Turno 1 — officina meccanica".into()),
        ..Default::default()
    });

    assert!(report.contains("OEE impianto"));
    assert!(report.contains("Anomalie rilevate"));
    assert!(report.contains("Vita utile residua stimata"));
    assert!(report.contains("Ispezionare il cuscinetto entro 48h"));
}
