//! The token exchange, driven through the seam and through the real client.
//!
//! Two levels on purpose. Most tests use a stub so every documented failure is
//! reproducible with no network. One test drives the **real** `reqwest` client
//! against a local server, so the seam is not the only thing ever exercised —
//! otherwise the seam becomes a place where the real client can be wrong without
//! any test noticing.

use std::cell::RefCell;
use std::net::TcpListener;
use std::time::Duration;

use tiny_http::{Response, Server};

use capybaras_shell::exchange::{
    self, EXCHANGE_ENDPOINT, ExchangeFailure, ExchangeResult, HttpPoster, HttpResponse, Poster,
};

/// A 43-character verifier, which is the RFC minimum. Built at runtime rather than
/// written out, so it cannot be miscounted and cannot be rewritten by a sanitiser.
fn verifier() -> String {
    "A".repeat(43)
}

const CODE: &str = "test-code-value";
const ENDPOINT: &str = "https://example.invalid/exchange";

/// Build a JSON body at runtime. Written this way so no `<name>=<value>` literal
/// appears in this file — see the note in tests/loopback.rs for why that matters.
fn json_body(name: &str, value: &str) -> String {
    serde_json::json!({ name: value }).to_string()
}

struct Stub {
    status: u16,
    body: String,
    transport_error: Option<String>,
    sent: RefCell<Vec<(String, String)>>,
}

impl Stub {
    fn answering(status: u16, body: String) -> Self {
        Self {
            status,
            body,
            transport_error: None,
            sent: RefCell::new(Vec::new()),
        }
    }

    fn failing(detail: &str) -> Self {
        Self {
            status: 0,
            body: String::new(),
            transport_error: Some(detail.to_string()),
            sent: RefCell::new(Vec::new()),
        }
    }

    fn last_body(&self) -> Option<String> {
        self.sent.borrow().last().map(|(_, body)| body.clone())
    }

    fn calls(&self) -> usize {
        self.sent.borrow().len()
    }
}

impl Poster for Stub {
    fn post_json(&self, url: &str, body: &str) -> Result<HttpResponse, String> {
        self.sent.borrow_mut().push((url.to_string(), body.to_string()));
        match &self.transport_error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
            }),
        }
    }
}

fn key_of(result: ExchangeResult) -> String {
    match result {
        ExchangeResult::Key(key) => key,
        other => panic!("expected a key, got {other:?}"),
    }
}

fn reason_of(result: ExchangeResult) -> ExchangeFailure {
    match result {
        ExchangeResult::Failed { reason, .. } => reason,
        ExchangeResult::Key(key) => panic!("expected a failure, got a key: {key}"),
    }
}

// ---------------------------------------------------------------------------
// The happy path
// ---------------------------------------------------------------------------

#[test]
fn a_good_response_yields_the_key() {
    let stub = Stub::answering(200, json_body("key", "test-key-value"));
    let result = exchange::exchange(ENDPOINT, CODE, &verifier(), &stub);
    assert_eq!(key_of(result), "test-key-value");
}

#[test]
fn the_request_carries_the_code_and_the_verifier_and_s256() {
    let stub = Stub::answering(200, json_body("key", "test-key-value"));
    exchange::exchange(ENDPOINT, CODE, &verifier(), &stub);

    let body = stub.last_body().expect("a request was sent");
    assert!(body.contains(CODE), "the code must be sent");
    assert!(body.contains(&verifier()), "the verifier must be sent");
    assert!(body.contains("code_challenge_method"), "the method must be sent");
    assert!(body.contains("S256"));
    // Never `plain`, for any input.
    assert!(!body.contains("plain"), "the weakened method was sent: {body}");
}

#[test]
fn the_request_goes_to_the_endpoint_it_was_given() {
    let stub = Stub::answering(200, json_body("key", "k"));
    exchange::exchange(ENDPOINT, CODE, &verifier(), &stub);
    assert_eq!(stub.sent.borrow()[0].0, ENDPOINT);
    // And the real default is the documented OpenRouter endpoint.
    assert_eq!(EXCHANGE_ENDPOINT, "https://openrouter.ai/api/v1/auth/keys");
}

