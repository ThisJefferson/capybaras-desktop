//! The OpenRouter flow primitives, tested without a network or a browser.
//!
//! The point of these tests is that every failure this flow can produce is
//! **reproducible here** — expired codes, a denied sign-in, a mismatched state,
//! a truncated verifier — rather than discoverable only by a user hitting it.

use capybaras_shell::oauth::{
    self, CallbackFailure, CallbackResult, KEY_LABEL, StatePolicy, callback_url, parse_callback,
    parse_callback_with,
};

// ---------------------------------------------------------------------------
// The verifier
// ---------------------------------------------------------------------------

#[test]
fn a_verifier_is_43_url_safe_characters() {
    let verifier = oauth::create_verifier();
    assert_eq!(verifier.len(), 43, "32 bytes of base64url is 43 characters");
    assert!(
        oauth::is_valid_verifier(&verifier),
        "a verifier we generated must be structurally legal: {verifier}"
    );
}

#[test]
fn a_verifier_uses_only_unreserved_characters() {
    // The classic PKCE silent failure is standard base64 leaking in `+`, `/` or
    // `=` padding. It works locally, and the server rejects the challenge -- or
    // worse, computes a different one and the error is unhelpful.
    for _ in 0..64 {
        let verifier = oauth::create_verifier();
        assert!(!verifier.contains('+'), "standard base64 leaked in: {verifier}");
        assert!(!verifier.contains('/'), "standard base64 leaked in: {verifier}");
        assert!(!verifier.contains('='), "padding leaked in: {verifier}");
    }
}

#[test]
fn two_verifiers_differ() {
    // A constant or seeded generator would pass every structural test above and
    // make every attempt guessable, which is the whole attack.
    let a = oauth::create_verifier();
    let b = oauth::create_verifier();
    assert_ne!(a, b);
}

#[test]
fn the_rfc_7636_appendix_b_vector_matches() {
    // The same vector is pinned in tests/pkce.test.ts. If the Rust port ever
    // drifts from the TypeScript -- or from the RFC -- this fails here rather
    // than in the field, where the symptom is an unhelpful server error.
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        oauth::challenge_for(verifier),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert!(oauth::is_valid_verifier(verifier), "the RFC's own verifier must validate");
}

#[test]
fn verifier_bounds_are_enforced() {
    assert!(!oauth::is_valid_verifier(&"a".repeat(42)), "42 is below the RFC minimum");
    assert!(oauth::is_valid_verifier(&"a".repeat(43)));
    assert!(oauth::is_valid_verifier(&"a".repeat(128)));
    assert!(!oauth::is_valid_verifier(&"a".repeat(129)), "129 is above the RFC maximum");
    assert!(!oauth::is_valid_verifier(""));
}

#[test]
fn verifiers_with_reserved_characters_are_refused() {
    for bad in ["+".repeat(43), "/".repeat(43), "=".repeat(43), "a".repeat(42) + "+"] {
        assert!(!oauth::is_valid_verifier(&bad), "should refuse: {bad}");
    }
    for ok in ["-".repeat(43), "_".repeat(43), ".".repeat(43), "~".repeat(43)] {
        assert!(oauth::is_valid_verifier(&ok), "should allow: {ok}");
    }
}

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

#[test]
fn state_comparison_is_exact() {
    let state = oauth::create_state();
    assert!(oauth::states_match(&state, &state));
    assert!(!oauth::states_match(&state, &state[..state.len() - 1]), "truncation must fail");
    assert!(!oauth::states_match(&state, ""), "empty must fail, not match");
    assert!(!oauth::states_match(&state, "something-else-entirely"));
    assert!(!oauth::states_match("", ""));
    assert!(!oauth::states_match("", &state));
}

#[test]
fn two_states_differ() {
    assert_ne!(oauth::create_state(), oauth::create_state());
}

/// A callback URL, built at runtime so no `<name>=<value>` literal exists here.
fn callback_url_with(params: &[(&str, &str)]) -> String {
    let query: Vec<String> = params
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    format!("http://localhost:1/callback?{}", query.join("&"))
}

const A_CODE: &str = "abc123";

#[test]
fn if_present_tolerates_a_provider_that_sends_no_state() {
    // THE REAL CASE, and the one that broke the first live sign-in. OpenRouter
    // documents no `state` parameter, so any policy that demands one can never
    // succeed against the actual provider.
    let state = oauth::create_state();
    let url = callback_url_with(&[("code", A_CODE)]);

    match parse_callback_with(&url, StatePolicy::IfPresent(&state)) {
        CallbackResult::Ok { code } => assert_eq!(code, A_CODE),
        CallbackResult::Failed { reason, .. } => {
            panic!("a provider that sends no state must still be accepted, got {reason:?}")
        }
    }
}

