//! Local Linux sensors, normalized into the existing event shape.
//!
//! The machine this runs on already publishes its own temperatures: thermal
//! zones under `/sys/class/thermal`, and chip sensors under
//! `/sys/class/hwmon`. Reading them needs no `lm-sensors`, no daemon and no
//! privileges — only the kernel's documented sysfs ABI.
//!
//! This is a **perception edge**, not a telemetry store. Every reading becomes
//! an existing [`OperationalEvent`] with the raw sysfs path as its provenance,
//! so the fact projection and the deterministic query answer "what is CPU
//! temperature?" from a real reading without a new type, database or graph.
//!
//! Units: `temp*_input` and `thermal_zone*/temp` are **millidegrees Celsius**
//! by kernel ABI, so they are divided by 1000 here. An implausible value (past
//! 200 °C) is dropped rather than reported, because a garbage register read is
//! worse than a missing sensor.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::fact::normalize_key;
use crate::operational::{
    OperationalEvent, OperationalKind, OperationalProvenance, OperationalStatus,
};

/// Plausibility ceiling for a converted reading, in degrees Celsius. Past this
/// the register is garbage (many drivers publish `0xFFFFFFFF` when absent).
const MAX_PLAUSIBLE_C: f64 = 200.0;

/// The host this process runs on, used as the event subject so a query can
/// constrain on the device (`for pansa`). Falls back to `localhost`.
pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".to_string())
}

/// Read every readable local temperature sensor under `root` (normally `/sys`).
///
/// `at` is passed in so the translation is deterministic and testable. The
/// result is sorted by sysfs path, so two runs over the same tree agree.
pub fn read_sysfs(root: &Path, subject: &str, at: DateTime<Utc>) -> Vec<OperationalEvent> {
    let mut readings: Vec<(PathBuf, String, f64)> = Vec::new();

    for zone in entries(&root.join("class/thermal")) {
        if !file_name_starts(&zone, "thermal_zone") {
            continue;
        }
        let temp = zone.join("temp");
        let Some(degrees) = read_millidegrees(&temp) else {
            continue;
        };
        let zone_type = read_trim(&zone.join("type")).unwrap_or_default();
        readings.push((temp, thermal_metric(&zone_type), degrees));
    }

    for chip_dir in entries(&root.join("class/hwmon")) {
        if !file_name_starts(&chip_dir, "hwmon") {
            continue;
        }
        let chip = read_trim(&chip_dir.join("name")).unwrap_or_default();
        for file in entries(&chip_dir) {
            let Some(name) = file.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.starts_with("temp") || !name.ends_with("_input") {
                continue;
            }
            let Some(degrees) = read_millidegrees(&file) else {
                continue;
            };
            let base = name.trim_end_matches("_input");
            let label = read_trim(&chip_dir.join(format!("{base}_label"))).unwrap_or_default();
            readings.push((file, hwmon_metric(&chip, &label), degrees));
        }
    }

    readings.sort_by(|a, b| a.0.cmp(&b.0));
    readings
        .into_iter()
        .map(|(path, metric, degrees)| reading_event(subject, &path, &metric, degrees, at))
        .collect()
}

/// Convenience: read the live `/sys` tree on this host.
///
/// `/sys` is hardcoded here on purpose — this is the one convenience wrapper
/// for callers that want *this* machine, while [`read_sysfs`] stays the
/// testable seam that takes a root. An empty result is logged at debug so a
/// container without sensor mounts is distinguishable from a machine that is
/// genuinely cool, without failing the caller.
pub fn local_events() -> Vec<OperationalEvent> {
    let events = read_sysfs(Path::new("/sys"), &hostname(), Utc::now());
    if events.is_empty() {
        log::debug!("no readable temperature sensors under /sys");
    }
    events
}

