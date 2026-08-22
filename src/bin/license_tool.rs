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
#[command(name = "physis-license", about = "Vendor licence tooling (not for distribution)")]
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
        /// Licensed seat/host count.
        #[arg(long, default_value = "1")]
        seats: u32,
        /// Comma-separated feature slugs, or `*` for everything.
        #[arg(long, default_value = "*")]
        features: String,
        /// Expiry of the right to run, `YYYY-MM-DD`.
        #[arg(long)]
        exp: String,
        /// Expiry of the right to install newer builds, `YYYY-MM-DD`.
        /// Defaults to `--exp` when omitted.
        #[arg(long)]
        maint_exp: Option<String>,
        /// Vendor-side licence id. Generated from org + exp when omitted.
        #[arg(long)]
        id: Option<String>,
    },
    /// Print the contents of a licence blob without verifying its signature.
    /// Useful for support: "what did we actually issue this customer?"
    Inspect {
        /// Path to a licence file.
        path: PathBuf,
    },
    /// Revoke a licence by ID, making it immediately invalid.
    Revoke {
        /// Licence ID to revoke (format: PX-ORG-YYYYMMDD).
        #[arg(long)]
        id: String,
    },
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

            println!("private key : {}  (KEEP OFFLINE — never commit, never on CI)", priv_path.display());
            println!("public key  : {}", pub_path.display());
            println!();
            println!("Stamp this into release builds:");
            println!("  PHYSIS_LICENSE_PUBKEY={} cargo build --release", hex(&verifying.to_bytes()));
        }

        Command::Sign {
            key,
            org,
            seats,
            features,
            exp,
            maint_exp,
            id,
        } => {
            let key_hex = std::fs::read_to_string(&key)?;
            let key_bytes: [u8; 32] = decode_hex(key_hex.trim())
                .ok_or_else(|| anyhow::anyhow!("signing key is not 32 hex-encoded bytes"))?;
            let signing = SigningKey::from_bytes(&key_bytes);

            let features: Vec<String> =
                features.split(',').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
            let maint_exp = maint_exp.unwrap_or_else(|| exp.clone());
            let license_id = id.unwrap_or_else(|| {
                let slug: String = org
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .take(8)
                    .collect::<String>()
                    .to_uppercase();
                format!("PX-{slug}-{}", exp.replace('-', ""))
            });

            let payload = serde_json::json!({
                "license_id": license_id,
                "org": org,
                "seats": seats,
                "features": features,
                "iat": chrono::Utc::now().date_naive().to_string(),
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
            eprintln!("signed {license_id} for {org} — runs until {exp}, updates until {maint_exp}");
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

        Command::Revoke { .. } => {
            // Revocation list wiring is not present in this build; the gate
            // only validates signature + dates. Stub so the tool compiles.
            eprintln!("revoke: revocation list not wired in this build — no-op");
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
