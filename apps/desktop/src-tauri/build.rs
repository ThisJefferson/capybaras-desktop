use std::path::Path;

fn main() {
    tauri_build::build();

    // -----------------------------------------------------------------------
    // Bake the sidecar bundle's hash into the binary (docs/threat-model.md, T2).
    //
    // The decision logic ships as a plain, writable JavaScript file, so anyone
    // who can write it owns the gate. Baking the hash HERE means the binary
    // knows what the bundle is supposed to be, and can refuse to start when what
    // is actually on disk differs.
    //
    // What this buys, stated honestly: tampering is DETECTED. It does not stop
    // someone who also patches the binary -- but that is a much larger job than
    // editing a text file, and against a signed binary it is detectable in turn.
    // -----------------------------------------------------------------------
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set");
    let bundle = Path::new(&manifest)
        .join("..")
        .join("sidecar")
        .join("dist")
        .join("sidecar.mjs");

    // Rebuild when the bundle changes, so the baked hash can never go stale
    // relative to what will actually run.
    println!("cargo:rerun-if-changed={}", bundle.display());

    let hash = match std::fs::read(&bundle) {
        Ok(bytes) => {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hex::encode(hasher.finalize())
        }
        Err(error) => {
            // FAIL CLOSED. No bundle means no gate, so bake an unusable sentinel
            // rather than a wildcard. Verification will then refuse to start the
            // sidecar and say why, instead of silently permitting anything.
            println!(
                "cargo:warning=sidecar bundle not found at {} ({}). Run `npm run build:sidecar`. Integrity verification will refuse to start the sidecar.",
                bundle.display(),
                error
            );
            "UNKNOWN".to_string()
        }
    };

    println!("cargo:rustc-env=CAPYBARAS_SIDECAR_SHA256={hash}");
}