#[test]
fn if_present_still_rejects_a_state_that_arrived_and_is_wrong() {
    // Tolerating absence must not mean tolerating disagreement.
    let url = callback_url_with(&[("code", A_CODE), ("state", "not-ours")]);
    match parse_callback_with(&url, StatePolicy::IfPresent("the-state-we-sent")) {
        CallbackResult::Failed { reason, .. } => {
            assert_eq!(reason, CallbackFailure::StateMismatch)
        }
        CallbackResult::Ok { .. } => panic!("a present but wrong state must be refused"),
    }
}

#[test]
fn if_present_accepts_a_state_that_arrived_and_is_right() {
    let url = callback_url_with(&[("code", A_CODE), ("state", "ours")]);
    match parse_callback_with(&url, StatePolicy::IfPresent("ours")) {
        CallbackResult::Ok { code } => assert_eq!(code, A_CODE),
        CallbackResult::Failed { reason, .. } => panic!("a matching state was refused: {reason:?}"),
    }
}

#[test]
fn required_still_demands_a_state_that_never_arrives() {
    // The strict policy remains available and remains strict, so relaxing the
    // default cannot have quietly deleted the check.
    let url = callback_url_with(&[("code", A_CODE)]);
    match parse_callback_with(&url, StatePolicy::Required("the-state-we-sent")) {
        CallbackResult::Failed { reason, .. } => {
            assert_eq!(reason, CallbackFailure::StateMismatch)
        }
        CallbackResult::Ok { .. } => panic!("Required must insist on a state"),
    }
}

// ---------------------------------------------------------------------------
// The authorization URL
// ---------------------------------------------------------------------------

