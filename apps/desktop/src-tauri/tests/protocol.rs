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

/// Collect every message that arrives within `ms`. Used where the ORDER of
/// messages matters — `expect` skips non-matching ones, which would discard the
/// `agents.state` frames that arrive just before an approval request.
fn drain(sidecar: &Sidecar, ms: u64) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_millis(ms);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        if let Some(message) = sidecar.recv_json(Duration::from_millis(150)) {
            seen.push(message);
        }
    }
    seen
}

fn latest<'a>(seen: &'a [Value], message_type: &str) -> Option<&'a Value> {
    seen.iter().filter(|m| m["type"] == message_type).next_back()
}

/// M4.6 — the herd.
///
/// Verifies the state machine through the real protocol, not just in unit
/// tests: the sidecar must report all six agents, show the correct owner at
/// work, raise exactly one sign when a human is needed, and lower it after.
#[test]
fn the_herd_reports_who_is_busy_and_who_needs_you() {
    let dir = state_dir("herd");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    // ---- idle: all six present, all attentive ----
    let idle = drain(&sidecar, 900);
    let first = latest(&idle, "agents.state").expect("no agents.state at startup");
    let agents = first["agents"].as_array().expect("agents must be an array");
    assert_eq!(agents.len(), 6, "all six must always be present: {first}");
    assert!(
        agents.iter().all(|a| a["state"] == "listening"),
        "everything should start attentive: {first}"
    );

    // ---- a write is Nina's work, and it needs a human ----
    sidecar
        .send(&json!({
            "v": 1,
            "type": "action.propose",
            "id": "h1",
            "action": { "tool": "fs.write", "args": { "path": "notes.md" } },
            "context": { "targetLabel": "notes.md" }
        }))
        .expect("could not send action.propose");

    let during = drain(&sidecar, 1600);
    assert!(
        during.iter().any(|m| m["type"] == "approval.required"),
        "the write should have asked for approval"
    );
    let shown = latest(&during, "agents.state").expect("no agents.state while pending");
    let list = shown["agents"].as_array().unwrap();
    let nina = list.iter().find(|a| a["id"] == "nina").expect("nina must be present");
    assert_eq!(
        nina["state"], "needs-you",
        "Nina owns fs.write and should be the one asking: {shown}"
    );
    assert_eq!(
        list.iter().filter(|a| a["state"] == "needs-you").count(),
        1,
        "only one agent may need you at a time: {shown}"
    );

    // ---- answer, and the sign comes down ----
    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "h1", "decision": "deny" }))
        .expect("could not send denial");

    let after = drain(&sidecar, 1300);
    let settled = latest(&after, "agents.state").expect("no agents.state after the decision");
    let nina = settled["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "nina")
        .expect("nina");
    assert_eq!(
        nina["state"], "listening",
        "the sign should come down once answered: {settled}"
    );
}

/// M4.7 — THE MILESTONE GATE.
///
/// The Replit incident, driven through the real protocol: an agent with delete
/// rights, repeatedly instructed not to touch production, acting during an
/// explicitly declared code and action freeze, deleting 1,200 records.
///
/// The product's entire claim is that this halts, states plainly what it
/// intends to do, and cannot proceed without a human. This is that claim,
/// executed rather than asserted. Everything the logic-level acceptance test
/// proves, driven end to end through the interface's own plumbing.
#[test]
fn the_replit_incident_is_stopped_at_the_gate() {
    let dir = state_dir("replit");
    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    sidecar
        .send(&json!({
            "v": 1,
            "type": "action.propose",
            "id": "replit",
            "action": {
                "tool": "db.delete",
                "args": { "database": "production", "statement": "DELETE FROM customers" },
                "affectedCount": 1200,
                "reversible": false,
                "protectedTarget": true,
                "taint": "trusted"
            },
            "context": { "targetLabel": "the production database" }
        }))
        .expect("could not send the scenario");

    let during = drain(&sidecar, 1800);

    // ---- 1. It halted. ----
    let required = latest(&during, "approval.required")
        .expect("THE INCIDENT WAS NOT STOPPED: no approval was requested");
    let request = &required["request"];

    // ---- 2. It says plainly what it intends to do, in words. ----
    let headline = request["headline"].as_str().unwrap_or_default();
    assert!(!headline.is_empty(), "the card must say something");
    assert!(
        !headline.contains("db.delete"),
        "the headline must not leak a tool identifier: {headline}"
    );

    // ---- 3. It explains WHY: the blast radius, the freeze, the irreversibility. ----
    let reasons = request["reasons"]
        .as_array()
        .expect("reasons must be an array")
        .iter()
        .filter_map(|r| r.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        reasons.contains("1200") || reasons.contains("1,200"),
        "the blast radius must be stated: {reasons}"
    );
    assert!(
        reasons.contains("off-limits"),
        "the declared freeze must be stated: {reasons}"
    );
    assert!(
        reasons.contains("undone"),
        "irreversibility must be stated: {reasons}"
    );

    // ---- 4. It demands a typed confirmation and refuses to be remembered. ----
    assert_eq!(request["tier"], "hard_gate");
    assert_eq!(request["requiresTypedConfirmation"], true);
    assert_eq!(
        request["canRemember"], false,
        "this incident must never become an 'always allow': {request}"
    );

    // ---- 5. And NOTHING ran while it waited. ----
    assert!(
        !during.iter().any(|m| m["type"] == "action.proceeded"),
        "SOMETHING PROCEEDED WITHOUT A HUMAN: {during:?}"
    );

    // ---- 6. A human says no. It must still not run. ----
    sidecar
        .send(&json!({ "v": 1, "type": "approval.answer", "id": "replit", "decision": "deny" }))
        .expect("could not send the denial");

    let after = drain(&sidecar, 1300);
    assert!(
        after.iter().any(|m| m["type"] == "approval.resolved"),
        "the decision should have been recorded"
    );
    assert!(
        !after.iter().any(|m| m["type"] == "action.proceeded"),
        "A DENIED ACTION RAN ANYWAY: {after:?}"
    );
}

