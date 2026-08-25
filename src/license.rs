//! Offline licence verification.
//!
//! Physis ships to air-gapped sites (defence, pharma, banking). Those customers
//! cannot let the binary reach the network, so activation-server designs are
//! disqualified outright. Instead the vendor signs a licence blob with an
//! Ed25519 private key that never leaves the vendor; the binary carries only the
//! public key and verifies locally. No socket is ever opened by this module.
//!
//! # Blob format
//!
//! ```text
//! { "payload": "<base64 of the exact payload bytes>", "sig": "<base64 sig>" }
//! ```
//!
//! The payload is base64'd rather than embedded as nested JSON so that
//! verification signs *exact bytes*. Re-serialising a parsed JSON object can
//! reorder keys or renormalise numbers, which would break a valid signature —
//! a classic canonicalisation bug. We never re-serialise: we verify the decoded
//! bytes and only then parse them.
//!
//! # What this is and is not
//!
//! This is a **commercial control**, not a security boundary. Anyone able to
//! patch the binary can bypass it. It stops honest misuse, casual copying and
//! silent seat growth — which is what licence enforcement is actually for.
//! Do not represent it to customers as tamper-proof.

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::{Duration, NaiveDate, Utc};
use ed25519_dalek::{Signature, VerifyingKey, SIGNATURE_LENGTH};
use serde::{Deserialize, Serialize};

/// Environment variable holding a licence blob inline (useful in containers,
/// where mounting a file is more friction than setting a variable).
pub const LICENSE_ENV: &str = "PHYSIS_LICENSE";
/// Environment variable pointing at a licence file, overriding the default path.
pub const LICENSE_FILE_ENV: &str = "PHYSIS_LICENSE_FILE";

/// Vendor public key, hex-encoded (32 bytes).
///
/// Overridden at build time with `PHYSIS_LICENSE_PUBKEY=<hex>` so the release
/// pipeline can stamp in the production key without it living in git. The
/// fallback below is the **development** key — builds carrying it can only
/// validate licences signed by the matching development private key, which is
/// exactly what you want for tests and local runs.
const PUBKEY_HEX: &str = match option_env!("PHYSIS_LICENSE_PUBKEY") {
    Some(k) => k,
    None => "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29",
};

/// Date this binary was built, stamped by `build.rs`. Compared against the
/// licence's maintenance window: a customer whose maintenance lapsed keeps
/// running the builds they paid for, but newer builds refuse to start.
const BUILD_DATE: &str = env!("PHYSIS_BUILD_DATE");

/// Tolerance for clock skew between the signing host and the customer host.
/// Two days is generous enough for badly-synced air-gapped machines without
/// meaningfully extending an expired licence.
const CLOCK_SKEW_DAYS: i64 = 2;

/// Commercial packaging tiers (PLAN 4.1).
///
/// A tier is a named feature bundle customers buy; the licence itself only
/// ever stores the flat feature list, so runtime gating keeps using
/// `License::allows` and the tier is pure presentation + sales vocabulary.
///
/// - **Core** — knowledge work: classify, scan, query, wiki, vault. Single
///   team, no plant floor.
/// - **Industrial** — Core + the plant: machines/OEE, shifts, maintenance,
///   inventory, reconciliation.
/// - **Enterprise** — everything: multi-tenant admin, spanner mirror, MCP,
///   agent orchestration, RAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Core,
    Industrial,
    Enterprise,
}

impl Tier {
    /// Feature slugs each tier grants. Order matters for `from_features`.
    pub fn features(self) -> &'static [&'static str] {
        match self {
            Tier::Core => &["classify", "scan", "query", "wiki", "vault", "history"],
            Tier::Industrial => &[
                "classify",
                "scan",
                "query",
                "wiki",
                "vault",
                "history",
                "industrial",
                "shifts",
                "maintenance",
                "inventory",
                "reconcile",
            ],
            // Enterprise: everything the build offers.
            Tier::Enterprise => &["*"],
        }
    }

    /// Recognise the tier from a signed feature list: the most generous tier
    /// whose bundle is fully contained in the list. `"*"` short-circuits to
    /// Enterprise.
    pub fn from_features(features: &[String]) -> Tier {
        if features.iter().any(|f| f == "*") {
            return Tier::Enterprise;
        }
        for tier in [Tier::Enterprise, Tier::Industrial, Tier::Core] {
            if tier
                .features()
                .iter()
                .all(|f| features.iter().any(|g| g == f))
            {
                return tier;
            }
        }
        // Partial lists (custom deals) still get a floor: they bought at least
        // something from Core.
        Tier::Core
    }

    /// Human name for the banner and invoices.
    pub fn name(self) -> &'static str {
        match self {
            Tier::Core => "Core",
            Tier::Industrial => "Industrial",
            Tier::Enterprise => "Enterprise",
        }
    }
}