#[test]
fn the_authorization_url_carries_what_openrouter_needs() {
    let url = oauth::authorization_url(
        &callback_url(51234),
        "a-challenge-value",
        KEY_LABEL,
    );

    assert!(url.starts_with("https://openrouter.ai/auth?"), "got: {url}");
    // `callback_url` contains a colon and slashes, so it MUST be percent-encoded
    // as a query VALUE. A naive string concatenation produces a URL the provider
    // cannot parse, and it fails as "invalid callback" rather than as our bug.
    assert!(
        url.contains("callback_url=http%3A%2F%2Flocalhost%3A51234%2Fcallback"),
        "the callback URL must be encoded as a value: {url}"
    );
    assert!(url.contains("code_challenge=a-challenge-value"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("key_label=Capybaras"), "the key must be identifiable when revoking: {url}");
}

#[test]
fn the_authorization_url_never_offers_plain() {
    // `plain` provides essentially no protection. Offering it is a way to weaken
    // the flow by configuration, so it must never appear.
    for label in ["Capybaras", "", "a label with spaces"] {
        let url = oauth::authorization_url(&callback_url(1), "c", label);
        assert!(!url.contains("plain"), "the weakened method appeared: {url}");
        assert!(url.contains("code_challenge_method=S256"));
    }
}

#[test]
fn the_authorization_url_has_no_verifier_in_it() {
    // Assert the *shape* of the design: only the challenge goes to the provider.
    let attempt = oauth::create_attempt();
    let url = oauth::authorization_url(&callback_url(40000), &attempt.challenge, KEY_LABEL);
    assert!(!url.contains(&attempt.verifier), "the verifier must never be sent");
    assert!(url.contains(&attempt.challenge));
}

#[test]
fn the_callback_url_targets_localhost() {
    let url = callback_url(51234);
    assert_eq!(url, "http://localhost:51234/callback");
    // Not 0.0.0.0, not a LAN address, not https (there is no certificate for a
    // loopback name, and the provider documents http for localhost).
    assert!(!url.contains("0.0.0.0"));
}

// ---------------------------------------------------------------------------
// Reading the callback
// ---------------------------------------------------------------------------

#[test]
fn a_good_callback_yields_the_code() {
    match parse_callback("http://localhost:51234/callback?code=abc123", None) {
        CallbackResult::Ok { code } => assert_eq!(code, "abc123"),
        CallbackResult::Failed { reason, .. } => panic!("expected a code, got {reason:?}"),
    }
}

#[test]
fn the_code_survives_url_encoding() {
    // Codes are opaque. One containing an encoded character must come back
    // decoded, and a truncated decode would produce a code the exchange rejects.
    match parse_callback("http://localhost:51234/callback?code=a%2Bb%2Fc%3D", None) {
        CallbackResult::Ok { code } => assert_eq!(code, "a+b/c="),
        CallbackResult::Failed { reason, .. } => panic!("expected a code, got {reason:?}"),
    }
}

#[test]
fn a_refusal_reads_as_a_refusal() {
    // The user saying no is not a bug in our code, and it must not surface as
    // one -- the message they see should match what they did.
    match parse_callback("http://localhost:51234/callback?error=access_denied", None) {
        CallbackResult::Failed { reason, detail } => {
            assert_eq!(reason, CallbackFailure::Denied);
            assert_eq!(reason.as_str(), "denied");
            assert_eq!(detail.as_deref(), Some("access_denied"));
        }
        CallbackResult::Ok { .. } => panic!("a refusal must not yield a code"),
    }
}

#[test]
fn an_unrecognised_provider_error_is_malformed_rather_than_denied() {
    match parse_callback("http://localhost:51234/callback?error=server_error", None) {
        CallbackResult::Failed { reason, .. } => assert_eq!(reason, CallbackFailure::Malformed),
        CallbackResult::Ok { .. } => panic!("expected a failure"),
    }
}

#[test]
fn a_callback_without_a_code_fails_rather_than_returning_empty() {
    for url in [
        "http://localhost:51234/callback",
        "http://localhost:51234/callback?code=",
        "http://localhost:51234/callback?state=x",
    ] {
        match parse_callback(url, None) {
            CallbackResult::Failed { reason, .. } => {
                assert_eq!(reason, CallbackFailure::NoCode, "for {url}");
            }
            CallbackResult::Ok { code } => panic!("{url} produced a code: {code:?}"),
        }
    }
}

#[test]
fn nonsense_is_malformed_rather_than_a_panic() {
    for url in ["", "not a url", "://", "http://[", "%"] {
        match parse_callback(url, None) {
            CallbackResult::Failed { reason, .. } => {
                assert_eq!(reason, CallbackFailure::Malformed, "for {url:?}");
            }
            CallbackResult::Ok { .. } => panic!("{url:?} produced a code"),
        }
    }
}

#[test]
fn state_is_verified_only_when_one_was_sent() {
    let state = oauth::create_state();
    let url_with = format!("http://localhost:1/callback?code=x&state={state}");
    let url_without = "http://localhost:1/callback?code=x".to_string();

    // When we sent one, it must come back.
    assert!(matches!(
        parse_callback(&url_with, Some(&state)),
        CallbackResult::Ok { .. }
    ));
    match parse_callback(&url_without, Some(&state)) {
        CallbackResult::Failed { reason, .. } => assert_eq!(reason, CallbackFailure::StateMismatch),
        CallbackResult::Ok { .. } => panic!("a missing state must fail when we sent one"),
    }
    // When the provider does not send one at all, a code must still work. This is
    // the documented OpenRouter case, and requiring a state we know will never
    // arrive would break every real sign-in.
    assert!(matches!(
        parse_callback(&url_without, None),
        CallbackResult::Ok { .. }
    ));
    assert!(matches!(
        parse_callback(&url_with, None),
        CallbackResult::Ok { .. }
    ));
}

#[test]
fn a_mismatched_state_beats_a_valid_code() {
    // Order matters: if the code were returned first, a login-CSRF attempt would
    // get as far as the exchange before being refused.
    let url = "http://localhost:1/callback?code=an-attackers-code&state=not-ours";
    match parse_callback(url, Some("something-else")) {
        CallbackResult::Failed { reason, .. } => assert_eq!(reason, CallbackFailure::StateMismatch),
        CallbackResult::Ok { .. } => panic!("the state check must run before the code is used"),
    }
}

#[test]
fn extra_query_parameters_are_ignored() {
    // The provider may add parameters. An unknown one must not break the flow.
    match parse_callback(
        "http://localhost:1/callback?foo=bar&code=real&baz=qux&state=s",
        Some("s"),
    ) {
        CallbackResult::Ok { code } => assert_eq!(code, "real"),
        CallbackResult::Failed { reason, .. } => panic!("expected a code, got {reason:?}"),
    }
}

// ---------------------------------------------------------------------------
// Leak surfaces
// ---------------------------------------------------------------------------

#[test]
fn debug_on_an_attempt_does_not_print_the_verifier() {
    // The most likely leak is a log line or a panic message that formats the
    // struct holding the secret. `Debug` is written by hand for exactly this.
    let attempt = oauth::create_attempt();
    let printed = format!("{attempt:?}");
    assert!(
        !printed.contains(&attempt.verifier),
        "the verifier reached a Debug string, which is how it reaches a log"
    );
    assert!(printed.contains("[redacted]"));
    // The challenge is public -- it was sent to the provider -- so it may show.
    assert!(printed.contains(&attempt.challenge));
}

#[test]
fn the_callback_page_does_not_echo_the_request() {
    // The browser's URL contains a live code. A "debug" page that echoed the
    // request would put it in the DOM, in history, and in any screenshot.
    for success in [true, false] {
        let page = oauth::callback_page(success);
        assert!(!page.contains("code="), "the page must not echo a query string");
        assert!(!page.contains("http://localhost"), "the page must not echo the request URL");
        assert!(page.contains("Capybaras"));
    }
}

#[test]
fn the_callback_page_says_something_useful_in_both_cases() {
    let good = oauth::callback_page(true);
    assert!(good.contains("connected"));
    assert!(good.contains("close this tab"));

    let bad = oauth::callback_page(false);
    assert!(bad.contains("did not work"));
    // The failure page must not leave the user thinking something changed.
    assert!(bad.contains("No changes were made"));
}
