use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{SigningKey, Signer};
use serde_json;
#[derive(serde::Serialize)]
struct License {
    license_id: String,
    org: String,
    seats: u32,
    features: Vec<String>,
    iat: String,
    exp: String,
    maint_exp: String,
    revoked: bool,
}
fn main() {
    // Dev key: private = all-zeros [0u8;32], pubkey from license.rs: DEV_FALLBACK_PUBKEY_HEX
    let signing_key = SigningKey::from_bytes(&[0u8; 32]);
    let license = License {
        license_id: "PX-DEV-20260912".into(),
        org: "dev local".into(),
        seats: 1,
        features: vec!["*".to_string()],
        iat: "2026-09-12".into(),
        exp: "2099-01-01".into(),
        maint_exp: "2099-01-01".into(),
        revoked: false,
    };
    let payload = serde_json::to_vec(&license).unwrap();
    let sig = signing_key.sign(&payload);
    let payload_b64 = B64.encode(&payload);
    let sig_b64 = B64.encode(sig.to_bytes());
    let envelope = serde_json::json!({
        "payload": payload_b64,
        "sig": sig_b64,
    });
    println!("{}", envelope);
}