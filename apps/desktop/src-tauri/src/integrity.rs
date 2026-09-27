//! Integrity verification for the sidecar bundle (docs/threat-model.md, T2).
//!
//! **The threat.** Capybaras's decision logic is the sidecar: it ships as a
//! plain, readable, *writable* JavaScript file. It is not compiled, not signed,
//! and — until this module existed — not checked before launch. Anyone able to
//! write that file owns the gate: a tampered bundle could approve everything and
//! report that it had been careful.
//!
//! **What this does.** `build.rs` hashes the bundle at compile time and bakes the
//! value into the binary. At startup the shell re-hashes what is on disk and
//! compares. A mismatch means the app refuses to start.
//!
//! **What it does not do, stated so nobody assumes more.** It does not stop an
//! attacker who also patches the binary's expected value. That is a
//! substantially harder job than editing a text file, and against a *signed*
//! binary (M6) it is detectable in turn — but the honest claim is
//! **"tampering is detected"**, not "tampering is impossible".

use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The SHA-256 of the sidecar bundle, baked in at compile time by `build.rs`.
///
/// The literal `UNKNOWN` is what `build.rs` bakes when it could not read a
/// bundle. It can never match a real hash, so verification fails closed.
pub const EXPECTED_SHA256: &str = env!("CAPYBARAS_SIDECAR_SHA256");

#[derive(Debug)]
pub enum IntegrityError {
    /// `build.rs` did not find a bundle, so there is nothing to verify against.
    NoExpectation { path: PathBuf },
    /// The bundle could not be read at all.
    Unreadable { path: PathBuf, detail: String },
    /// The bundle is not the one this binary was built against.
    Tampered {
        path: PathBuf,
        expected: String,
        found: String,
    },
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoExpectation { path } => write!(
                f,
                "cannot verify the sidecar: this build has no expected hash baked in. \
                 The bundle at {} was missing when the shell was compiled -- run \
                 `npm run build:sidecar`, then rebuild.",
                path.display()
            ),
            Self::Unreadable { path, detail } => write!(
                f,
                "cannot verify the sidecar: {} could not be read ({}).",
                path.display(),
                detail
            ),
            Self::Tampered { path, expected, found } => write!(
                f,
                "REFUSING TO START: the sidecar at {} is not the one this build expects.\n\
                 \u{20} expected {}\n\
                 \u{20} found    {}\n\
                 Something has modified the decision logic. Rebuild with \
                 `run.cmd`, and if this recurs, treat the machine as compromised.",
                path.display(),
                expected,
                found
            ),
        }
    }
}

impl std::error::Error for IntegrityError {}

/// Verify that `path` is the bundle this binary was built against.
pub fn verify(path: &Path) -> Result<(), IntegrityError> {
    if EXPECTED_SHA256 == "UNKNOWN" {
        return Err(IntegrityError::NoExpectation {
            path: path.to_path_buf(),
        });
    }

    let bytes = std::fs::read(path).map_err(|error| IntegrityError::Unreadable {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let found = hex::encode(hasher.finalize());

    if found == EXPECTED_SHA256 {
        Ok(())
    } else {
        Err(IntegrityError::Tampered {
            path: path.to_path_buf(),
            expected: EXPECTED_SHA256.to_string(),
            found,
        })
    }
}

/// The same computation, exposed so a test can prove the check can fail.
pub fn sha256_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