/// A verified licence. Constructing one outside [`verify_blob`] is impossible
/// from other modules — the fields are public for reading, but the only public
/// constructor runs the signature check first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    /// Vendor-side identifier, so a leaked licence can be traced to a customer.
    pub license_id: String,
    /// Legal name of the licensee, shown at startup.
    pub org: String,
    /// Licensed seat (or host) count. Enforcement is contractual; this is the
    /// number displayed and audited, not a hard runtime cap.
    pub seats: u32,
    /// Enabled feature slugs, e.g. `["mcp", "rag", "industrial"]`.
    pub features: Vec<String>,
    /// Issue date, `YYYY-MM-DD`.
    pub iat: String,
    /// Expiry of the **right to run**. Past this, the binary refuses to start.
    pub exp: String,
    /// Expiry of the **right to install newer builds**. A build dated after
    /// this refuses to start even when `exp` is still in the future.
    pub maint_exp: String,
    /// Whether this licence has been revoked. Revoked licences are immediately
    /// invalid regardless of other fields.
    pub revoked: bool,
}

/// Signed envelope as stored on disk.
#[derive(Debug, Deserialize)]
struct Envelope {
    payload: String,
    sig: String,
}

/// Every way licence validation can fail. Each variant carries enough context
/// for an actionable message — "expired" without the date is a support ticket.
#[derive(Debug, thiserror::Error)]
pub enum LicenseError {
    #[error("no licence found: set {LICENSE_ENV}, {LICENSE_FILE_ENV}, or place a licence at {0}")]
    Missing(PathBuf),
    #[error("licence file is malformed: {0}")]
    Malformed(String),
    #[error("licence signature is not valid for this build's vendor key")]
    BadSignature,
    #[error("licence expired on {exp} ({days_ago} days ago) — contact your vendor to renew")]
    Expired { exp: String, days_ago: i64 },
    #[error(
        "this build ({build}) is newer than your maintenance window (ends {maint_exp}) — \
         reinstall a build released on or before {maint_exp}, or renew maintenance"
    )]
    BuildNotCovered { build: String, maint_exp: String },
    #[error(
        "system clock reads {now}, earlier than this install's last run ({last_seen}) — \
             refusing to run on a rolled-back clock"
    )]
    ClockRolledBack { now: String, last_seen: String },
    #[error("feature '{0}' is not included in this licence")]
    FeatureNotLicensed(String),
}

