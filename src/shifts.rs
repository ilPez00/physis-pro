//! Shift history — CSV/JSON import with row-level validation, durable
//! per-tenant storage, and aggregation into OEE reports.
//!
//! Covers PLAN.md 5.1 (CSV/JSON shift importer, validation + error output)
//! and 5.3 (durable tenant/plant-scoped shift history). The importer accepts
//! partial files: valid rows land, invalid rows come back with line numbers
//! and reasons, so one typo in a shop-floor export never bounces the batch.
//!
//! Storage mirrors the Study Log pattern: one sled blob per tenant keyed by
//! `shifts:history:{uid}`, capped at the newest [`HISTORY_CAP`] records.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::oee::{OeeReport, OeeShift, StopEvent, StopReason};

/// Newest-cap on a tenant's stored shift history (same ceiling as Study Log).
pub const HISTORY_CAP: usize = 5000;

/// One persisted shift entry: the OEE accumulator plus optional provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftRecord {
    /// When the shift happened (caller-supplied; RFC 3339 on import).
    #[serde(default)]
    pub recorded_at: Option<DateTime<Utc>>,
    /// Free-form label, e.g. "Turno 1 — 2026-06-12".
    #[serde(default)]
    pub shift_label: Option<String>,
    /// The shift data itself.
    pub shift: OeeShift,
}

/// One rejected row: where it was in the source and why it failed validation.
/// Line numbers refer to physical lines of the uploaded text for CSV, and to
/// 1-based element positions for JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowError {
    pub line: usize,
    pub error: String,
}

/// Result of an import: what landed, what bounced, how much history the tenant
/// now holds.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub accepted: usize,
    pub rejected: Vec<RowError>,
    pub stored: usize,
}

// ── Validation ───────────────────────────────────────────────────────────────

/// Business rules every imported shift must satisfy, regardless of source
/// format. Empty list = valid.
pub fn validate_shift(shift: &OeeShift) -> Vec<String> {
    let mut errors = Vec::new();
    if shift.machine_id.trim().is_empty() {
        errors.push("machine_id must not be blank".into());
    }
    if !shift.shift_length_secs.is_finite() || shift.shift_length_secs < 0.0 {
        errors.push("shift_length_secs must be a non-negative number".into());
    }
    if !shift.ideal_cycle_time_secs.is_finite() || shift.ideal_cycle_time_secs < 0.0 {
        errors.push("ideal_cycle_time_secs must be a non-negative number".into());
    }
    if shift.total_count > 0 && shift.ideal_cycle_time_secs <= 0.0 {
        errors.push("ideal_cycle_time_secs must be > 0 when total_count > 0".into());
    }
    if shift.good_count > shift.total_count {
        errors.push(format!(
            "good_count ({}) cannot exceed total_count ({})",
            shift.good_count, shift.total_count
        ));
    }
    for stop in &shift.stops {
        if !stop.duration_secs.is_finite() || stop.duration_secs < 0.0 {
            errors.push("stop durations must be non-negative numbers".into());
            break;
        }
    }
    errors
}

/// Parse a stop reason from flexible CSV spelling ("breakdown", "Setup &
/// Adjustment", "minor_stop", …). Returns None when nothing matches.
fn parse_stop_reason(text: &str) -> Option<StopReason> {
    let key = text.trim().to_lowercase();
    let key = key.replace([' ', '-'], "_");
    match key.as_str() {
        "breakdown" | "failure" => Some(StopReason::Breakdown),
        "setup_adjustment" | "setup" | "changeover" => Some(StopReason::SetupAdjustment),
        "minor_stop" | "minor" | "jam" => Some(StopReason::MinorStop),
        "reduced_speed" | "speed" | "reduced" => Some(StopReason::ReducedSpeed),
        "planned_stop" | "planned" | "no_demand" => Some(StopReason::PlannedStop),
        _ => None,
    }
}

// ── CSV ──────────────────────────────────────────────────────────────────────

