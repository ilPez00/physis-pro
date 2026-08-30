//! Vendor-side licence tooling: generate a signing keypair, mint licences,
//! inspect what a customer was actually issued.
//!
//! This binary is gated behind the `vendor` feature and is never built into a
//! customer package. Keep the private key off build machines: it is the only
//! secret that matters in this scheme, and it cannot be rotated without
//! reissuing every outstanding licence.
//!
//! Typical vendor flow:
//!
//! ```text
//! physis-license keygen --out ~/.physis-vendor
//! # → prints the public key; stamp it into release builds:
//! PHYSIS_LICENSE_PUBKEY=<hex> cargo build --release
//!
//! physis-license sign --key ~/.physis-vendor/signing.key \
//!     --org "Evotec SpA" --seats 25 --features rag,mcp \
//!     --exp 2027-08-15 --maint-exp 2027-08-15 > evotec.license.key
//! ```

use std::path::PathBuf;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signer, SigningKey};

#[derive(Parser)]
#[command(
    name = "physis-license",
    about = "Vendor licence tooling (not for distribution)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a fresh Ed25519 signing keypair.
    Keygen {
        /// Directory to write `signing.key` (private) and `verifying.key` (public).
        #[arg(long, default_value = ".physis-vendor")]
        out: PathBuf,
    },
    /// Mint a signed licence blob on stdout.
    Sign {
        /// Path to the private signing key produced by `keygen`.
        #[arg(long)]
        key: PathBuf,
        /// Licensee legal name, as it should appear in the startup banner.
        #[arg(long)]
        org: String,
        /// Commercial SKU: core | industrial | enterprise. Expands to the
        /// SKU's feature bundle; `--features` overrides when given.
        #[arg(long)]
        sku: Option<String>,
        /// Alias for `--sku`, kept for compatibility.
        #[arg(long, hide = true)]
        tier: Option<String>,
        /// Licensed seat/host count.
        #[arg(long, default_value = "1")]
        seats: u32,
        /// Comma-separated feature slugs, or `*` for everything.
        /// Overrides `--sku` when present.
        #[arg(long)]
        features: Option<String>,
        /// Expiry of the right to run, `YYYY-MM-DD`.
        #[arg(long)]
        exp: Option<String>,
        /// Expiry of the right to install newer builds, `YYYY-MM-DD`.
        /// Defaults to `--exp` when omitted.
        #[arg(long)]
        maint_exp: Option<String>,
        /// Vendor-side licence id. Generated from org + exp when omitted.
        #[arg(long)]
        id: Option<String>,
        /// Mint a 14-day evaluation licence: expires two weeks from today,
        /// maintenance included, id prefixed TRIAL- so it is traceable.
        #[arg(long)]
        trial: bool,
    },
    /// Print the contents of a licence blob without verifying its signature.
    /// Useful for support: "what did we actually issue this customer?"
    Inspect {
        /// Path to a licence file.
        path: PathBuf,
    },
    /// Revoke a licence by ID, adding it to a revocation list the vendor ships
    /// to air-gapped hosts. Each host refuses to start once it has a copy of
    /// the list containing that licence's id.
    Revoke {
        /// Licence ID to revoke (format: PX-ORG-YYYYMMDD).
        #[arg(long)]
        id: String,
        /// Revocation list to append to (created if missing). The default
        /// matches the runtime default: `$PHYSIS_DATA_DIR/revocations.json`
        /// (else `./.physis/revocations.json`).
        #[arg(long)]
        list: Option<PathBuf>,
    },
    /// Print the orderable SKU → feature-bundle table for the order form.
    ListSkus,
    /// Sign a release artifact, emitting a detached signature on stdout.
    /// Air-gapped customers verify the tarball with this before unpacking.
    SignArtifact {
        /// Private release key (distinct from the licence signing key).
        #[arg(long)]
        key: PathBuf,
        /// File to sign.
        #[arg(long)]
        file: PathBuf,
    },
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Keygen { out } => {
            use rand_core::OsRng;

            std::fs::create_dir_all(&out)?;
            let signing = SigningKey::generate(&mut OsRng);
            let verifying = signing.verifying_key();

            let priv_path = out.join("signing.key");
            let pub_path = out.join("verifying.key");
            std::fs::write(&priv_path, hex(&signing.to_bytes()))?;
            std::fs::write(&pub_path, hex(&verifying.to_bytes()))?;

            // The private key is the whole security model — make it unreadable
            // to anyone but the owner before we even report success.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&priv_path, std::fs::Permissions::from_mode(0o600))?;
            }

            println!(
                "private key : {}  (KEEP OFFLINE — never commit, never on CI)",
                priv_path.display()
            );
            println!("public key  : {}", pub_path.display());
            println!();
            println!("Stamp this into release builds:");
            println!(
                "  PHYSIS_LICENSE_PUBKEY={} cargo build --release",
                hex(&verifying.to_bytes())
            );
        }

        Command::Sign {
            key,
            org,
            sku,
            tier,
            seats,
            features,
            exp,
            maint_exp,
            id,
            trial,
        } => {
            let key_hex = std::fs::read_to_string(&key)?;
            let key_bytes: [u8; 32] = decode_hex(key_hex.trim())
                .ok_or_else(|| anyhow::anyhow!("signing key is not 32 hex-encoded bytes"))?;
            let signing = SigningKey::from_bytes(&key_bytes);

            // Feature list: explicit --features wins; else expand the SKU
            // bundle; else default to Enterprise (`*`) so ad-hoc internal
            // licences keep working exactly as before the flag existed.
            let features: Vec<String> = if let Some(f) = features {
                f.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else if let Some(t) = sku.or(tier) {
                let bundle = physis_pro::license::Tier::from_sku(&t)
                    .ok_or_else(|| anyhow::anyhow!(
                        "unknown SKU '{t}' — run `physis-license list-skus` (core|industrial|enterprise)"
                    ))?;
                bundle.features().iter().map(|s| s.to_string()).collect()
            } else {
                vec!["*".to_string()]
            };

            let today = chrono::Utc::now().date_naive();
            let (exp, maint_exp, license_id, trial) = if trial {
                let e = today + chrono::Duration::days(14);
                let e = e.to_string();
                let id = id.unwrap_or_else(|| format!("TRIAL-{}", e.replace('-', "")));
                (e.clone(), e, id, true)
            } else {
                let e = exp.ok_or_else(|| {
                    anyhow::anyhow!("--exp YYYY-MM-DD is required unless --trial is used")
                })?;
                let maint = maint_exp.unwrap_or_else(|| e.clone());
                let id = id.unwrap_or_else(|| {
                    let slug: String = org
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .take(8)
                        .collect::<String>()
                        .to_uppercase();
                    format!("PX-{slug}-{}", e.replace('-', ""))
                });
                (e, maint, id, false)
            };

            let payload = serde_json::json!({
                "license_id": license_id,
                "org": org,
                "seats": seats,
                "features": features,
                "iat": today.to_string(),
                "exp": exp,
                "maint_exp": maint_exp,
                "revoked": false,
            });

            // Sign the exact bytes we will ship; never re-serialise downstream.
            let payload_bytes = serde_json::to_vec(&payload)?;
            let sig = signing.sign(&payload_bytes);

            let envelope = serde_json::json!({
                "payload": B64.encode(&payload_bytes),
                "sig": B64.encode(sig.to_bytes()),
            });
            println!("{}", serde_json::to_string_pretty(&envelope)?);
            eprintln!(
                "signed {} for {} — runs until {}, updates until {}{}",
                license_id,
                org,
                exp,
                maint_exp,
                if trial { " (TRIAL — 14 days)" } else { "" }
            );
        }

        Command::SignArtifact { key, file } => {
            let key_hex = std::fs::read_to_string(&key)?;
            let key_bytes: [u8; 32] = decode_hex(key_hex.trim())
                .ok_or_else(|| anyhow::anyhow!("release key is not 32 hex-encoded bytes"))?;
            let signing = SigningKey::from_bytes(&key_bytes);

            // Sign the artifact bytes directly. Signing a hash instead would add
            // a second algorithm to agree on with no security gain — Ed25519
            // already hashes internally.
            let bytes = std::fs::read(&file)?;
            let sig = signing.sign(&bytes);

            println!("{}", B64.encode(sig.to_bytes()));
            eprintln!(
                "signed {} ({} bytes) with public key {}",
                file.display(),
                bytes.len(),
                hex(&signing.verifying_key().to_bytes())
            );
        }

        Command::Revoke { id, list } => {
            use std::collections::BTreeSet;
            // Read-modify-write the revocation list. Sorted, so the file stays
            // stable across runs and easy to diff when the vendor ships it.
            let path = list.unwrap_or_else(|| {
                std::env::var("PHYSIS_DATA_DIR")
                    .ok()
                    .map(PathBuf::from)
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| PathBuf::from(".physis"))
                    .join("revocations.json")
            });
            let mut revoked: BTreeSet<String> = std::fs::read_to_string(&path)
                .ok()
                .and_then(|blob| serde_json::from_str::<serde_json::Value>(&blob).ok())
                .and_then(|v| v.get("revoked").cloned())
                .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                .unwrap_or_default()
                .into_iter()
                .collect();
            let inserted = revoked.insert(id.clone());
            let doc = serde_json::json!({ "revoked": revoked.iter().collect::<Vec<_>>() });
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, serde_json::to_string_pretty(&doc)?)?;

            if inserted {
                eprintln!("revoked {id} — appended to {}", path.display());
                eprintln!(
                    "ship this file to affected air-gapped hosts (with a release, \
                     or as a drop-in); each host refuses to start once the list \
                     reaches it and contains this id"
                );
            } else {
                eprintln!("{id} was already on {}", path.display());
            }
        }

        Command::ListSkus => {
            for ent in physis_pro::license::ENTITLEMENTS {
                println!(
                    "{}  {:<12} {}",
                    ent.sku,
                    ent.label,
                    ent.tier.features().join(", ")
                );
            }
        }

        Command::Inspect { path } => {
            let blob = std::fs::read_to_string(&path)?;
            let envelope: serde_json::Value = serde_json::from_str(&blob)?;
            let payload = envelope
                .get("payload")
                .and_then(|p| p.as_str())
                .ok_or_else(|| anyhow::anyhow!("no payload field"))?;
            let decoded = B64.decode(payload)?;
            let parsed: serde_json::Value = serde_json::from_slice(&decoded)?;
            println!("{}", serde_json::to_string_pretty(&parsed)?);
            println!();
            println!("NOTE: signature not checked — use the product binary to validate.");
        }
    }
    Ok(())
}

fn decode_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let v: Option<Vec<u8>> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect();
    v?.try_into().ok()
}
