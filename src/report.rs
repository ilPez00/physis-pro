//! Shift report — the sellable artifact of the industrial vertical
//! (INDUSTRIAL_VERTICAL_PLAN.md step 2).
//!
//! Pure composition: takes OEE, anomaly, RUL, and countermeasure data already
//! computed by `oee` / `maintenance` / the dream loop and renders one readable
//! Markdown document per shift. Italian template first — the buyer reads
//! Italian. No state, no I/O: callers (web handler, CLI) gather the inputs, so
//! the renderer is replica-safe and works identically online and locally.

use serde::{Deserialize, Serialize};

use crate::maintenance::{AnomalyScore, RulEstimate};
use crate::oee::{OeeReport, PlantOee};

/// Everything a shift report can include. Only `oee` is required; empty
/// sections are omitted from the output rather than rendered hollow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShiftReportInput {
    /// Per-machine OEE for the shift (one entry per machine).
    pub oee: Vec<OeeReport>,
    /// Plant rollup; computed from `oee` when absent and there are ≥2 machines.
    #[serde(default)]
    pub plant: Option<PlantOee>,
    /// Anomalies detected during the shift.
    #[serde(default)]
    pub anomalies: Vec<AnomalyScore>,
    /// Remaining-useful-life projections: (machine_id, metric, estimate).
    #[serde(default)]
    pub rul: Vec<RulEntry>,
    /// Proposed countermeasures (typically dream-loop output).
    #[serde(default)]
    pub countermeasures: Vec<String>,
    /// Free-form shift label, e.g. "Turno 1 — 2026-06-12". Optional.
    #[serde(default)]
    pub shift_label: Option<String>,
}

/// A remaining-useful-life projection bound to a machine and metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulEntry {
    pub machine_id: String,
    pub metric: String,
    pub estimate: RulEstimate,
}