const REQUIRED_COLUMNS: [&str; 5] = [
    "machine_id",
    "shift_length_secs",
    "ideal_cycle_time_secs",
    "total_count",
    "good_count",
];
const OPTIONAL_COLUMNS: [&str; 4] = ["downtime_secs", "stop_reason", "recorded_at", "shift_label"];

/// Minimal RFC-4180 reader: quoted fields, doubled quotes, commas, CR/LF/CRLF
/// line endings. Returns `(physical line number of record start, fields)`.
fn split_csv_records(text: &str) -> Vec<(usize, Vec<String>)> {
    let mut records = Vec::new();
    let mut field = String::new();
    let mut fields: Vec<String> = Vec::new();
    let mut in_quotes = false;
    let mut line = 1usize;
    let mut record_line = 1usize;
    let mut saw_any = false;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        field.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                }
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => {
                in_quotes = true;
                saw_any = true;
            }
            ',' => {
                fields.push(std::mem::take(&mut field));
                saw_any = true;
            }
            '\r' => { /* consumed with the newline below */ }
            '\n' => {
                if saw_any || !field.is_empty() {
                    fields.push(std::mem::take(&mut field));
                    records.push((record_line, std::mem::take(&mut fields)));
                }
                line += 1;
                record_line = line;
                saw_any = false;
            }
            _ => {
                field.push(c);
                saw_any = true;
            }
        }
    }
    if saw_any || !field.is_empty() || !fields.is_empty() {
        fields.push(field);
        records.push((record_line, fields));
    }
    records
}

fn parse_num<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.trim()
        .parse::<T>()
        .map_err(|_| format!("'{text}' is not a valid number"))
}

/// Column positions resolved from the CSV header, so row parsing never has to
/// touch the header again.
struct Columns {
    machine_id: usize,
    shift_length_secs: usize,
    ideal_cycle_time_secs: usize,
    total_count: usize,
    good_count: usize,
    downtime_secs: Option<usize>,
    stop_reason: Option<usize>,
    recorded_at: Option<usize>,
    shift_label: Option<usize>,
}

fn resolve_columns(header: &[String]) -> Option<Columns> {
    let pos = |name: &str| header.iter().position(|h| h == name);
    Some(Columns {
        machine_id: pos("machine_id")?,
        shift_length_secs: pos("shift_length_secs")?,
        ideal_cycle_time_secs: pos("ideal_cycle_time_secs")?,
        total_count: pos("total_count")?,
        good_count: pos("good_count")?,
        downtime_secs: pos("downtime_secs"),
        stop_reason: pos("stop_reason"),
        recorded_at: pos("recorded_at"),
        shift_label: pos("shift_label"),
    })
}