fn reading_event(
    subject: &str,
    path: &Path,
    metric: &str,
    degrees: f64,
    at: DateTime<Utc>,
) -> OperationalEvent {
    let source = if path.to_string_lossy().contains("/hwmon") {
        "hwmon"
    } else {
        "thermal"
    };
    let mut provenance = OperationalProvenance::new("sysfs", source);
    provenance.path = Some(path.to_path_buf());
    let mut event = OperationalEvent::new(
        OperationalKind::Machine,
        OperationalStatus::Info,
        at,
        subject,
        provenance,
    );
    event.machine_id = Some(subject.to_string());
    event
        .properties
        .insert(metric.to_string(), format!("{degrees:.1} °C"));
    event.rekey();
    event
}

/// Canonical metric name for a thermal zone. A CPU/package zone is named
/// `cpu_temperature`; anything else keeps its own type so sensors stay
/// separately named and an ambiguous query stays visible.
fn thermal_metric(zone_type: &str) -> String {
    let t = normalize_key(zone_type);
    if t.contains("x86_pkg") || t.contains("cpu") || t.contains("pkg_temp") {
        return "cpu_temperature".to_string();
    }
    if t.is_empty() {
        "thermal_zone_temperature".to_string()
    } else {
        format!("{t}_temperature")
    }
}

/// Canonical metric name for an hwmon chip sensor. The chip family decides the
/// prefix (`k10temp`/`coretemp` → cpu, `amdgpu` → gpu, `nvme` → nvme); a
/// package/control sensor is the canonical `cpu_temperature`, a per-core or
/// per-die label stays separately named. Unknown chips fall back to their own
/// name, never a guess.
fn hwmon_metric(chip: &str, label: &str) -> String {
    let c = chip.to_ascii_lowercase();
    let l = label.to_ascii_lowercase();
    let family = if c.contains("k10temp")
        || c.contains("coretemp")
        || c.contains("zenpower")
        || c.contains("cpu")
    {
        Some("cpu")
    } else if c.contains("amdgpu") || c.contains("radeon") || c.contains("nouveau") {
        Some("gpu")
    } else if c.contains("nvme") {
        Some("nvme")
    } else {
        None
    };
    let Some(family) = family else {
        return join_metric(&[chip, label, "temperature"]);
    };
    if family == "cpu" && (l.is_empty() || l.contains("tctl") || l.contains("package")) {
        return "cpu_temperature".to_string();
    }
    join_metric(&[family, &l, "temperature"])
}

fn join_metric(parts: &[&str]) -> String {
    let joined = parts
        .iter()
        .map(|p| normalize_key(p))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if joined.is_empty() {
        "temperature".to_string()
    } else {
        joined
    }
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    paths
}

fn file_name_starts(path: &Path, prefix: &str) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.starts_with(prefix))
}

