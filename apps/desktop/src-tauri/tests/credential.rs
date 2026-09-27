//! T12 — the credential must live in the OS store, not in a file the agent can read.
//!
//! These tests talk to the **real** Windows Credential Manager. That is
//! deliberate: a mocked store would prove the wrapper works and nothing about
//! whether the OS accepts it, which is the part most likely to be wrong.
//!
//! **EVERY CREDENTIAL NAME IS UNIQUE PER INVOCATION, AND THAT IS LOAD-BEARING.**
//!
//! The OS credential store is a *single machine-wide resource* shared by every
//! process on the machine. So a fixed test name is not a private slot — it is a
//! globally shared mutable one. `cargo test` additionally runs tests in parallel.
//!
//! This file has been bitten by that twice. The first version gave every test the
//! same name, so they overwrote each other. The second gave each test a distinct
//! *fixed* name, which made collisions unlikely rather than impossible, and an
//! intermittent failure in `different_names_hold_different_secrets` survived —
//! observed once in roughly twenty full-suite runs, and not reproduced in
//! eighteen further runs afterwards.
//!
//! A per-invocation suffix removes the class instead of the instance. That is the
//! only version of this fix worth having: **a flaky test in the credential store
//! teaches people to press re-run, and re-running is exactly what you must never
//! do here.** What it does NOT do is prove the flake was a collision — that
//! remains a hypothesis. If it recurs, the next step is serialising these tests
//! behind a mutex, since the store itself is one shared resource.

use std::sync::atomic::{AtomicUsize, Ordering};

use capybaras_shell::credential;

/// All test credentials live under this prefix, so anything this suite leaves
/// behind is identifiable in the user's Credential Manager.
const PREFIX: &str = "test.capybaras.";

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A credential name nothing else on the machine can be using.
fn credential_name(label: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{PREFIX}{label}.{}-{n}", std::process::id())
}

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
    let key = credential_name("roundtrip");

    // Clean first so a leftover from a failed run cannot make this pass.
    credential::delete(&key).expect("pre-clean");

    credential::save(&key, "test-value-123").expect("save should succeed");

    let loaded = credential::load(&key).expect("load should succeed");
    assert_eq!(
        loaded.as_deref(),
        Some("test-value-123"),
        "the value that came back is not the value that went in"
    );

    credential::delete(&key).expect("delete should succeed");
    assert_eq!(
        credential::load(&key).expect("load after delete should succeed"),
        None,
        "the credential survived deletion"
    );
}

#[test]
fn saving_again_replaces_rather_than_duplicating() {
    if !store_or_skip() {
        return;
    }
    let key = credential_name("replace");
    credential::delete(&key).expect("pre-clean");

    credential::save(&key, "first").expect("save first");
    credential::save(&key, "second").expect("save second");

    assert_eq!(
        credential::load(&key).expect("load").as_deref(),
        Some("second"),
        "a second save should replace the first, not sit beside it"
    );

    credential::delete(&key).expect("cleanup");
}

#[test]
fn a_missing_credential_is_not_an_error() {
    if !store_or_skip() {
        return;
    }
    let key = credential_name("missing");
    credential::delete(&key).expect("pre-clean");

    // "Not connected" is a normal state, not a failure. An API that errors here
    // pushes callers toward swallowing errors, which is how real ones get missed.
    assert_eq!(credential::load(&key).expect("a missing entry is not an error"), None);
}

#[test]
fn deleting_something_absent_is_not_an_error() {
    if !store_or_skip() {
        return;
    }
    let key = credential_name("delete-absent");
    credential::delete(&key).expect("pre-clean");
    // Disconnect should be idempotent: pressing it twice must not complain.
    credential::delete(&key).expect("deleting an absent credential should be fine");
}

#[test]
fn an_empty_secret_is_refused() {
    if !store_or_skip() {
        return;
    }
    let key = credential_name("empty");
    credential::delete(&key).expect("pre-clean");

    // THE OS STORE ACCEPTS AN EMPTY PASSWORD -- verified by running this.
    // So the refusal has to be ours, and this test asserts ours rather than
    // asserting something untrue about Windows.
    assert!(
        credential::save(&key, "").is_err(),
        "an empty secret was accepted, so 'connected' could be true while holding nothing"
    );
    let stored = credential::load(&key).expect("load");
    assert_ne!(stored.as_deref(), Some(""), "an empty secret reached the store");
}

#[test]
fn different_names_hold_different_secrets() {
    if !store_or_skip() {
        return;
    }
    // The property the earlier parallel-run bug violated: the name is the identity.
    let alpha = credential_name("alpha");
    let beta = credential_name("beta");
    credential::delete(&alpha).expect("pre-clean");
    credential::delete(&beta).expect("pre-clean");

    credential::save(&alpha, "one").expect("save alpha");
    credential::save(&beta, "two").expect("save beta");

    assert_eq!(credential::load(&alpha).expect("load alpha").as_deref(), Some("one"));
    assert_eq!(credential::load(&beta).expect("load beta").as_deref(), Some("two"));

    credential::delete(&alpha).expect("cleanup");
    credential::delete(&beta).expect("cleanup");
}

#[test]
fn test_credential_names_are_namespaced_and_unique() {
    // Two properties this file now depends on. Namespaced, so anything left
    // behind is identifiable in the user's Credential Manager rather than an
    // unexplained entry. Unique, so a fixed name can never be a shared slot.
    let first = credential_name("example");
    let second = credential_name("example");

    assert!(first.starts_with(PREFIX), "{first} is not namespaced");
    assert!(first.len() > PREFIX.len());
    assert_ne!(first, second, "two invocations produced the same name");
}

#[test]
fn the_store_reports_whether_it_is_usable() {
    // Whatever it returns, it must not panic or hang -- the app calls this before
    // offering a Connect button, and a crash there would be a startup failure.
    let usable = credential::available();
    eprintln!("OS credential store usable here: {usable}");
}