fn build_record(cols: &Columns, fields: &[String]) -> Result<ShiftRecord, String> {
    let cell = |i: usize| -> &String { &fields[i] };
    let machine_id = cell(cols.machine_id).trim().to_string();
    let shift_length_secs: f32 = parse_num(cell(cols.shift_length_secs))?;
    let ideal_cycle_time_secs: f32 = parse_num(cell(cols.ideal_cycle_time_secs))?;
    let total_count: u64 = parse_num(cell(cols.total_count))?;
    let good_count: u64 = parse_num(cell(cols.good_count))?;

    let mut stops: Vec<StopEvent> = Vec::new();
    // Blank or zero downtime means "no stoppage" — common in machine exports.
    let downtime_raw = cols.downtime_secs.map(|i| cell(i).trim()).unwrap_or("");
    let downtime_secs: f32 = if downtime_raw.is_empty() {
        0.0
    } else {
        parse_num(downtime_raw)?
    };
    let has_downtime = downtime_secs > 0.0;
    let reason = cols.stop_reason.map(|i| cell(i).trim()).unwrap_or("");
    match (has_downtime, !reason.is_empty()) {
        (true, true) => {
            let parsed = parse_stop_reason(reason)
                .ok_or_else(|| format!("unknown stop_reason '{reason}' (expected e.g. breakdown, setup, minor_stop, reduced_speed, planned_stop)"))?;
            stops.push(StopEvent {
                reason: parsed,
                duration_secs: downtime_secs,
                note: String::new(),
            });
        }
        (true, false) => return Err("downtime_secs given without stop_reason".into()),
        (false, true) => return Err("stop_reason given without downtime_secs".into()),
        (false, false) => {}
    }

    let recorded_at = match cols.recorded_at.map(|i| cell(i).trim()).unwrap_or("") {
        "" => None,
        raw => Some(
            DateTime::parse_from_rfc3339(raw)
                .map_err(|_| {
                    format!("recorded_at '{raw}' is not RFC 3339 (e.g. 2026-08-24T06:00:00Z)")
                })?
                .with_timezone(&Utc),
        ),
    };
    let shift_label = cols.shift_label.map(|i| cell(i).trim()).unwrap_or("");
    let shift_label = (!shift_label.is_empty()).then(|| shift_label.to_string());

    Ok(ShiftRecord {
        recorded_at,
        shift_label,
        shift: OeeShift {
            machine_id,
            shift_length_secs,
            ideal_cycle_time_secs,
            stops,
            total_count,
            good_count,
        },
    })
}