fn read_trim(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read a millidegree-Celsius input and convert to degrees. Returns `None` for
/// a missing file, a non-integer, or an implausible value.
fn read_millidegrees(path: &Path) -> Option<f64> {
    let raw = read_trim(path)?;
    let milli: i64 = raw.parse().ok()?;
    let degrees = milli as f64 / 1000.0;
    if !degrees.is_finite() || degrees.abs() > MAX_PLAUSIBLE_C {
        return None;
    }
    Some(degrees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::{answer_fact, facts_from_events, FactAnswer};

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-19T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    /// A fake `/sys` tree shaped like pansa: a CPU package sensor on k10temp,
    /// an nvme composite, a wifi thermal zone, and an unreadable amdgpu edge.
    fn fake_sys() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let base = root.path();
        write(
            &base.join("class/thermal/thermal_zone0/type"),
            "iwlwifi_1\n",
        );
        write(&base.join("class/thermal/thermal_zone0/temp"), "41000\n");
        write(&base.join("class/hwmon/hwmon0/name"), "k10temp\n");
        write(&base.join("class/hwmon/hwmon0/temp1_input"), "79500\n");
        write(&base.join("class/hwmon/hwmon0/temp1_label"), "Tctl\n");
        write(&base.join("class/hwmon/hwmon0/temp3_input"), "78250\n");
        write(&base.join("class/hwmon/hwmon0/temp3_label"), "Tccd1\n");
        write(&base.join("class/hwmon/hwmon1/name"), "nvme\n");
        write(&base.join("class/hwmon/hwmon1/temp1_input"), "44850\n");
        write(&base.join("class/hwmon/hwmon1/temp1_label"), "Composite\n");
        write(&base.join("class/hwmon/hwmon2/name"), "amdgpu\n");
        write(&base.join("class/hwmon/hwmon2/temp1_input"), "\n");
        root
    }

    #[test]
    fn fake_sysfs_tree_normalizes_to_events_with_paths() {
        let root = fake_sys();
        let events = read_sysfs(root.path(), "pansa", at());
        assert_eq!(events.len(), 4, "{events:#?}");
        let cpu = events
            .iter()
            .find(|e| e.properties.contains_key("cpu_temperature"))
            .expect("k10temp Tctl is cpu_temperature");
        assert_eq!(cpu.properties["cpu_temperature"], "79.5 °C");
        assert_eq!(cpu.subject, "pansa");
        assert_eq!(cpu.machine_id.as_deref(), Some("pansa"));
        assert_eq!(cpu.provenance.connector, "sysfs");
        assert_eq!(cpu.provenance.source, "hwmon");
        assert!(cpu
            .provenance
            .path
            .as_ref()
            .unwrap()
            .ends_with("temp1_input"));
        assert!(events
            .iter()
            .any(|e| e.properties.contains_key("nvme_composite_temperature")));
        assert!(events
            .iter()
            .any(|e| e.properties.contains_key("iwlwifi_1_temperature")));
    }

    #[test]
    fn implausible_and_unreadable_inputs_are_dropped() {
        let root = fake_sys();
        let base = root.path();
        write(&base.join("class/hwmon/hwmon3/name"), "garbage\n");
        write(&base.join("class/hwmon/hwmon3/temp1_input"), "4294967295\n");
        write(&base.join("class/hwmon/hwmon4/name"), "notanumber\n");
        write(&base.join("class/hwmon/hwmon4/temp1_input"), "abc\n");
        let events = read_sysfs(root.path(), "pansa", at());
        assert_eq!(events.len(), 4, "garbage sensors add nothing: {events:#?}");
    }

    #[test]
    fn projection_and_query_answer_cpu_temperature_from_sysfs() {
        let root = fake_sys();
        let facts = facts_from_events(&read_sysfs(root.path(), "pansa", at()));
        match answer_fact("what is CPU temperature?", &facts) {
            FactAnswer::One(f) => {
                assert_eq!(f.value, "79.5 °C");
                assert_eq!(f.subject.as_deref(), Some("pansa"));
                assert!(f.source.as_deref().unwrap().ends_with("temp1_input"));
            }
            other => panic!("expected one cpu temperature, got {other:?}"),
        }
    }

    #[test]
    #[ignore = "reads the live /sys tree on this host"]
    fn live_smoke_answers_cpu_temperature_from_this_host() {
        let events = local_events();
        if events.is_empty() {
            eprintln!("no local sensors exposed; skipping live assertion");
            return;
        }
        for e in &events {
            assert!(
                e.provenance.path.is_some(),
                "every live reading keeps its sysfs path: {e:?}"
            );
        }
        let facts = facts_from_events(&events);
        let answer = answer_fact("what is CPU temperature?", &facts);
        eprintln!("live CPU answer: {:?}", answer.render());
        if facts.iter().any(|f| f.key == "cpu_temperature") {
            assert!(matches!(answer, FactAnswer::One(_)), "{answer:?}");
        }
    }

    #[test]
    fn several_sensors_of_one_family_stay_separately_named() {
        assert_eq!(hwmon_metric("k10temp", "Tctl"), "cpu_temperature");
        assert_eq!(hwmon_metric("k10temp", "Tccd1"), "cpu_tccd1_temperature");
        assert_eq!(hwmon_metric("amdgpu", "edge"), "gpu_edge_temperature");
        assert_eq!(
            hwmon_metric("nvme", "Composite"),
            "nvme_composite_temperature"
        );
        assert_eq!(hwmon_metric("asus", "CPU Fan"), "asus_cpu_fan_temperature");
    }
}