#[test]
fn a_key_with_surrounding_whitespace_is_trimmed() {
    // A key with a trailing newline fails every later request with a 401 that
    // points nowhere near the cause.
    let stub = Stub::answering(200, json_body("key", "  padded-key\n"));
    assert_eq!(key_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub)), "padded-key");
}

// ---------------------------------------------------------------------------
// Refusals, classified
// ---------------------------------------------------------------------------

#[test]
fn a_400_is_a_method_disagreement() {
    let stub = Stub::answering(400, String::new());
    assert_eq!(
        reason_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub)),
        ExchangeFailure::BadMethod
    );
}

#[test]
fn a_403_mentioning_expiry_is_classified_as_expired() {
    let stub = Stub::answering(403, "the code has expired".to_string());
    let reason = reason_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub));
    assert_eq!(reason, ExchangeFailure::Expired);
    assert_eq!(reason.as_str(), "expired");
}

#[test]
fn a_403_without_expiry_is_classified_as_a_bad_verifier() {
    // These need different actions from the user, so telling them apart matters.
    let stub = Stub::answering(403, "invalid code or code_verifier".to_string());
    assert_eq!(
        reason_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub)),
        ExchangeFailure::BadVerifier
    );
}

#[test]
fn a_405_is_a_rejected_method() {
    let stub = Stub::answering(405, String::new());
    assert_eq!(
        reason_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub)),
        ExchangeFailure::BadRequest
    );
}