/// Format seconds as a compact Italian duration: "1h 23m", "45m", "30s".
fn dur(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

fn pct(x: f32) -> String {
    format!("{:.1}%", x * 100.0)
}

/// Render the Italian shift report as Markdown.
pub fn render_shift_report_it(input: &ShiftReportInput) -> String {
    let mut md = String::new();
    let title = input.shift_label.as_deref().unwrap_or("Rapporto di turno");
    md.push_str(&format!("# {title}\n\n"));

    // ── Plant rollup (when more than one machine) ───────────────────────────
    let computed_plant;
    let plant = match (&input.plant, input.oee.len()) {
        (Some(p), _) => Some(p),
        (None, n) if n >= 2 => {
            computed_plant = crate::oee::plant_oee(&input.oee);
            Some(&computed_plant)
        }
        _ => None,
    };
    if let Some(p) = plant {
        md.push_str("## Impianto\n\n");
        md.push_str(&format!(
            "**OEE impianto: {}** — disponibilità {}, prestazione {}, qualità {} \
             ({} macchine, {} pezzi buoni su {}).\n\n",
            pct(p.oee),
            pct(p.availability),
            pct(p.performance),
            pct(p.quality),
            p.machine_count,
            p.total_good,
            p.total_parts,
        ));
    }

    // ── Per-machine OEE ─────────────────────────────────────────────────────
    for r in &input.oee {
        md.push_str(&format!("## Macchina {}\n\n", r.machine_id));
        md.push_str("| Indicatore | Valore |\n|---|---|\n");
        md.push_str(&format!("| Disponibilità | {} |\n", pct(r.availability)));
        md.push_str(&format!("| Prestazione | {} |\n", pct(r.performance)));
        md.push_str(&format!("| Qualità | {} |\n", pct(r.quality)));
        md.push_str(&format!(
            "| **OEE** | **{}**{} |\n",
            pct(r.oee),
            if r.world_class { " (world-class ≥85%)" } else { "" }
        ));
        md.push_str(&format!(
            "| Produzione | {} pezzi buoni / {} totali |\n",
            r.good_count, r.total_count
        ));
        md.push_str(&format!(
            "| Tempo di marcia | {} su {} pianificati |\n\n",
            dur(r.run_time_secs as f64),
            dur(r.planned_production_time_secs as f64)
        ));

        if !r.losses.is_empty() {
            md.push_str("### Perdite principali\n\n");
            md.push_str("| Causa | Tempo perso | Pezzi persi |\n|---|---|---|\n");
            for b in &r.losses {
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    b.label,
                    dur(b.lost_secs as f64),
                    b.lost_parts
                ));
            }
            md.push('\n');
        }
    }

    // ── Anomalies ───────────────────────────────────────────────────────────
    let live: Vec<&AnomalyScore> = input.anomalies.iter().filter(|a| a.is_anomaly).collect();
    if !live.is_empty() {
        md.push_str("## Anomalie rilevate\n\n");
        for a in live {
            md.push_str(&format!(
                "- **{}** — {} a {:.1}σ dalla linea di base (valore {:.3}, z={:+.2})\n",
                a.machine_id,
                a.worst_feature,
                a.max_abs_z,
                a.per_feature
                    .iter()
                    .find(|f| f.feature == a.worst_feature)
                    .map(|f| f.value)
                    .unwrap_or(f32::NAN),
                a.per_feature
                    .iter()
                    .find(|f| f.feature == a.worst_feature)
                    .map(|f| f.z)
                    .unwrap_or(f32::NAN),
            ));
        }
        md.push('\n');
    }

    // ── RUL projections ─────────────────────────────────────────────────────
    if !input.rul.is_empty() {
        md.push_str("## Vita utile residua stimata\n\n");
        for e in &input.rul {
            md.push_str(&format!(
                "- **{} / {}** — limite {} raggiunto tra ~{} (valore attuale {:.2}, deriva {:+.4}/s)\n",
                e.machine_id,
                e.metric,
                e.estimate.limit,
                dur(e.estimate.seconds_to_limit),
                e.estimate.current_value,
                e.estimate.slope_per_sec,
            ));
        }
        md.push('\n');
    }

    // ── Countermeasures ─────────────────────────────────────────────────────
    if !input.countermeasures.is_empty() {
        md.push_str("## Contromisure proposte\n\n");
        for c in &input.countermeasures {
            md.push_str(&format!("- {c}\n"));
        }
        md.push('\n');
    }

    md.push_str("---\n*Generato da Physis — classificazione ontologica e coerenza di processo.*\n");
    md
}