/// Parse + validate a shift-history CSV. Header must name all five required
/// columns and only known columns; data rows are validated independently so a
/// bad row never blocks its neighbours.
pub fn parse_shift_csv(text: &str) -> (Vec<ShiftRecord>, Vec<RowError>) {
    let records = split_csv_records(text);
    let mut out = Vec::new();
    let mut errors = Vec::new();

    let header = match records.first() {
        Some((_, fields)) => fields
            .iter()
            .map(|f| f.trim().to_lowercase())
            .collect::<Vec<_>>(),
        None => {
            return (
                out,
                vec![RowError {
                    line: 1,
                    error: "empty upload: no CSV header".into(),
                }],
            )
        }
    };

    let missing: Vec<&str> = REQUIRED_COLUMNS
        .iter()
        .copied()
        .filter(|c| !header.iter().any(|h| h == c))
        .collect();
    if !missing.is_empty() {
        errors.push(RowError {
            line: header_line(&records),
            error: format!("missing required column(s): {}", missing.join(", ")),
        });
    }
    let unknown: Vec<String> = header
        .iter()
        .filter(|c| {
            !REQUIRED_COLUMNS.contains(&c.as_str()) && !OPTIONAL_COLUMNS.contains(&c.as_str())
        })
        .cloned()
        .collect();
    if !unknown.is_empty() {
        errors.push(RowError {
            line: header_line(&records),
            error: format!(
                "unknown column(s): {} (supported: {})",
                unknown.join(", "),
                REQUIRED_COLUMNS
                    .iter()
                    .chain(OPTIONAL_COLUMNS.iter())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    if !errors.is_empty() {
        // A broken header means column positions are meaningless; report and stop.
        return (out, errors);
    }

    let cols = match resolve_columns(&header) {
        Some(c) => c,
        None => unreachable!("missing required columns already reported above"),
    };

    for (line_no, fields) in records.iter().skip(1) {
        match build_record(&cols, fields) {
            Ok(record) => {
                let problems = validate_shift(&record.shift);
                if problems.is_empty() {
                    out.push(record);
                } else {
                    errors.push(RowError {
                        line: *line_no,
                        error: problems.join("; "),
                    });
                }
            }
            Err(e) => errors.push(RowError {
                line: *line_no,
                error: e,
            }),
        }
    }
    (out, errors)
}

fn header_line(records: &[(usize, Vec<String>)]) -> usize {
    records.first().map(|(l, _)| *l).unwrap_or(1)
}

// ── JSON ─────────────────────────────────────────────────────────────────────

/// Parse + validate a JSON array of shift objects (`OeeShift` shape plus
/// optional `recorded_at` / `shift_label`). Element positions stand in for
/// line numbers in the error output.
pub fn parse_shift_json(elements: &[serde_json::Value]) -> (Vec<ShiftRecord>, Vec<RowError>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, value) in elements.iter().enumerate() {
        let line = i + 1;
        let recorded_at = match value.get("recorded_at") {
            None | Some(serde_json::Value::Null) => None,
            Some(raw) => match raw
                .as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            {
                Some(dt) => Some(dt.with_timezone(&Utc)),
                None => {
                    errors.push(RowError {
                        line,
                        error: "recorded_at must be RFC 3339".into(),
                    });
                    continue;
                }
            },
        };
        let shift_label = value
            .get("shift_label")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());
        // A shift with no stoppages must not be forced to spell out "stops":
        // inject the empty default before strict parsing.
        let mut owned = value.clone();
        if let Some(obj) = owned.as_object_mut() {
            obj.entry("stops").or_insert_with(|| serde_json::json!([]));
        }
        match serde_json::from_value::<OeeShift>(owned) {
            Ok(mut shift) => {
                shift.machine_id = shift.machine_id.trim().to_string();
                let problems = validate_shift(&shift);
                if problems.is_empty() {
                    out.push(ShiftRecord {
                        recorded_at,
                        shift_label,
                        shift,
                    });
                } else {
                    errors.push(RowError {
                        line,
                        error: problems.join("; "),
                    });
                }
            }
            Err(e) => errors.push(RowError {
                line,
                error: format!("invalid shift object: {e}"),
            }),
        }
    }
    (out, errors)
}

// ── Aggregation ──────────────────────────────────────────────────────────────

/// Merge history into one cumulative `OeeShift` per machine (stable order by
/// machine id). Stop events concatenate; counts and scheduled time sum; the
/// ideal cycle time is production-weighted across shifts (last seen wins when
/// no shift produced anything). The result feeds `.compute()` directly, so
/// availability/performance/quality stay mathematically consistent with the
/// underlying rows.
pub fn aggregate(records: &[ShiftRecord]) -> Vec<OeeShift> {
    let mut machines: BTreeMap<String, OeeShift> = BTreeMap::new();
    let mut weighted_cycle: BTreeMap<String, (f64, u64)> = BTreeMap::new();

    for rec in records {
        let shift = &rec.shift;
        let entry = machines
            .entry(shift.machine_id.clone())
            .or_insert_with(|| OeeShift {
                machine_id: shift.machine_id.clone(),
                shift_length_secs: 0.0,
                ideal_cycle_time_secs: shift.ideal_cycle_time_secs,
                stops: Vec::new(),
                total_count: 0,
                good_count: 0,
            });
        entry.shift_length_secs += shift.shift_length_secs;
        entry.stops.extend(shift.stops.iter().cloned());
        entry.total_count += shift.total_count;
        entry.good_count += shift.good_count;
        let w = weighted_cycle.entry(shift.machine_id.clone()).or_default();
        w.0 += shift.ideal_cycle_time_secs as f64 * shift.total_count as f64;
        w.1 += shift.total_count;
        if shift.total_count > 0 {
            entry.ideal_cycle_time_secs = shift.ideal_cycle_time_secs;
        }
    }

    machines
        .into_iter()
        .map(|(id, mut shift)| {
            if let Some((num, den)) = weighted_cycle.get(&id) {
                if *den > 0 {
                    shift.ideal_cycle_time_secs = (*num / *den as f64) as f32;
                }
            }
            shift
        })
        .collect()
}

// ── Evidence export ──────────────────────────────────────────────────────────

/// A self-contained audit bundle for one tenant's shift history: the raw
/// records as uploaded **and** the per-record OEE recomputed at export time,
/// so a customer can verify every headline number from first principles
/// without trusting our aggregation. This is PLAN 5.2's "evidence export".
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceBundle {
    pub generated_at: DateTime<Utc>,
    /// Tenant the history belongs to.
    pub tenant: String,
    /// Machine filter applied, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    pub machine_count: usize,
    pub shift_count: usize,
    /// Observed window over dated records; absent when none carry a date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    /// Raw records, upload order preserved.
    pub records: Vec<ShiftRecord>,
    /// Per-record OEE, aligned index-for-index with `records`.
    pub reports: Vec<OeeReport>,
}

/// Inclusive date window of an evidence bundle.
#[derive(Debug, Clone, Serialize)]
pub struct Span {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

/// Observed window over the dated records; None when none carry a date.
pub fn records_span(records: &[ShiftRecord]) -> Option<Span> {
    let dated: Vec<DateTime<Utc>> = records.iter().filter_map(|r| r.recorded_at).collect();
    (!dated.is_empty()).then(|| Span {
        from: *dated.iter().min().unwrap(),
        to: *dated.iter().max().unwrap(),
    })
}

/// Build the evidence bundle from a tenant's records (already filtered by the
/// caller when a machine slice was requested).
pub fn evidence_bundle(
    tenant: &str,
    machine: Option<&str>,
    records: &[ShiftRecord],
) -> EvidenceBundle {
    let machines: std::collections::BTreeSet<&str> = records
        .iter()
        .map(|r| r.shift.machine_id.as_str())
        .collect();
    let span = records_span(records);
    EvidenceBundle {
        generated_at: Utc::now(),
        tenant: tenant.to_string(),
        machine: machine.map(|m| m.to_string()),
        machine_count: machines.len(),
        shift_count: records.len(),
        span,
        reports: records.iter().map(|r| r.shift.compute()).collect(),
        records: records.to_vec(),
    }
}

// ── Persistence (sled, per tenant) ───────────────────────────────────────────

pub fn shift_key(uid: &str) -> String {
    format!("shifts:history:{uid}")
}

pub fn save_shifts(db: &sled::Db, uid: &str, records: &[ShiftRecord]) -> anyhow::Result<()> {
    db.insert(shift_key(uid), serde_json::to_vec(records)?)?;
    Ok(())
}

pub fn load_shifts(db: &sled::Db, uid: &str) -> Vec<ShiftRecord> {
    db.get(shift_key(uid))
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_slice::<Vec<ShiftRecord>>(&v).ok())
        .unwrap_or_default()
}

/// Wipe a tenant's history entirely (demo reset).
pub fn clear_shifts(db: &sled::Db, uid: &str) {
    let _ = db.remove(shift_key(uid));
}

/// Append to a tenant's history, capping at the newest [`HISTORY_CAP`] records.
/// Returns the new total.
pub fn append_shifts(
    db: &sled::Db,
    uid: &str,
    mut incoming: Vec<ShiftRecord>,
) -> anyhow::Result<usize> {
    let mut all = load_shifts(db, uid);
    all.append(&mut incoming);
    if all.len() > HISTORY_CAP {
        all = all[all.len() - HISTORY_CAP..].to_vec();
    }
    save_shifts(db, uid, &all)?;
    Ok(all.len())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oee::StopReason;

    const GOOD_CSV: &str = "\
machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count,downtime_secs,stop_reason,recorded_at,shift_label
CNC-01,28800,60,400,390,1800,breakdown,2026-08-24T06:00:00Z,Turno 1
CNC-02,28800,45,500,500,,,,
";

    #[test]
    fn csv_happy_path() {
        let (records, errors) = parse_shift_csv(GOOD_CSV);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(records.len(), 2);
        let first = &records[0];
        assert_eq!(first.shift.machine_id, "CNC-01");
        assert_eq!(first.shift.stops.len(), 1);
        assert_eq!(first.shift.stops[0].reason, StopReason::Breakdown);
        assert_eq!(first.shift.stops[0].duration_secs, 1800.0);
        assert!(first.recorded_at.is_some());
        assert_eq!(first.shift_label.as_deref(), Some("Turno 1"));
        // Blank optionals stay None.
        assert!(records[1].shift.stops.is_empty());
        assert!(records[1].recorded_at.is_none());
        assert!(records[1].shift_label.is_none());
    }

    #[test]
    fn csv_partial_acceptance_reports_lines() {
        let csv = "\
machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count
CNC-01,28800,60,400,390
CNC-BAD,28800,60,500,600
CNC-02,-5,60,10,10
CNC-03,28800,0,100,90
";
        let (records, errors) = parse_shift_csv(csv);
        assert_eq!(records.len(), 1, "only the first row lands");
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].line, 3);
        assert!(errors[0].error.contains("cannot exceed total_count"));
        assert_eq!(errors[1].line, 4);
        assert!(errors[1].error.contains("non-negative"));
        assert_eq!(errors[2].line, 5);
        assert!(errors[2].error.contains("> 0"));
    }

    #[test]
    fn csv_header_errors_name_the_problem() {
        let (records, errors) = parse_shift_csv("machine_id,total\nA,1\n");
        assert!(records.is_empty());
        assert!(errors[0].error.contains("missing required column(s)"));

        let (records, errors) = parse_shift_csv(
            "machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,goodcout\nA,1,1,1,1\n",
        );
        assert!(records.is_empty(), "a typo'd header blocks everything");
        assert!(
            errors
                .iter()
                .any(|e| e.error.contains("missing required column(s): good_count")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.error.contains("unknown column(s): goodcout")),
            "{errors:?}"
        );
    }

    #[test]
    fn csv_quoted_fields_and_crlf() {
        let csv = "machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count,shift_label\r\n\
                   \"CNC, 07\",28800,60,10,9,\"Turno \"\"Notte\"\" — linea 2\"\r\n";
        let (records, errors) = parse_shift_csv(csv);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].shift.machine_id, "CNC, 07");
        assert_eq!(
            records[0].shift_label.as_deref(),
            Some("Turno \"Notte\" — linea 2")
        );
    }

    #[test]
    fn csv_stop_columns_are_mutually_required() {
        let base = "machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count";
        let (_, errors) = parse_shift_csv(&format!("{base},downtime_secs\nA,1,1,1,1,50\n"));
        assert!(errors[0].error.contains("without stop_reason"));
        let (_, errors) = parse_shift_csv(&format!("{base},stop_reason\nA,1,1,1,1,jam\n"));
        assert!(errors[0].error.contains("without downtime_secs"));
        let (_, errors) = parse_shift_csv(&format!(
            "{base},downtime_secs,stop_reason\nA,1,1,1,1,50,mystery\n"
        ));
        assert!(errors[0].error.contains("unknown stop_reason 'mystery'"));
    }

    #[test]
    fn json_import_with_provenance_and_rejects() {
        let elements = serde_json::json!([
            {
                "machine_id": "PRESS-2",
                "shift_length_secs": 14400.0,
                "ideal_cycle_time_secs": 30.0,
                "total_count": 300,
                "good_count": 295,
                "stops": [{"reason": "minor_stop", "duration_secs": 240.0}],
                "recorded_at": "2026-08-23T22:00:00Z",
                "shift_label": "Notte"
            },
            {"machine_id": "", "shift_length_secs": 1.0, "ideal_cycle_time_secs": 1.0,
             "total_count": 1, "good_count": 1},
            {"machine_id": "X", "shift_length_secs": 1.0, "ideal_cycle_time_secs": 1.0,
             "total_count": 5, "good_count": 9}
        ]);
        let (records, errors) = parse_shift_json(elements.as_array().unwrap());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].shift.machine_id, "PRESS-2");
        assert_eq!(
            records[0].recorded_at.unwrap().to_rfc3339(),
            "2026-08-23T22:00:00+00:00"
        );
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].line, 2);
        assert_eq!(errors[1].line, 3);
    }

    #[test]
    fn aggregation_sums_and_weights() {
        let (records, errors) = parse_shift_csv(GOOD_CSV);
        assert!(errors.is_empty());
        let agg = aggregate(&records);
        assert_eq!(agg.len(), 2, "one merged shift per machine");

        let (records, _) = parse_shift_csv(
            "machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count,downtime_secs,stop_reason\n\
             M1,100,10,100,90,20,breakdown\n\
             M1,100,10,100,80,0,\n",
        );
        let agg = aggregate(&records);
        assert_eq!(agg.len(), 1);
        let m = &agg[0];
        assert_eq!(m.shift_length_secs, 200.0);
        assert_eq!(m.total_count, 200);
        assert_eq!(m.good_count, 170);
        assert_eq!(m.stops.len(), 1);
        // Production-weighted cycle: both shifts identical → still 10.
        assert!((m.ideal_cycle_time_secs - 10.0).abs() < f32::EPSILON);

        let report = m.compute();
        // Availability: run_time 180 / planned 200.
        assert!(
            (report.availability - 0.9).abs() < 1e-4,
            "{}",
            report.availability
        );
        assert!((report.quality - 0.85).abs() < 1e-4, "{}", report.quality);
    }

    #[test]
    fn evidence_bundle_is_self_verifying() {
        let (records, _) = parse_shift_csv(GOOD_CSV);
        let bundle = evidence_bundle("plant", None, &records);
        assert_eq!(bundle.tenant, "plant");
        assert_eq!(bundle.shift_count, 2);
        assert_eq!(bundle.machine_count, 2);
        assert_eq!(bundle.reports.len(), bundle.records.len());
        // First record carries recorded_at → span opens at it.
        assert_eq!(
            bundle.span.as_ref().unwrap().from.to_rfc3339(),
            "2026-08-24T06:00:00+00:00"
        );
        // Per-record OEE recomputable from raw record: quality = good/total.
        assert!((bundle.reports[0].quality - 390.0 / 400.0).abs() < 1e-4);
        assert_eq!(bundle.reports[0].machine_id, "CNC-01");

        // Machine slice narrows everything consistently.
        let slice: Vec<ShiftRecord> = records
            .iter()
            .filter(|r| r.shift.machine_id == "CNC-02")
            .cloned()
            .collect();
        let sliced = evidence_bundle("plant", Some("CNC-02"), &slice);
        assert_eq!(sliced.shift_count, 1);
        assert_eq!(sliced.machine, Some("CNC-02".into()));
        assert!(sliced.span.is_none(), "undated record → no window claimed");
    }

    #[test]
    fn history_round_trips_through_sled_and_caps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        {
            let db = sled::Config::new().path(&path).open().unwrap();
            let (records, _) = parse_shift_csv(GOOD_CSV);

            append_shifts(&db, "alice", records.clone()).unwrap();
            append_shifts(&db, "bob", records[..1].to_vec()).unwrap();
            assert_eq!(load_shifts(&db, "alice").len(), 2, "per-tenant isolation");
            assert_eq!(load_shifts(&db, "bob").len(), 1);

            // Cap: flood one tenant past the ceiling, oldest fall off.
            let filler = records[0].clone();
            let flood: Vec<ShiftRecord> = (0..HISTORY_CAP).map(|_| filler.clone()).collect();
            append_shifts(&db, "alice", flood).unwrap();
            assert_eq!(load_shifts(&db, "alice").len(), HISTORY_CAP);
        }

        // Reopening the store must not lose the persisted history.
        // Retry on lock contention: under full-suite parallel load the
        // dropped handle's flock can linger briefly (PH-010 flake).
        let mut attempt = 0;
        let db = loop {
            match sled::Config::new().path(&path).open() {
                Ok(db) => break db,
                Err(_) if attempt < 20 => {
                    attempt += 1;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => panic!("reopen failed after retry: {e}"),
            }
        };
        assert_eq!(load_shifts(&db, "alice").len(), HISTORY_CAP);
        assert_eq!(load_shifts(&db, "bob").len(), 1);
    }
}
