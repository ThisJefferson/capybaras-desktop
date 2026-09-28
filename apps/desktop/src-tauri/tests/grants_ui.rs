//! The shell's side of grant revocation.
//!
//! The sidecar already knows how to revoke — `tests/protocol.rs` drives a real
//! sidecar and proves a revoked grant makes the same action ask again. What this
//! file pins is the *shape of the message the shell sends*, because a disagreement
//! between the two halves about where the grant id lives would produce the
//! quietest possible failure: the sidecar would accept the message, revoke
//! nothing, and cheerfully report the unchanged list.

use capybaras_shell::{grants_list_message, grants_revoke_message};

#[test]
fn the_list_message_asks_the_sidecar_for_the_remembered_choices() {
    let message = grants_list_message();
    assert_eq!(message["v"], 1);
    assert_eq!(message["type"], "grants.list");
}

#[test]
fn the_revoke_message_names_the_grant_where_the_sidecar_looks_for_it() {
    let message = grants_revoke_message("grant-abc");
    assert_eq!(message["v"], 1);
    assert_eq!(message["type"], "grants.revoke");
    assert_eq!(
        message["scope"]["id"], "grant-abc",
        "the sidecar reads the id from scope.id; anywhere else is a silent no-op"
    );
}

#[test]
fn the_revoke_message_carries_no_other_authority() {
    // A revoke is a WITHDRAWAL of permission. It must name exactly one grant and
    // nothing else — no tool, no target, no wildcard. A message that could carry
    // a scope could be read as a request for one.
    let message = grants_revoke_message("grant-abc");
    assert_eq!(message.as_object().map(|o| o.len()), Some(3), "{message}");
    let scope = message["scope"].as_object().expect("scope object");
    assert_eq!(scope.len(), 1, "only the id belongs in a revoke: {message}");
}