/// Minimal HTML escaping for text we interpolate into the document.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Render the Italian shift report as a self-contained HTML document —
/// inline CSS, no external assets, so it works as an email body and prints
/// cleanly to PDF from any browser (the "render path" without new binaries
/// in the container; a native PDF writer can come later if a customer needs
/// the file generated server-side).
pub fn render_shift_report_html(input: &ShiftReportInput) -> String {
    let title = input.shift_label.as_deref().unwrap_or("Rapporto di turno");
    let mut b = String::new();

    let computed_plant;
    let plant = match (&input.plant, input.oee.len()) {
        (Some(p), _) => Some(p),
        (None, n) if n >= 2 => {
            computed_plant = crate::oee::plant_oee(&input.oee);
            Some(&computed_plant)
        }
        _ => None,
    };
    if let Some(p) = plant {
        b.push_str(&format!(
            "<section><h2>Impianto</h2><p class=\"kpi-line\"><strong>OEE impianto: {}</strong> — \
             disponibilità {}, prestazione {}, qualità {} ({} macchine, {} pezzi buoni su {}).</p></section>",
            pct(p.oee), pct(p.availability), pct(p.performance), pct(p.quality),
            p.machine_count, p.total_good, p.total_parts,
        ));
    }

    for r in &input.oee {
        b.push_str(&format!("<section><h2>Macchina {}</h2>", esc(&r.machine_id)));
        let wc = if r.world_class { " <span class=\"badge\">world-class ≥85%</span>" } else { "" };
        b.push_str(&format!(
            "<table><tbody>\
             <tr><th>Disponibilità</th><td>{}</td></tr>\
             <tr><th>Prestazione</th><td>{}</td></tr>\
             <tr><th>Qualità</th><td>{}</td></tr>\
             <tr class=\"oee\"><th>OEE</th><td><strong>{}</strong>{}</td></tr>\
             <tr><th>Produzione</th><td>{} pezzi buoni / {} totali</td></tr>\
             <tr><th>Tempo di marcia</th><td>{} su {} pianificati</td></tr>\
             </tbody></table>",
            pct(r.availability), pct(r.performance), pct(r.quality), pct(r.oee), wc,
            r.good_count, r.total_count,
            dur(r.run_time_secs as f64), dur(r.planned_production_time_secs as f64),
        ));
        if !r.losses.is_empty() {
            b.push_str("<h3>Perdite principali</h3><table><thead><tr><th>Causa</th><th>Tempo perso</th><th>Pezzi persi</th></tr></thead><tbody>");
            for l in &r.losses {
                b.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    esc(&l.label), dur(l.lost_secs as f64), l.lost_parts
                ));
            }
            b.push_str("</tbody></table>");
        }
        b.push_str("</section>");
    }

    let live: Vec<&AnomalyScore> = input.anomalies.iter().filter(|a| a.is_anomaly).collect();
    if !live.is_empty() {
        b.push_str("<section><h2>Anomalie rilevate</h2><ul class=\"alert\">");
        for a in live {
            let f = a.per_feature.iter().find(|f| f.feature == a.worst_feature);
            b.push_str(&format!(
                "<li><strong>{}</strong> — {} a {:.1}σ dalla linea di base (valore {:.3}, z={:+.2})</li>",
                esc(&a.machine_id), esc(&a.worst_feature), a.max_abs_z,
                f.map(|f| f.value).unwrap_or(f32::NAN),
                f.map(|f| f.z).unwrap_or(f32::NAN),
            ));
        }
        b.push_str("</ul></section>");
    }

    if !input.rul.is_empty() {
        b.push_str("<section><h2>Vita utile residua stimata</h2><ul>");
        for e in &input.rul {
            b.push_str(&format!(
                "<li><strong>{} / {}</strong> — limite {} raggiunto tra ~{} (valore attuale {:.2}, deriva {:+.4}/s)</li>",
                esc(&e.machine_id), esc(&e.metric), e.estimate.limit,
                dur(e.estimate.seconds_to_limit), e.estimate.current_value, e.estimate.slope_per_sec,
            ));
        }
        b.push_str("</ul></section>");
    }

    if !input.countermeasures.is_empty() {
        b.push_str("<section><h2>Contromisure proposte</h2><ul>");
        for c in &input.countermeasures {
            b.push_str(&format!("<li>{}</li>", esc(c)));
        }
        b.push_str("</ul></section>");
    }

    format!(
        "<!DOCTYPE html><html lang=\"it\"><head><meta charset=\"utf-8\">\
         <title>{t}</title><style>{css}</style></head><body>\
         <h1>{t}</h1>{body}\
         <footer>Generato da Physis — classificazione ontologica e coerenza di processo.</footer>\
         </body></html>",
        t = esc(title),
        css = REPORT_CSS,
        body = b,
    )
}

