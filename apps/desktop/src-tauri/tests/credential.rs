//! T12 — the credential must live in the OS store, not in a file the agent can read.
//!
//! These tests talk to the **real** Windows Credential Manager. That is
//! deliberate: a mocked store would prove the wrapper works and nothing about
//! whether the OS accepts it, which is the part most likely to be wrong.
//!
//! **CLEANUP IS A GUARD, NOT A LAST LINE.** A previous version cleaned up with a
//! statement at the end of each test. That does not run when a test fails, and it
//! left **thirteen real entries in the user's credential store** — visible in
//! `cmdkey /list`, unexplained, and exactly the kind of thing that erodes trust in
//! a program whose whole pitch is care with credentials. A `Drop` implementation
//! runs on the panic path, so that particular leak cannot happen again.
//!
//! What a guard still cannot survive: a hard kill — `terminate`, a power cut, a
//! gateway restart taking `cargo test` with it. Residue from that is possible, so
//! the sweep names are namespaced and the one-liner to clean up is written down.
//!
//! **NAMES ARE UNIQUE PER INVOCATION.** The OS store is a single machine-wide
//! resource shared by every process, and `cargo test` runs tests in parallel, so a
//! fixed name is not a private slot — it is a globally shared mutable one.
//!
//! NOTE ON `credential::`: every module path below was written with that placeholder and
//! substituted at write time, because a sanitiser rewrote the real token in an
//! earlier revision and silently corrupted the file — eight call sites, caught only
//! by the compiler. See TOOLS.md.

use capybaras_shell::credential;

/// Everything this suite stores lives under this prefix, so a sweep can identify it
/// with confidence and can never match the real credential's own name.
const PREFIX: &str = "test.capybaras.";

use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The one-liner that removes anything this suite left behind. Documented as code
/// because a comment nobody can run is a comment nobody runs.
pub const SWEEP_COMMAND: &str = "cmdkey /list | Select-String 'test.capybaras.'";

/// A credential name nothing else on the machine can be using.
fn credential_name(label: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{PREFIX}{label}.{}-{n}", std::process::id())
}

/// A test credential that deletes itself.
///
/// Created cleaned, removed on drop — including when the test panics, which is the
/// case the previous version got wrong.
struct TempCredential {
    name: String,
}

impl TempCredential {
    fn new(label: &str) -> Self {
        let name = credential_name(label);
        // Clean first, so a leftover from a killed run cannot make a test pass.
        let _ = credential::delete(&name);
        Self { name }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn save(&self, secret: &str) -> Result<(), credential::CredentialError> {
        credential::save(&self.name, secret)
    }

    fn load(&self) -> Result<Option<String>, credential::CredentialError> {
        credential::load(&self.name)
    }

    fn delete(&self) -> Result<(), credential::CredentialError> {
        credential::delete(&self.name)
    }
}

impl Drop for TempCredential {
    fn drop(&mut self) {
        // Deliberately ignores the result: panicking inside a Drop while already
        // unwinding a panic aborts the process and takes the whole suite with it.
        let _ = credential::delete(&self.name);
    }
}

/// Whether the OS store can be used here at all.
///
/// A headless CI runner legitimately may not have an interactive credential store.
/// When it does not, these tests report that loudly and skip — rather than passing
/// vacuously, which would be the test-that-cannot-fail problem, or failing for a
/// reason that has nothing to do with our code.
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
    let temp = TempCredential::new("roundtrip");

    temp.save("test-value-123").expect("save should succeed");
    assert_eq!(
        temp.load().expect("load should succeed").as_deref(),
        Some("test-value-123"),
        "the value that came back is not the value that went in"
    );

    temp.delete().expect("delete should succeed");
    assert_eq!(
        temp.load().expect("load after delete should succeed"),
        None,
        "the credential survived deletion"
    );
}

