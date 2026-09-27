//! T2 — the sidecar bundle must be the one this binary was built against.
//!
//! The decision logic ships as a plain, writable JavaScript file, so these tests
//! are about the difference between *claiming* integrity and *checking* it.

use std::path::PathBuf;

use capybaras_shell::{integrity, paths};

fn bundle() -> PathBuf {
    paths::sidecar_script()
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn the_baked_hash_describes_the_bundle_on_disk() {
    // Catches a STALE bundle: rebuild the sidecar without rebuilding the shell and
    // the baked hash no longer describes what would actually run. That is exactly
    // the situation the runtime check exists to refuse, so it should not be
    // possible to reach it by accident in development either.
    match integrity::verify(&bundle()) {
        Ok(()) => {}
        Err(problem) => panic!("the built shell does not match the bundle on disk:\n{problem}"),
    }
}

#[test]
fn the_expected_hash_is_a_real_hash_and_not_a_placeholder() {
    // `build.rs` bakes the literal "UNKNOWN" when it cannot find a bundle. If that
    // ever reaches a release build, verification would be meaningless rather than
    // merely broken -- so assert it is a genuine digest.
    assert_ne!(integrity::EXPECTED_SHA256, "UNKNOWN", "no bundle was present at compile time");
    assert_eq!(integrity::EXPECTED_SHA256.len(), 64, "expected a hex SHA-256");
    assert!(integrity::EXPECTED_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn a_tampered_bundle_is_refused() {
    // THE TEST THAT MATTERS. An instrument that cannot fail is worthless: this
    // proves the check detects a changed file rather than always returning Ok.
    //
    // One appended comment is enough -- and that is the point. The attacker this
    // defends against is not trying to be subtle; they are editing a text file.
    let original = std::fs::read(bundle()).expect("could not read the bundle");
    let mut tampered = original.clone();
    tampered.extend_from_slice(b"\n// injected by a test\n");

    let path = temp("capybaras-tampered-sidecar.mjs");
    std::fs::write(&path, &tampered).expect("could not write the tampered copy");

    let result = integrity::verify(&path);
    assert!(result.is_err(), "a modified bundle must NOT verify");
    let message = result.err().unwrap().to_string();
    assert!(
        message.contains("REFUSING TO START"),
        "the refusal has to be unmistakable to whoever sees it: {message}"
    );
    assert!(message.contains("expected"), "the message should show both hashes: {message}");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_bundle_is_refused_rather_than_assumed_fine() {
    // Fail closed. An absent file is not "nothing to worry about" -- it is a
    // machine where the agent has no gate at all.
    let path = temp("capybaras-does-not-exist-9f3a2c.mjs");
    let _ = std::fs::remove_file(&path);
    assert!(integrity::verify(&path).is_err(), "a missing bundle must fail closed");
}

#[test]
fn an_empty_file_does_not_match() {
    // The degenerate case: truncating the bundle to nothing must not read as a
    // match, which is the shape of bug where a zero-length read compares equal to
    // a zero-length expectation.
    let path = temp("capybaras-empty-sidecar.mjs");
    std::fs::write(&path, b"").expect("could not write the empty file");
    assert!(integrity::verify(&path).is_err(), "an empty bundle must not verify");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn hashing_is_stable_and_matches_a_known_value() {
    // Pins the algorithm. If someone swaps SHA-256 for something weaker, the
    // baked hash and the runtime hash would still agree with each other -- so the
    // agreement alone proves nothing about strength. This does.
    let digest = integrity::sha256_of(b"abc");
    assert_eq!(
        digest,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "the digest is not SHA-256"
    );
}