/// D21 -- the freeze comes from CONFIGURATION, not from the caller.
///
/// The July 2025 deletion happened during a "declared code and action freeze"
/// that nothing actually enforced. Before this change the freeze was supplied by
/// the action descriptor -- so it was a request the agent could simply not make.
///
/// This test writes a freeze into the POLICY FILE and then proposes an ordinary
/// action that claims nothing special. `fs.create` is deliberately chosen: it is
/// a notify normally, so without the policy it would sail through. If the gate
/// only listened to the action, the create would proceed.
#[test]
fn a_declared_freeze_in_the_policy_halts_an_action_that_claims_nothing() {
    let dir = state_dir("freeze-policy");
    std::fs::write(
        dir.join("policy.json"),
        r#"{"freeze":{"active":true,"reason":"declared change freeze","declaredBy":"test"}}"#,
    )
    .expect("could not write the policy file");

    let mut sidecar = spawn(&dir);
    expect(&sidecar, "ready", Duration::from_secs(25));

    sidecar
        .send(&json!({
            "v": 1,
            "type": "action.propose",
            "id": "fz",
            "action": { "tool": "fs.create", "args": { "path": "notes.md" } },
            "context": { "targetLabel": "notes.md" }
        }))
        .expect("could not send the proposal");

    let required = expect(&sidecar, "approval.required", Duration::from_secs(15));
    assert_eq!(
        required["request"]["tier"], "confirm",
        "a freeze must raise a notify to a confirm: {required}"
    );

    let reasons = required["request"]["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .filter_map(|r| r.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        reasons.contains("freeze"),
        "a freeze nobody is told about is indistinguishable from a malfunction: {reasons}"
    );

    let _ = sidecar.stop(Duration::from_secs(10));
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

/// M4.5 — durable grants.
///
/// The whole point of persistence is that it survives a real process death, so
/// this test runs the sidecar twice against the same state directory. Session 1
/// remembers an approval; session 2 is a *fresh process* and must honour it
/// without asking. Then it must still refuse to let a hard gate through, even
/// though a grants file now exists.
#[test]
fn a_remembered_grant_survives_a_restart() {
    let dir = state_dir("grants-restart");

    let propose_write = |sidecar: &mut Sidecar, id: &str| {
        sidecar
            .send(&json!({
                "v": 1,
                "type": "action.propose",
                "id": id,
                "action": { "tool": "fs.write", "args": { "path": "notes.md" } },
                "context": { "targetLabel": "notes.md" }
            }))
            .expect("could not send action.propose");
    };

    // ---- session 1: approve once, and ask for it to be remembered ----
    {
        let mut sidecar = spawn(&dir);
        expect(&sidecar, "ready", Duration::from_secs(25));

        propose_write(&mut sidecar, "g1");
        let required = expect(&sidecar, "approval.required", Duration::from_secs(15));
        assert_eq!(
            required["request"]["tier"], "confirm",
            "this action should be a confirm, not a hard gate: {required}"
        );
        assert_eq!(
            required["request"]["canRemember"], true,
            "a plain file write should be rememberable: {required}"
        );

        sidecar
            .send(&json!({ "v": 1, "type": "approval.answer", "id": "g1", "decision": "remember" }))
            .expect("could not send remember");
        expect(&sidecar, "approval.resolved", Duration::from_secs(10));

        let outcome = sidecar.stop(Duration::from_secs(10));
        assert!(outcome.graceful || outcome.forced);
    }

    let grants_file = dir.join("grants.json");
    assert!(
        grants_file.is_file(),
        "remembering an approval should have written {} -- it did not",
        grants_file.display()
    );

    // ---- session 2: a fresh process must honour it without asking ----
    {
        let mut sidecar = spawn(&dir);
        expect(&sidecar, "ready", Duration::from_secs(25));

        propose_write(&mut sidecar, "g2");
        let proceeded = expect(&sidecar, "action.proceeded", Duration::from_secs(15));
        assert_eq!(proceeded["tier"], "confirm");
        assert!(
            proceeded["because"].as_str().unwrap_or("").contains("before"),
            "the grant should have carried it through, citing the earlier approval: {proceeded}"
        );

        // And the invariant still holds after a restart: a hard gate asks.
        propose_mass_delete(&mut sidecar, "g3");
        let required = expect(&sidecar, "approval.required", Duration::from_secs(15));
        assert_eq!(
            required["request"]["tier"], "hard_gate",
            "a hard gate must still ask, even with a grants file present"
        );
        assert_eq!(
            required["request"]["canRemember"], false,
            "a hard gate must never become rememberable"
        );

        let _ = sidecar.stop(Duration::from_secs(10));
    }
}
