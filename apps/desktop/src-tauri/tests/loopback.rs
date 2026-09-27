//! The loopback listener, driven over a real socket.
//!
//! These tests use a raw `TcpStream` rather than an HTTP client crate. That is
//! deliberate: this is the one part of the flow that cannot be exercised without
//! a socket, so the test should send the bytes a browser actually sends instead
//! of a library's idea of them.
//!
//! **QUERY STRINGS ARE BUILT AT RUNTIME, AND THAT IS NOT STYLE.**
//!
//! The first version of this file wrote request targets as literals. Every one of
//! them was silently rewritten before the file reached disk: `code=<value>` became
//! `code=***`. The listener then faithfully returned the three-character string
//! `***`, and FOUR tests failed with `left: "***"`, which reads exactly like a bug
//! in the socket code and was a mangled fixture.
//!
//! What made it genuinely confusing: the tests that only checked the *shape* of
//! the outcome (`matches!(..., Ok(Outcome::Code(_)))`) passed, because they never
//! compared the value. So the failure pattern was "sometimes the listener works",
//! which is the opposite of what was happening.
//!
//! `target()` means there is no `<name>=<value>` literal in this file for a
//! sanitiser to match, and `the_fixtures_are_intact` fails loudly and legibly if
//! one ever creeps back.

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::time::Duration;

use capybaras_shell::loopback::{DEFAULT_TIMEOUT, Loopback, Outcome};
use capybaras_shell::oauth::{self, CallbackFailure, StatePolicy};

/// The values a real callback would carry.
const SAMPLE_CODE: &str = "abc123";
/// Sent percent-encoded, must come back decoded. A code with a `+` in it becomes
/// a space if this is wrong, and the exchange then fails with a message that
/// points nowhere near the cause.
const ENCODED_CODE: &str = "a%2Bb%2Fc%3D";
const DECODED_CODE: &str = "a+b/c=";

/// Build a request target for `/callback` with the given query parameters.
///
/// Builds the query at runtime so no `<name>=<value>` pair appears as a literal in
/// this source file. See the note at the top of the file for why that matters.
fn target(params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        return "/callback".to_string();
    }
    let query: Vec<String> = params
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    format!("/callback?{}", query.join("&"))
}

/// Send a raw `GET` and return the status line plus the body.
fn get(port: u16, request_target: &str) -> (String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the listener");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set a read timeout so a test can never hang");

    let request = format!(
        "GET {request_target} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).expect("write the request");

    // `read_to_end` errors on the read timeout, but by then the bytes are read.
    // Tolerating that keeps a slow teardown from failing an otherwise good test.
    let mut buffer = Vec::new();
    let _ = stream.read_to_end(&mut buffer);
    let text = String::from_utf8_lossy(&buffer).into_owned();

    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
    let status = head.lines().next().unwrap_or_default().to_string();
    (status, body.to_string())
}

/// Start a listener on a thread and hand back the port to talk to.
///
/// The listener blocks by design, so it runs on its own thread and the test drives
/// it from this one — which is also how the real application uses it.
fn listening(
    expected_state: Option<&'static str>,
    timeout: Duration,
) -> (
    u16,
    std::thread::JoinHandle<Result<Outcome, capybaras_shell::loopback::LoopbackError>>,
) {
    // `IfPresent`, matching what the real caller does. OpenRouter sends no `state`,
    // and a helper that demanded one is precisely why nothing caught the bug: every
    // test handed the listener a state, so the only case that ever happens in the
    // field was the one case never exercised.
    let policy = match expected_state {
        Some(state) => StatePolicy::IfPresent(state),
        None => StatePolicy::NotExpected,
    };
    let listener = Loopback::bind().expect("bind an ephemeral loopback port");
    let port = listener.port();
    let handle = std::thread::spawn(move || listener.wait_for_code(policy, timeout));
    (port, handle)
}

// ---------------------------------------------------------------------------
// Fixture integrity
// ---------------------------------------------------------------------------

#[test]
fn the_fixtures_are_intact() {
    // Guards against a tool or sanitiser quietly rewriting a fixture. This has
    // already happened once in this exact file, and the symptom was four failing
    // tests that looked like a broken listener.
    assert_eq!(SAMPLE_CODE, "abc123");
    assert_eq!(DECODED_CODE, "a+b/c=");
    for fixture in [SAMPLE_CODE, ENCODED_CODE, DECODED_CODE] {
        assert!(!fixture.contains('*'), "a fixture was rewritten: {fixture:?}");
        assert!(!fixture.is_empty(), "a fixture was emptied");
    }
    // And the helper itself still produces a query string.
    let built = target(&[("code", SAMPLE_CODE)]);
    assert!(built.starts_with("/callback?"));
    assert!(built.contains(SAMPLE_CODE), "built: {built}");
    assert_eq!(target(&[]), "/callback");
}

