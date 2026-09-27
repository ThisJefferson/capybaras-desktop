//! Protocol tests (M4.1 / M4.2 / M4.3).
//!
//! These drive the **real sidecar over the real pipe**. Each test maps to one of
//! the five rules in `docs/protocol.md`, because those rules are the product's
//! promise — and a promise without a test is just a comment.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use capybaras_shell::paths;
use capybaras_shell::sidecar::Sidecar;
use serde_json::{json, Value};

fn state_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "capybaras-proto-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("could not create test state dir");
    dir
}

fn spawn(dir: &PathBuf) -> Sidecar {
    let sidecar = Sidecar::spawn(&paths::bundled_node(), &paths::sidecar_script(), dir)
        .expect("sidecar spawn failed");
    sidecar
}

/// Read until a message of `want` arrives. Panics if `action.proceeded` shows up
/// first while an approval is pending — which is the failure these exist to catch.
fn expect(sidecar: &Sidecar, want: &str, timeout: Duration) -> Value {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(message) = sidecar.recv_json(Duration::from_millis(400)) {
            if message["type"] == want {
                return message;
            }
        }
    }
    panic!("never received a `{want}` message");
}

fn propose_mass_delete(sidecar: &mut Sidecar, id: &str) {
    sidecar
        .send(&json!({
            "v": 1,
            "type": "action.propose",
            "id": id,
            "action": {
                "tool": "fs.delete",
                "args": { "path": "/prod/database.sqlite" },
                "affectedCount": 5000
            }
        }))
        .expect("could not send action.propose");
}

#[test]
fn rule_1_a_hard_gate_cannot_proceed_without_an_answer() {
    let dir = state_dir("rule1");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    propose_mass_delete(&mut sidecar, "a1");

    let required = expect(&sidecar, "approval.required", Duration::from_secs(15));
    assert_eq!(required["id"], "a1");
    assert_eq!(required["request"]["tier"], "hard_gate");
    assert_eq!(
        required["request"]["canRemember"], false,
        "a hard gate must never be offered as rememberable"
    );

    // THE ASSERTION THAT MATTERS. With an approval pending, the action must not
    // run. Nothing times it out into approval.
    let drain_until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < drain_until {
        if let Some(message) = sidecar.recv_json(Duration::from_millis(300)) {
            assert_ne!(
                message["type"], "action.proceeded",
                "PROCEEDED WITHOUT AN ANSWER: {message}"
            );
        }
    }

    // Only now, with an explicit human answer, may it proceed.
    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "a1", "decision": "allow" }))
        .expect("could not send approval.answer");

    expect(&sidecar, "approval.resolved", Duration::from_secs(10));
    expect(&sidecar, "action.proceeded", Duration::from_secs(10));
}

#[test]
fn rule_2_a_denial_is_final_and_cannot_be_replayed() {
    let dir = state_dir("rule2");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    propose_mass_delete(&mut sidecar, "d1");
    expect(&sidecar, "approval.required", Duration::from_secs(15));

    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "d1", "decision": "deny" }))
        .expect("could not send denial");
    expect(&sidecar, "approval.resolved", Duration::from_secs(10));

    // Replaying the same id must not approve anything. A naive implementation
    // that resolved "the next pending approval" would hand this message to a
    // different action entirely.
    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "d1", "decision": "allow" }))
        .expect("could not send replay");

    let error = expect(&sidecar, "error", Duration::from_secs(10));
    assert_eq!(error["id"], "d1");
}

#[test]
fn rule_3_a_hard_gate_refuses_to_be_remembered_and_stays_pending() {
    let dir = state_dir("rule3");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    propose_mass_delete(&mut sidecar, "m1");
    expect(&sidecar, "approval.required", Duration::from_secs(15));

    // Ask to remember something that can never be remembered.
    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "m1", "decision": "remember" }))
        .expect("could not send remember");

    let error = expect(&sidecar, "error", Duration::from_secs(10));
    assert_eq!(error["id"], "m1");

    // And crucially it must STAY PENDING rather than being quietly approved.
    let drain_until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < drain_until {
        if let Some(message) = sidecar.recv_json(Duration::from_millis(300)) {
            assert_ne!(
                message["type"], "action.proceeded",
                "a refused `remember` must not approve the action: {message}"
            );
        }
    }

    // The human can still decide explicitly.
    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "m1", "decision": "allow" }))
        .expect("could not send allow");
    expect(&sidecar, "approval.resolved", Duration::from_secs(10));
}

#[test]
fn a_silent_read_proceeds_without_asking() {
    let dir = state_dir("silent");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    sidecar
        .send(&json!({
            "v": 1,
            "type": "action.propose",
            "id": "s1",
            "action": { "tool": "fs.read", "args": { "path": "notes.md" } }
        }))
        .expect("could not send propose");

    let proceeded = expect(&sidecar, "action.proceeded", Duration::from_secs(10));
    assert_eq!(proceeded["tier"], "silent");
}

#[test]
fn an_unsupported_protocol_version_is_refused_not_guessed_at() {
    let dir = state_dir("version");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    sidecar
        .send(&json!({ "v": 99, "type": "action.propose", "id": "x1", "action": { "tool": "fs.read" } }))
        .expect("could not send");

    let error = expect(&sidecar, "error", Duration::from_secs(10));
    assert!(
        error["message"].as_str().unwrap_or("").contains("99"),
        "the refusal should name the version it did not understand: {error}"
    );
}