/// Inline stylesheet: legible on screen and in email clients, compact when
/// printed (browser "print to PDF" is the PDF path).
const REPORT_CSS: &str = "\
body{font-family:-apple-system,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1a1a1a;max-width:720px;margin:2rem auto;padding:0 1rem;line-height:1.45}\
h1{font-size:1.5rem;border-bottom:2px solid #1a1a1a;padding-bottom:.4rem}\
h2{font-size:1.15rem;margin-top:1.6rem}h3{font-size:1rem}\
table{border-collapse:collapse;width:100%;margin:.6rem 0}\
th,td{border:1px solid #ccc;padding:.35rem .6rem;text-align:left;font-size:.92rem}\
thead th{background:#f2f2f2}tbody th{width:11rem;font-weight:600;background:#fafafa}\
tr.oee td,tr.oee th{background:#eef6ee}\
.badge{background:#2e7d32;color:#fff;border-radius:3px;padding:.05rem .4rem;font-size:.78rem;margin-left:.4rem}\
ul.alert li{color:#b71c1c}\
footer{margin-top:2rem;font-size:.8rem;color:#666;border-top:1px solid #ddd;padding-top:.5rem}\
@media print{body{margin:0;max-width:none}h2{break-after:avoid}section{break-inside:avoid}}";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oee::{OeeShift, StopReason};

    fn shift_report() -> OeeReport {
        let mut s = OeeShift::new("tornio-1", 8.0 * 3600.0, 30.0);
        s.record_stop(StopReason::Breakdown, 1800.0, "cuscinetto surriscaldato");
        s.record_production(800, 13);
        s.compute()
    }

    #[test]
    fn test_report_contains_kpis_and_losses() {
        let input = ShiftReportInput {
            oee: vec![shift_report()],
            shift_label: Some("Turno 1 — 2026-06-12".into()),
            countermeasures: vec!["Ispezione cuscinetto mandrino entro 48h".into()],
            ..Default::default()
        };
        let md = render_shift_report_it(&input);
        assert!(md.contains("# Turno 1 — 2026-06-12"));
        assert!(md.contains("Macchina tornio-1"));
        assert!(md.contains("Disponibilità"));
        assert!(md.contains("OEE"));
        assert!(md.contains("813 totali"), "total parts in production row:\n{md}");
        assert!(md.contains("Perdite principali"));
        assert!(md.contains("Contromisure proposte"));
        assert!(md.contains("Ispezione cuscinetto"));
        // Single machine → no plant section.
        assert!(!md.contains("## Impianto"));
    }

    #[test]
    fn test_two_machines_get_plant_rollup() {
        let input = ShiftReportInput {
            oee: vec![shift_report(), {
                let mut s = OeeShift::new("fresa-2", 8.0 * 3600.0, 45.0);
                s.record_production(500, 4);
                s.compute()
            }],
            ..Default::default()
        };
        let md = render_shift_report_it(&input);
        assert!(md.contains("## Impianto"));
        assert!(md.contains("2 macchine"));
        assert!(md.contains("Macchina tornio-1"));
        assert!(md.contains("Macchina fresa-2"));
    }

    #[test]
    fn test_empty_sections_omitted() {
        let input = ShiftReportInput { oee: vec![shift_report()], ..Default::default() };
        let md = render_shift_report_it(&input);
        assert!(!md.contains("Anomalie rilevate"));
        assert!(!md.contains("Vita utile residua"));
        assert!(!md.contains("Contromisure"));
    }

    #[test]
    fn test_html_render_is_self_contained_and_escaped() {
        let mut r = shift_report();
        r.machine_id = "tornio <1>".into(); // must be escaped
        let input = ShiftReportInput {
            oee: vec![r],
            countermeasures: vec!["Stringere & verificare".into()],
            ..Default::default()
        };
        let html = render_shift_report_html(&input);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<style>"), "CSS must be inline");
        assert!(html.contains("tornio &lt;1&gt;"), "machine id escaped");
        assert!(html.contains("Stringere &amp; verificare"), "countermeasure escaped");
        assert!(html.contains("Disponibilità"));
        assert!(!html.contains("<script"), "no scripts in an email-able document");
    }

    #[test]
    fn test_duration_formatting() {
        assert_eq!(dur(30.0), "30s");
        assert_eq!(dur(300.0), "5m");
        assert_eq!(dur(5400.0), "1h 30m");
    }
}