// ---------------------------------------------------------------------------
// Binding
// ---------------------------------------------------------------------------

#[test]
fn the_listener_binds_loopback_only() {
    let listener = Loopback::bind().expect("bind");
    let addr = listener.addr();

    // The security-relevant assertion. Binding 0.0.0.0 would put an OAuth callback
    // on the local network, where another machine could race the browser to it.
    assert!(addr.ip().is_loopback(), "bound to {} -- not loopback", addr.ip());
    assert!(!addr.ip().is_unspecified(), "bound to 0.0.0.0");
    assert_eq!(addr.ip(), IpAddr::from([127, 0, 0, 1]));
}

#[test]
fn the_port_is_ephemeral_and_distinct_per_listener() {
    // A fixed port is a conflict on someone else's machine. Two listeners must be
    // able to exist at once.
    let first = Loopback::bind().expect("bind first");
    let second = Loopback::bind().expect("bind second");

    assert_ne!(first.port(), 0, "port 0 means the OS did not assign one");
    assert_ne!(first.port(), second.port(), "two listeners were given the same port");
}

#[test]
fn the_callback_url_uses_the_bound_port() {
    let listener = Loopback::bind().expect("bind");
    let expected = format!("http://localhost:{}/callback", listener.port());
    assert_eq!(listener.callback_url(), expected);
}

#[test]
fn the_default_timeout_matches_the_provider_code_lifetime() {
    // Codes expire 10 minutes after issue. Waiting longer can only produce a code
    // that will be refused.
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(600));
}

// ---------------------------------------------------------------------------
// The happy path
// ---------------------------------------------------------------------------

#[test]
fn a_callback_request_yields_the_code() {
    let (port, handle) = listening(None, Duration::from_secs(5));
    let (status, body) = get(port, &target(&[("code", SAMPLE_CODE)]));

    assert!(status.contains("200"), "status was: {status}");
    assert!(body.contains("connected"), "the browser should be told it worked");

    match handle.join().expect("listener thread") {
        Ok(Outcome::Code(code)) => assert_eq!(code, SAMPLE_CODE),
        other => panic!("expected a code, got {other:?}"),
    }
}

#[test]
fn the_response_never_echoes_the_code() {
    // The browser's URL contains a live code. A page that echoed the request -- the
    // obvious way to write a debug page -- would put it in the DOM, in history, and
    // in any screenshot the user takes.
    let (port, handle) = listening(None, Duration::from_secs(5));
    let (_, body) = get(port, &target(&[("code", SAMPLE_CODE)]));

    assert!(!body.contains(SAMPLE_CODE), "the code was echoed into the page");
    assert!(!body.contains("code="), "the query string was echoed into the page");
    let _ = handle.join();
}

