//! Does a published update verify against the key the app trusts?
//!
//! The updater refuses anything the public key in `tauri.conf.json` does not vouch for, which is
//! the one thing standing between an installed copy and someone else's binary. That check can
//! only fail in production — when a release is signed with a different key, or not at all — so
//! this runs it here, against the artefacts a release actually produced.
//!
//! Ignored by default: it wants files on disk, put there by `scripts/fetch-update.sh` or by hand.
//!
//!   SKIPFRAME_UPDATE_DIR=/path/with/latest.json/and/installer \
//!     cargo test -p skipframe --test updater_signature -- --ignored --nocapture

use std::path::PathBuf;

/// The public key the shipped app checks updates against, read from the config rather than
/// repeated here: a copy could agree with a test and disagree with what users run.
fn configured_pubkey() -> String {
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("tauri.conf.json").unwrap()).unwrap();
    let encoded = config["plugins"]["updater"]["pubkey"].as_str().unwrap();
    String::from_utf8(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).unwrap(),
    )
    .unwrap()
}

#[test]
#[ignore = "needs a downloaded release; see the module comment"]
fn a_published_update_is_signed_by_the_key_the_app_trusts() {
    let dir = PathBuf::from(
        std::env::var("SKIPFRAME_UPDATE_DIR").expect("SKIPFRAME_UPDATE_DIR is not set"),
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();

    // The config holds the whole public key file, base64'd: a comment line, then the key.
    let public_key =
        minisign_verify::PublicKey::from_base64(configured_pubkey().lines().nth(1).unwrap())
            .expect("the configured public key does not parse");

    let mut checked = 0;
    for (platform, entry) in manifest["platforms"].as_object().unwrap() {
        let file = entry["url"].as_str().unwrap().rsplit('/').next().unwrap();
        let path = dir.join(file);
        if !path.exists() {
            println!("{platform}: {file} not downloaded, skipped");
            continue;
        }

        let signature = String::from_utf8(
            base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                entry["signature"].as_str().unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let signature = minisign_verify::Signature::decode(&signature).unwrap();
        let bytes = std::fs::read(&path).unwrap();

        public_key
            .verify(&bytes, &signature, false)
            .unwrap_or_else(|e| panic!("{platform}: {file} does not verify: {e}"));
        println!("{platform}: {file} verifies ({} bayt)", bytes.len());
        checked += 1;
    }

    assert!(checked > 0, "nothing was downloaded to check");
}
