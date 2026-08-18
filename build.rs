use std::process::Command;

/// Stamp the build date into the binary so the licence gate can decide whether a
/// customer's maintenance window covers *this* build (see `src/license.rs`).
///
/// Without this, a lapsed-maintenance customer could keep installing newer
/// releases forever: the licence would still validate, because nothing in it
/// would know how new the binary is.
fn main() {
    let date = Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());

    println!("cargo:rustc-env=PHYSIS_BUILD_DATE={date}");

    // Re-stamp whenever the licence gate itself changes, so a rebuild of the
    // gate can never reuse a stale date from an earlier build.
    println!("cargo:rerun-if-changed=src/license.rs");
    println!("cargo:rerun-if-env-changed=PHYSIS_LICENSE_PUBKEY");
}