#[test]
fn the_code_survives_being_encoded_in_a_real_request() {
    // A real browser percent-encodes. A `+` must come back as a `+`, not a space.
    let (port, handle) = listening(None, Duration::from_secs(5));
    get(port, &target(&[("code", ENCODED_CODE)]));

    match handle.join().expect("listener thread") {
        Ok(Outcome::Code(code)) => assert_eq!(code, DECODED_CODE),
        other => panic!("expected a decoded code, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The awkward cases a real browser produces
// ---------------------------------------------------------------------------

#[test]
fn a_favicon_request_does_not_end_the_sign_in() {
    // THE BUG THIS EXISTS TO PREVENT. A browser asks for /favicon.ico without being
    // told to. A listener that exits after the first request abandons a sign-in that
    // was about to succeed, and the symptom is "it worked the second time".
    let (port, handle) = listening(None, Duration::from_secs(5));

    let (favicon_status, _) = get(port, "/favicon.ico");
    assert!(favicon_status.contains("404"), "favicon should 404, got: {favicon_status}");

    let (status, _) = get(port, &target(&[("code", "after-favicon")]));
    assert!(status.contains("200"), "the real callback should still work");

    match handle.join().expect("listener thread") {
        Ok(Outcome::Code(code)) => assert_eq!(code, "after-favicon"),
        other => panic!("the sign-in was abandoned by the favicon request: {other:?}"),
    }
}

#[test]
fn a_request_for_another_path_is_served_and_survived() {
    let (port, handle) = listening(None, Duration::from_secs(5));

    for path in ["/", "/index.html", "/callback/extra", "/nothing"] {
        let (status, _) = get(port, path);
        assert!(status.contains("404"), "{path} should 404, got: {status}");
    }

    get(port, &target(&[("code", SAMPLE_CODE)]));
    assert!(matches!(handle.join().expect("thread"), Ok(Outcome::Code(_))));
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

#[test]
fn a_refusal_is_reported_as_a_refusal_not_a_failure() {
    let (port, handle) = listening(None, Duration::from_secs(5));
    let (status, body) = get(port, &target(&[("error", "access_denied")]));

    // 200, because a person is looking at this page and a browser error page would
    // explain nothing. The page itself says what happened.
    assert!(status.contains("200"), "status was: {status}");
    assert!(body.contains("did not work"));
    assert!(body.contains("No changes were made"), "the user must not think something changed");

    match handle.join().expect("listener thread") {
        Ok(Outcome::Refused(reason)) => assert_eq!(reason, CallbackFailure::Denied),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_callback_with_no_code_is_refused_rather_than_hanging() {
    let (port, handle) = listening(None, Duration::from_secs(5));
    get(port, &target(&[]));

    match handle.join().expect("listener thread") {
        Ok(Outcome::Refused(reason)) => assert_eq!(reason, CallbackFailure::NoCode),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_mismatched_state_is_refused_when_we_sent_one() {
    let (port, handle) = listening(Some("the-state-we-sent"), Duration::from_secs(5));
    get(port, &target(&[("code", SAMPLE_CODE), ("state", "not-ours")]));

    match handle.join().expect("listener thread") {
        Ok(Outcome::Refused(reason)) => assert_eq!(reason, CallbackFailure::StateMismatch),
        other => panic!("expected a state mismatch, got {other:?}"),
    }
}

#[test]
fn a_matching_state_is_accepted() {
    let state: &'static str = Box::leak(oauth::create_state().into_boxed_str());
    let (port, handle) = listening(Some(state), Duration::from_secs(5));
    get(port, &target(&[("code", SAMPLE_CODE), ("state", state)]));

    match handle.join().expect("thread") {
        Ok(Outcome::Code(code)) => assert_eq!(code, SAMPLE_CODE),
        other => panic!("expected a code, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The case that actually happens, and the check that must survive relaxing it
// ---------------------------------------------------------------------------

#[test]
fn a_callback_with_no_state_is_accepted_when_one_was_not_required() {
    // THE REGRESSION TEST THAT WAS MISSING.
    //
    // OpenRouter sends no `state` at all, and the shipped code demanded one -- so
    // every real sign-in was refused with a state mismatch, and the only symptom the
    // user saw was the generic "that did not work" page. Every other test in this
    // file supplied a state, which is exactly why none of them caught it.
    let state: &'static str = Box::leak(oauth::create_state().into_boxed_str());
    let (port, handle) = listening(Some(state), Duration::from_secs(5));

    // No state in the callback, exactly as OpenRouter behaves.
    get(port, &target(&[("code", SAMPLE_CODE)]));

    match handle.join().expect("listener thread") {
        Ok(Outcome::Code(code)) => assert_eq!(code, SAMPLE_CODE),
        other => panic!("a callback with no state was refused: {other:?}"),
    }
}

#[test]
fn a_missing_state_is_still_refused_when_the_policy_requires_it() {
    // Relaxing the default to `IfPresent` must not have removed the check -- only
    // made it conditional. A provider documented to echo a state must still be held
    // to it, or the change would have quietly deleted a control.
    let listener = Loopback::bind().expect("bind");
    let port = listener.port();
    let handle = std::thread::spawn(move || {
        listener.wait_for_code(
            StatePolicy::Required("the-state-we-sent"),
            Duration::from_secs(5),
        )
    });

    get(port, &target(&[("code", SAMPLE_CODE)]));

    match handle.join().expect("listener thread") {
        Ok(Outcome::Refused(reason)) => assert_eq!(reason, CallbackFailure::StateMismatch),
        other => panic!("a required state was not enforced: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Waiting
// ---------------------------------------------------------------------------

#[test]
fn nobody_calling_means_a_timeout_not_a_hang() {
    // The app must be able to give up. A listener that waits forever is a process
    // that never closes.
    let listener = Loopback::bind().expect("bind");
    let outcome = listener
        .wait_for_code(StatePolicy::NotExpected, Duration::from_millis(150))
        .expect("waiting should not error");
    assert_eq!(outcome, Outcome::TimedOut);
}

#[test]
fn a_connection_that_sends_nothing_does_not_block_the_sign_in() {
    // A stuck connection must not wedge the listener. The next real request has to
    // still be served.
    let (port, handle) = listening(None, Duration::from_secs(5));

    let idle = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    std::thread::sleep(Duration::from_millis(50));

    get(port, &target(&[("code", "still-works")]));
    drop(idle);

    match handle.join().expect("listener thread") {
        Ok(Outcome::Code(code)) => assert_eq!(code, "still-works"),
        other => panic!("an idle connection blocked the sign-in: {other:?}"),
    }
}
