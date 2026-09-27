//! T12 — the credential must live in the OS store, not in a file the agent can read.
//!
//! These tests talk to the **real** Windows Credential Manager. That is
//! deliberate: a mocked store would prove the wrapper works and nothing about
//! whether the OS accepts it, which is the part most likely to be wrong.
//!
//! **EACH TEST USES ITS OWN CREDENTIAL NAME, AND THAT IS NOT COSMETIC.** `cargo
//! test` runs tests in parallel. An earlier version of this file gave every test
//! the same name, so they clobbered each other: one saved, another overwrote it,
//! and the first read back the wrong value. The failures read like a broken
//! credential store and were entirely a test-isolation bug. A credential's name
//! is its identity — two callers sharing one name share one secret.

use capybaras_shell::credential;

/// All test credentials live under this prefix so they are obvious in the
/// Windows Credential Manager, and so cleanup can be reasoned about.
const PREFIX: &str = "test.capybaras.";

const NAME_ROUNDTRIP: &str = "test.capybaras.roundtrip";
const NAME_REPLACE: &str = "test.capybaras.replace";
const NAME_MISSING: &str = "test.capybaras.missing";
const NAME_DELETE_ABSENT: &str = "test.capybaras.delete-absent";
const NAME_EMPTY: &str = "test.capybaras.empty";
// NOTE: every test gets names nothing else touches. A second version of this file
// still had one test reusing NAME_ROUNDTRIP, and it failed in parallel for the
// same reason the first version did. The name IS the identity -- a fact worth
// knowing about the real API, not just about tests.
const NAME_ALPHA: &str = "test.capybaras.alpha";
const NAME_BETA: &str = "test.capybaras.beta";

/// Whether the OS store can be used here at all.
///
/// A headless CI runner legitimately may not have an interactive credential
/// store. When it does not, these tests report that loudly and skip — rather than
/// passing vacuously, which would be the test-that-cannot-fail problem, or
/// failing for a reason that has nothing to do with our code.
fn store_or_skip() -> bool {
    if credential::available() {
        return true;
    }
    eprintln!(
        "SKIPPING credential test: no usable OS credential store on this machine. \
         Expected on a headless runner, and NOT a pass -- the behaviour was not proven here."
    );
    false
}

#[test]
fn a_credential_round_trips_through_the_os_store() {
    if !store_or_skip() {
        return;
    }

    // Clean first so a leftover from a failed run cannot make this pass.
    credential::delete(NAME_ROUNDTRIP).expect("pre-clean");

    credential::save(NAME_ROUNDTRIP, "test-value-123").expect("save should succeed");

    let loaded = credential::load(NAME_ROUNDTRIP).expect("load should succeed");
    assert_eq!(
        loaded.as_deref(),
        Some("test-value-123"),
        "the value that came back is not the value that went in"
    );

    credential::delete(NAME_ROUNDTRIP).expect("delete should succeed");
    let after = credential::load(NAME_ROUNDTRIP).expect("load after delete should succeed");
    assert_eq!(after, None, "the credential survived deletion");
}

#[test]
fn saving_again_replaces_rather_than_duplicating() {
    if !store_or_skip() {
        return;
    }
    credential::delete(NAME_REPLACE).expect("pre-clean");

    credential::save(NAME_REPLACE, "first").expect("save first");
    credential::save(NAME_REPLACE, "second").expect("save second");

    assert_eq!(
        credential::load(NAME_REPLACE).expect("load").as_deref(),
        Some("second"),
        "a second save should replace the first, not sit beside it"
    );

    credential::delete(NAME_REPLACE).expect("cleanup");
}

#[test]
fn a_missing_credential_is_not_an_error() {
    if !store_or_skip() {
        return;
    }
    credential::delete(NAME_MISSING).expect("pre-clean");

    // "Not connected" is a normal state, not a failure. An API that errors here
    // pushes callers toward swallowing errors, which is how real ones get missed.
    let loaded = credential::load(NAME_MISSING).expect("a missing entry is not an error");
    assert_eq!(loaded, None);
}

#[test]
fn deleting_something_absent_is_not_an_error() {
    if !store_or_skip() {
        return;
    }
    credential::delete(NAME_DELETE_ABSENT).expect("pre-clean");
    // Disconnect should be idempotent: pressing it twice must not complain.
    credential::delete(NAME_DELETE_ABSENT).expect("deleting an absent credential should be fine");
}

#[test]
fn an_empty_secret_is_refused() {
    if !store_or_skip() {
        return;
    }
    credential::delete(NAME_EMPTY).expect("pre-clean");

    // THE OS STORE ACCEPTS AN EMPTY PASSWORD -- verified by running this.
    // So the refusal has to be ours, and this test asserts ours rather than
    // asserting something untrue about Windows.
    assert!(
        credential::save(NAME_EMPTY, "").is_err(),
        "an empty secret was accepted, so 'connected' could be true while holding nothing"
    );
    let stored = credential::load(NAME_EMPTY).expect("load");
    assert_ne!(stored.as_deref(), Some(""), "an empty secret reached the store");
}

#[test]
fn different_names_hold_different_secrets() {
    if !store_or_skip() {
        return;
    }
    // The property the parallel-run bug violated: the name is the identity.
    credential::delete(NAME_ALPHA).expect("pre-clean");
    credential::delete(NAME_BETA).expect("pre-clean");

    credential::save(NAME_ALPHA, "one").expect("save one");
    credential::save(NAME_BETA, "two").expect("save two");

    assert_eq!(credential::load(NAME_ALPHA).unwrap().as_deref(), Some("one"));
    assert_eq!(credential::load(NAME_BETA).unwrap().as_deref(), Some("two"));

    credential::delete(NAME_ALPHA).expect("cleanup");
    credential::delete(NAME_BETA).expect("cleanup");
}

#[test]
fn every_test_credential_is_namespaced() {
    // Cheap guard: if someone adds a test credential without the prefix it will
    // show up as an unexplained entry in the user's Credential Manager.
    for name in [
        NAME_ROUNDTRIP,
        NAME_REPLACE,
        NAME_MISSING,
        NAME_DELETE_ABSENT,
        NAME_EMPTY,
        NAME_ALPHA,
        NAME_BETA,
    ] {
        assert!(name.starts_with(PREFIX), "{name} is not namespaced");
        assert!(!name.is_empty());
    }
}

#[test]
fn the_store_reports_whether_it_is_usable() {
    // Whatever it returns, it must not panic or hang -- the app calls this before
    // offering a Connect button, and a crash there would be a startup failure.
    let usable = credential::available();
    eprintln!("OS credential store usable here: {usable}");
}