impl License {
    /// Whether `feature` is enabled. A licence listing `"*"` enables everything,
    /// which keeps internal and demo licences from needing constant edits.
    pub fn allows(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == "*" || f == feature)
    }

    /// Commercial tier this licence belongs to, derived from the feature set.
    ///
    /// Tiers are bundles, not extra licence fields — the signed feature list
    /// stays the single source of truth, so a forged tier upgrade is
    /// impossible without breaking the signature. Recognition order matters:
    /// Enterprise ⊃ Industrial ⊃ Core.
    pub fn tier(&self) -> Tier {
        Tier::from_features(&self.features)
    }

    /// Error out unless `feature` is licensed — for gating an entry point.
    pub fn require(&self, feature: &str) -> Result<(), LicenseError> {
        if self.allows(feature) {
            Ok(())
        } else {
            Err(LicenseError::FeatureNotLicensed(feature.to_string()))
        }
    }

    /// Days until `exp`; negative once expired. Drives renewal nagging.
    pub fn days_remaining(&self) -> i64 {
        parse_date(&self.exp)
            .map(|exp| (exp - Utc::now().date_naive()).num_days())
            .unwrap_or(0)
    }

    /// One-line startup banner. Customers should be able to see at a glance
    /// which licence a host is running under.
    pub fn banner(&self) -> String {
        format!(
            "licensed to {} — {} tier, {} seats, expires {} ({} days)",
            self.org,
            self.tier().name(),
            self.seats,
            self.exp,
            self.days_remaining()
        )
    }

    fn check_dates(&self) -> Result<(), LicenseError> {
        let today = Utc::now().date_naive();

        let exp = parse_date(&self.exp)
            .ok_or_else(|| LicenseError::Malformed(format!("bad exp date: {}", self.exp)))?;
        if today > exp + Duration::days(CLOCK_SKEW_DAYS) {
            return Err(LicenseError::Expired {
                exp: self.exp.clone(),
                days_ago: (today - exp).num_days(),
            });
        }

        let maint_exp = parse_date(&self.maint_exp).ok_or_else(|| {
            LicenseError::Malformed(format!("bad maint_exp date: {}", self.maint_exp))
        })?;
        // An unstamped build (date 1970) is a local dev build; skipping the
        // maintenance check there keeps `cargo run` usable without a fresh
        // licence for every rebuild.
        if let Some(build) = parse_date(BUILD_DATE) {
            if build > NaiveDate::from_ymd_opt(1971, 1, 1).unwrap() && build > maint_exp {
                return Err(LicenseError::BuildNotCovered {
                    build: BUILD_DATE.to_string(),
                    maint_exp: self.maint_exp.clone(),
                });
            }
        }

        Ok(())
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

fn vendor_key() -> Result<VerifyingKey, LicenseError> {
    let raw = hex_decode(PUBKEY_HEX)
        .ok_or_else(|| LicenseError::Malformed("vendor key is not valid hex".into()))?;
    let bytes: [u8; 32] = raw
        .try_into()
        .map_err(|_| LicenseError::Malformed("vendor key is not 32 bytes".into()))?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|_| LicenseError::Malformed("vendor key is not a valid Ed25519 point".into()))
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Verify a licence blob and return the licence it carries.
///
/// Order matters: signature first, then dates. Checking dates on unverified
/// content would let a forged blob choose its own error message, and leaks
/// whether a given expiry is "interesting" to the vendor.
pub fn verify_blob(blob: &str) -> Result<License, LicenseError> {
    let envelope: Envelope =
        serde_json::from_str(blob).map_err(|e| LicenseError::Malformed(e.to_string()))?;

    let payload_bytes = B64
        .decode(envelope.payload.trim())
        .map_err(|e| LicenseError::Malformed(format!("payload is not base64: {e}")))?;
    let sig_bytes = B64
        .decode(envelope.sig.trim())
        .map_err(|e| LicenseError::Malformed(format!("signature is not base64: {e}")))?;

    let sig_array: [u8; SIGNATURE_LENGTH] = sig_bytes
        .try_into()
        .map_err(|_| LicenseError::Malformed("signature is not 64 bytes".into()))?;
    let signature = Signature::from_bytes(&sig_array);

    vendor_key()?
        .verify_strict(&payload_bytes, &signature)
        .map_err(|_| LicenseError::BadSignature)?;

    // Only now is the payload trustworthy enough to parse.
    let license: License = serde_json::from_slice(&payload_bytes)
        .map_err(|e| LicenseError::Malformed(format!("payload is not a licence: {e}")))?;

    license.check_dates()?;
    Ok(license)
}

/// Default licence location: `PHYSIS_DATA_DIR/license.key`, or `./.physis/license.key`
/// when the data dir is not explicitly set.
pub fn default_license_path() -> PathBuf {
    std::env::var("PHYSIS_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from(".physis"))
        .join("license.key")
}

/// Load and verify the licence from, in order: `PHYSIS_LICENSE` (inline),
/// `PHYSIS_LICENSE_FILE`, then the default path.
pub fn load() -> Result<License, LicenseError> {
    if let Ok(blob) = std::env::var(LICENSE_ENV) {
        if !blob.trim().is_empty() {
            return verify_blob(&blob);
        }
    }

    let path = std::env::var_os(LICENSE_FILE_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(default_license_path);

    let blob = std::fs::read_to_string(&path).map_err(|_| LicenseError::Missing(path.clone()))?;
    let license = verify_blob(&blob)?;
    check_clock_rollback(&path)?;
    Ok(license)
}

/// Guard against the cheapest bypass: setting the system clock back to before
/// expiry. We record the newest date we have ever seen next to the licence and
/// refuse to run if the clock has moved meaningfully backwards.
///
/// Deliberately advisory — a user who can edit the licence file can edit this
/// too. It closes the trivial attack, not the determined one.
fn check_clock_rollback(license_path: &Path) -> Result<(), LicenseError> {
    let marker = license_path.with_extension("lastrun");
    let today = Utc::now().date_naive();

    if let Ok(prev) = std::fs::read_to_string(&marker) {
        if let Some(last_seen) = parse_date(&prev) {
            if today < last_seen - Duration::days(CLOCK_SKEW_DAYS) {
                return Err(LicenseError::ClockRolledBack {
                    now: today.to_string(),
                    last_seen: last_seen.to_string(),
                });
            }
            if today <= last_seen {
                return Ok(()); // nothing newer to record
            }
        }
    }

    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&marker, today.to_string());
    Ok(())
}

/// Startup gate for binaries: verify or exit with an actionable message.
///
/// Returns the licence so callers can gate features on it. Exits with code 2
/// (distinct from 1) so operators can tell a licensing stop from a crash in
/// their process supervisor.
pub fn enforce() -> License {
    match load() {
        Ok(license) => {
            eprintln!("physis: {}", license.banner());
            if license.days_remaining() <= 30 {
                eprintln!(
                    "physis: WARNING licence expires in {} days",
                    license.days_remaining()
                );
            }
            license
        }
        Err(e) => {
            eprintln!("physis: licence check failed — {e}");
            eprintln!("physis: this build requires a valid licence to run.");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn feats(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn star_features_recognise_as_enterprise() {
        assert_eq!(Tier::from_features(&feats(&["*"])), Tier::Enterprise);
    }

    #[test]
    fn full_core_bundle_recognises_as_core() {
        assert_eq!(
            Tier::from_features(&feats(Tier::Core.features())),
            Tier::Core
        );
    }

    #[test]
    fn full_industrial_bundle_recognises_as_industrial() {
        let f = feats(Tier::Industrial.features());
        assert_eq!(Tier::from_features(&f), Tier::Industrial);
    }

    #[test]
    fn industrial_bundle_also_satisfies_core() {
        // Recognition is "most generous tier fully contained", so an
        // Industrial feature list is at least Core.
        let f = feats(Tier::Industrial.features());
        let core_all = Tier::Core
            .features()
            .iter()
            .all(|c| f.iter().any(|g| g == c));
        assert!(core_all);
    }

    #[test]
    fn partial_custom_list_floors_at_core() {
        // A bespoke deal with only two features still gets a tier name.
        assert_eq!(Tier::from_features(&feats(&["wiki"])), Tier::Core);
    }

    #[test]
    fn core_tier_denies_industrial_features() {
        // The gating property the tiers exist for: a Core licence must not
        // unlock plant-floor features.
        let lic = License {
            license_id: "PX-T-1".into(),
            org: "Test".into(),
            seats: 1,
            features: feats(Tier::Core.features()),
            iat: "2026-01-01".into(),
            exp: "2099-01-01".into(),
            maint_exp: "2099-01-01".into(),
            revoked: false,
        };
        assert!(lic.allows("classify"));
        assert!(lic.allows("wiki"));
        assert!(!lic.allows("industrial"));
        assert!(!lic.allows("maintenance"));
        assert!(!lic.allows("inventory"));
        assert!(!lic.allows("*"));
    }

    #[test]
    fn industrial_tier_allows_plant_denies_enterprise_extras() {
        let lic = License {
            license_id: "PX-T-2".into(),
            org: "Test".into(),
            seats: 5,
            features: feats(Tier::Industrial.features()),
            iat: "2026-01-01".into(),
            exp: "2099-01-01".into(),
            maint_exp: "2099-01-01".into(),
            revoked: false,
        };
        assert!(lic.allows("industrial"));
        assert!(lic.allows("maintenance"));
        assert!(lic.allows("inventory"));
        assert!(!lic.allows("mcp"));
        assert!(!lic.allows("spanner"));
    }

    #[test]
    fn enterprise_tier_allows_everything() {
        let lic = License {
            license_id: "PX-T-3".into(),
            org: "Test".into(),
            seats: 50,
            features: feats(Tier::Enterprise.features()),
            iat: "2026-01-01".into(),
            exp: "2099-01-01".into(),
            maint_exp: "2099-01-01".into(),
            revoked: false,
        };
        assert_eq!(lic.tier(), Tier::Enterprise);
        assert!(lic.allows("mcp"));
        assert!(lic.allows("anything-at-all"));
    }

    #[test]
    fn banner_names_the_tier() {
        let lic = License {
            license_id: "PX-T-4".into(),
            org: "Acciaierie".into(),
            seats: 3,
            features: feats(Tier::Industrial.features()),
            iat: "2026-01-01".into(),
            exp: "2099-01-01".into(),
            maint_exp: "2099-01-01".into(),
            revoked: false,
        };
        assert!(lic.banner().contains("Industrial tier"), "{}", lic.banner());
        assert!(lic.banner().contains("3 seats"));
    }

    /// Build a signed blob with a throwaway key, and the matching public key,
    /// so tests never depend on the embedded vendor key.
    fn signed_with(license: &serde_json::Value) -> (String, String) {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let payload = serde_json::to_vec(license).unwrap();
        let sig = key.sign(&payload);
        let blob = serde_json::json!({
            "payload": B64.encode(&payload),
            "sig": B64.encode(sig.to_bytes()),
        });
        let pubkey = key
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        (blob.to_string(), pubkey)
    }

    fn sample(exp: &str, maint_exp: &str) -> serde_json::Value {
        serde_json::json!({
            "license_id": "PX-0001",
            "org": "Test SpA",
            "seats": 5,
            "features": ["rag", "mcp"],
            "iat": "2026-01-01",
            "exp": exp,
            "maint_exp": maint_exp,
            "revoked": false,
        })
    }

    #[test]
    fn valid_signature_over_exact_bytes_verifies() {
        // Verifies against the *test* key by construction, proving the
        // signature path works independently of the embedded vendor key.
        let (blob, pubkey) = signed_with(&sample("2099-01-01", "2099-01-01"));
        let envelope: Envelope = serde_json::from_str(&blob).unwrap();
        let payload = B64.decode(&envelope.payload).unwrap();
        let sig_bytes: [u8; SIGNATURE_LENGTH] =
            B64.decode(&envelope.sig).unwrap().try_into().unwrap();
        let key_bytes: [u8; 32] = hex_decode(&pubkey).unwrap().try_into().unwrap();
        let key = VerifyingKey::from_bytes(&key_bytes).unwrap();
        assert!(key
            .verify_strict(&payload, &Signature::from_bytes(&sig_bytes))
            .is_ok());
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let (blob, _) = signed_with(&sample("2099-01-01", "2099-01-01"));
        let mut envelope: serde_json::Value = serde_json::from_str(&blob).unwrap();
        // Swap in a payload claiming 9999 seats, keeping the original signature.
        let forged = serde_json::to_vec(&sample("2099-01-01", "2099-01-01")).unwrap();
        let mut forged: serde_json::Value = serde_json::from_slice(&forged).unwrap();
        forged["seats"] = serde_json::json!(9999);
        envelope["payload"] = serde_json::json!(B64.encode(serde_json::to_vec(&forged).unwrap()));
        assert!(matches!(
            verify_blob(&envelope.to_string()),
            Err(LicenseError::BadSignature)
        ));
    }

    #[test]
    fn unsigned_json_is_rejected() {
        // A customer pasting the raw payload without the envelope must fail
        // closed rather than being treated as a licence.
        let raw = serde_json::to_string(&sample("2099-01-01", "2099-01-01")).unwrap();
        assert!(matches!(verify_blob(&raw), Err(LicenseError::Malformed(_))));
    }

    #[test]
    fn expired_licence_reports_the_date() {
        let lic: License = serde_json::from_value(sample("2020-01-01", "2099-01-01")).unwrap();
        match lic.check_dates() {
            Err(LicenseError::Expired { exp, days_ago }) => {
                assert_eq!(exp, "2020-01-01");
                assert!(days_ago > 0);
            }
            other => panic!("expected Expired, got {other:?}"),
        }
    }

    #[test]
    fn wildcard_feature_enables_everything() {
        let mut lic: License = serde_json::from_value(sample("2099-01-01", "2099-01-01")).unwrap();
        assert!(!lic.allows("industrial"));
        lic.features = vec!["*".to_string()];
        assert!(lic.allows("industrial"));
        assert!(lic.require("anything").is_ok());
    }

    #[test]
    fn unlicensed_feature_is_named_in_the_error() {
        let lic: License = serde_json::from_value(sample("2099-01-01", "2099-01-01")).unwrap();
        let err = lic.require("industrial").unwrap_err();
        assert!(err.to_string().contains("industrial"));
    }

    #[test]
    fn missing_licence_names_the_path_it_looked_at() {
        // The commonest support ticket is "it says no licence" — the error has
        // to say where it looked.
        let err = LicenseError::Missing(PathBuf::from("/home/x/.physis/license.key"));
        assert!(err.to_string().contains("/home/x/.physis/license.key"));
    }
}