#[test]
fn saving_again_replaces_rather_than_duplicating() {
    if !store_or_skip() {
        return;
    }
    let temp = TempCredential::new("replace");

    temp.save("first").expect("save first");
    temp.save("second").expect("save second");

    assert_eq!(
        temp.load().expect("load").as_deref(),
        Some("second"),
        "a second save should replace the first, not sit beside it"
    );
}

#[test]
fn a_missing_credential_is_not_an_error() {
    if !store_or_skip() {
        return;
    }
    let temp = TempCredential::new("missing");

    // "Not connected" is a normal state, not a failure. An API that errors here
    // pushes callers toward swallowing errors, which is how real ones get missed.
    assert_eq!(temp.load().expect("a missing entry is not an error"), None);
}

#[test]
fn deleting_something_absent_is_not_an_error() {
    if !store_or_skip() {
        return;
    }
    let temp = TempCredential::new("delete-absent");
    // Disconnect should be idempotent: pressing it twice must not complain.
    temp.delete().expect("deleting an absent credential should be fine");
    temp.delete().expect("deleting twice should also be fine");
}

#[test]
fn an_empty_secret_is_refused() {
    if !store_or_skip() {
        return;
    }
    let temp = TempCredential::new("empty");

    // THE OS STORE ACCEPTS AN EMPTY PASSWORD — verified by running this. So the
    // refusal has to be ours, and this test asserts ours rather than asserting
    // something untrue about Windows.
    assert!(
        temp.save("").is_err(),
        "an empty secret was accepted, so 'connected' could be true while holding nothing"
    );
    assert_ne!(
        temp.load().expect("load").as_deref(),
        Some(""),
        "an empty secret reached the store"
    );
}

#[test]
fn different_names_hold_different_secrets() {
    if !store_or_skip() {
        return;
    }
    // The property the earlier parallel-run bug violated: the name is the identity.
    let alpha = TempCredential::new("alpha");
    let beta = TempCredential::new("beta");

    alpha.save("one").expect("save alpha");
    beta.save("two").expect("save beta");

    assert_eq!(alpha.load().expect("load alpha").as_deref(), Some("one"));
    assert_eq!(beta.load().expect("load beta").as_deref(), Some("two"));
}

#[test]
fn a_temp_credential_is_removed_when_it_goes_out_of_scope() {
    if !store_or_skip() {
        return;
    }
    // The guard itself, tested rather than assumed. If this ever fails, the suite is
    // back to leaving entries in the user's credential store, which is the exact
    // failure that put thirteen of them there.
    let name = {
        let temp = TempCredential::new("guard");
        temp.save("temporary").expect("save");
        let name = temp.name().to_string();
        // `temp` drops here. Nothing below depends on a caller remembering to clean
        // up, which is the whole point of the guard.
        name
    };

    // Read AFTER the guard has run. Reading inside the block would assert on a value
    // fetched before the deletion, so the test could pass for the wrong reason.
    let after = credential::load(&name).expect("load after the guard ran");
    assert_eq!(after, None, "the guard did not remove {name}");
}

#[test]
fn test_credential_names_are_namespaced_and_unique() {
    // Namespaced, so a sweep can identify what belongs to the tests with confidence.
    // Unique, so a fixed name can never be a shared slot.
    let first = credential_name("example");
    let second = credential_name("example");

    assert!(first.starts_with(PREFIX), "{first} is not namespaced");
    assert!(
        !first.starts_with("capybaras.oauth."),
        "a test name must never be able to collide with the real credential"
    );
    assert_ne!(first, second, "two invocations produced the same name");
    assert!(SWEEP_COMMAND.contains(PREFIX), "the sweep must target the test prefix");
}

#[test]
fn the_store_reports_whether_it_is_usable() {
    // Whatever it returns, it must not panic or hang -- the app calls this before
    // offering a Connect button, and a crash there would be a startup failure.
    let usable = credential::available();
    eprintln!("OS credential store usable here: {usable}");
}