#[test]
fn an_unexpected_status_carries_the_status_but_nothing_else() {
    let stub = Stub::answering(503, "upstream is having a bad day".to_string());
    match exchange::exchange(ENDPOINT, CODE, &verifier(), &stub) {
        ExchangeResult::Failed { reason, detail } => {
            assert_eq!(reason, ExchangeFailure::BadRequest);
            assert!(detail.contains("503"), "the status is not secret and is useful: {detail}");
            // The provider's own body is NOT echoed: it is the piece most likely to
            // quote the request back.
            assert!(!detail.contains("upstream is having a bad day"));
        }
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_transport_error_is_network_and_does_not_leak_the_underlying_error() {
    // A transport error can carry the request, and the request carries the code and
    // the verifier. So the detail is a fixed sentence, not an interpolation.
    let leaky = format!("connection reset while sending {{\"code\":\"{CODE}\"}}");
    let stub = Stub::failing(&leaky);

    match exchange::exchange(ENDPOINT, CODE, &verifier(), &stub) {
        ExchangeResult::Failed { reason, detail } => {
            assert_eq!(reason, ExchangeFailure::Network);
            assert_eq!(reason.as_str(), "network");
            assert!(!detail.contains(CODE), "the code leaked into an error message");
            assert!(!detail.contains("connection reset"), "the transport error was echoed");
            assert!(detail.contains("check your connection"), "the user gets something actionable");
        }
        other => panic!("expected a network failure, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Unreadable responses
// ---------------------------------------------------------------------------

#[test]
fn a_body_that_is_not_json_is_malformed() {
    let stub = Stub::answering(200, "<html>not json</html>".to_string());
    assert_eq!(
        reason_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub)),
        ExchangeFailure::Malformed
    );
}

#[test]
fn a_response_with_no_key_is_malformed() {
    for body in [
        json_body("user_id", "12345"),
        "{}".to_string(),
        json_body("key", ""),
        json_body("key", "   "),
        serde_json::json!({ "key": 42 }).to_string(),
    ] {
        let stub = Stub::answering(200, body.clone());
        assert_eq!(
            reason_of(exchange::exchange(ENDPOINT, CODE, &verifier(), &stub)),
            ExchangeFailure::Malformed,
            "body was: {body}"
        );
    }
}

// ---------------------------------------------------------------------------
// Refusals made locally, before anything is sent
// ---------------------------------------------------------------------------

#[test]
fn an_empty_code_is_refused_without_sending_anything() {
    let stub = Stub::answering(200, json_body("key", "k"));
    assert_eq!(
        reason_of(exchange::exchange(ENDPOINT, "", &verifier(), &stub)),
        ExchangeFailure::Malformed
    );
    assert_eq!(stub.calls(), 0, "nothing should be sent for an empty code");
}

#[test]
fn a_malformed_verifier_is_refused_without_sending_anything() {
    // A malformed verifier can only produce a confusing 403, and the local check is
    // free -- so refuse before the code leaves the machine.
    let stub = Stub::answering(200, json_body("key", "k"));
    for bad in ["", "too-short", &"a".repeat(129), &format!("{}!", "a".repeat(42))] {
        assert_eq!(
            reason_of(exchange::exchange(ENDPOINT, CODE, bad, &stub)),
            ExchangeFailure::BadVerifier,
            "verifier was: {bad:?}"
        );
    }
    assert_eq!(stub.calls(), 0, "nothing should be sent for a bad verifier");
}

// ---------------------------------------------------------------------------
// The property that matters most
// ---------------------------------------------------------------------------

#[test]
fn no_failure_message_ever_contains_the_code_or_the_verifier() {
    // The single most likely place for a credential to end up is an error message
    // that reaches a log file or a screenshot. Sweep every failure path.
    let cases: Vec<Stub> = vec![
        Stub::answering(400, String::new()),
        Stub::answering(403, format!("expired, and here is the request: {CODE}")),
        Stub::answering(403, format!("bad verifier: {}", verifier())),
        Stub::answering(405, String::new()),
        Stub::answering(500, format!("server error for {CODE}")),
        Stub::answering(200, format!("not json but mentions {CODE}")),
        Stub::answering(200, json_body("key", "")),
        Stub::failing(&format!("timeout sending {{\"code_verifier\":\"{}\"}}", verifier())),
    ];

    for (index, stub) in cases.iter().enumerate() {
        match exchange::exchange(ENDPOINT, CODE, &verifier(), stub) {
            ExchangeResult::Failed { detail, .. } => {
                assert!(
                    !detail.contains(CODE),
                    "case {index} leaked the code: {detail}"
                );
                assert!(
                    !detail.contains(&verifier()),
                    "case {index} leaked the verifier: {detail}"
                );
                assert!(!detail.is_empty(), "case {index} has an empty message");
            }
            ExchangeResult::Key(key) => panic!("case {index} returned a key: {key}"),
        }
    }
}

// ---------------------------------------------------------------------------
// The real client
// ---------------------------------------------------------------------------

/// Prove the real `reqwest` client actually works, not just the seam.
///
/// WHAT THIS DOES NOT PROVE: TLS against the real endpoint. That can only be shown
/// by talking to OpenRouter, which is exactly the test Jeff runs by clicking
/// Connect. This shows that the client frames a request, sends the right JSON, and
/// reads the response — the parts that would otherwise be untested code.
#[test]
fn the_real_client_sends_and_reads_correctly() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind a local stub server");
    let port = listener.local_addr().expect("local addr").port();
    let server = Server::from_listener(listener, None).expect("wrap in an HTTP server");

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        if let Ok(Some(mut request)) = server.recv_timeout(Duration::from_secs(10)) {
            let mut received = String::new();
            let _ = request.as_reader().read_to_string(&mut received);
            let _ = tx.send(received);
            let _ = request.respond(
                Response::from_string(json_body("key", "from-local-server"))
                    .with_status_code(200),
            );
        }
    });

    let poster = HttpPoster::new().expect("build the real client");
    let result = exchange::exchange(
        &format!("http://127.0.0.1:{port}/exchange"),
        CODE,
        &verifier(),
        &poster,
    );
    assert_eq!(key_of(result), "from-local-server");

    let received = rx.recv_timeout(Duration::from_secs(10)).expect("the stub saw a request");
    assert!(received.contains(CODE), "the real client must send the code");
    assert!(received.contains(&verifier()), "the real client must send the verifier");
    assert!(received.contains("S256"));
    assert!(!received.contains("plain"));
}

#[test]
fn the_real_client_classifies_a_refusal_from_a_real_server() {
    // The classification path through the real client, not just the stub.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = Server::from_listener(listener, None).expect("server");

    std::thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_secs(10)) {
            let _ = request.respond(
                Response::from_string("the code has expired").with_status_code(403),
            );
        }
    });

    let poster = HttpPoster::new().expect("client");
    let result = exchange::exchange(
        &format!("http://127.0.0.1:{port}/exchange"),
        CODE,
        &verifier(),
        &poster,
    );
    assert_eq!(reason_of(result), ExchangeFailure::Expired);
}
