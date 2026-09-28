//! The account balance, read from OpenRouter and shown only if it is understood.
//!
//! **THE FIGURE IS MONEY, SO THE TESTS ARE ABOUT TRUST, NOT ARITHMETIC.** A balance
//! is something a person reads to decide whether to keep spending, so a shape that
//! is not understood exactly must produce *nothing* — the same rule the model
//! catalogue follows. These tests pin the shape the endpoint actually returns
//! (measured live, 2026-09-28) and every way that shape can be wrong.
//!
//! Everything here runs against a stub. No network, no credential, no spend.

use std::cell::RefCell;

use capybaras_shell::credits::{self, CREDITS_ENDPOINT};
use capybaras_shell::http::{HttpResponse, Transport};
use capybaras_shell::usage::CreditScope;

/// A transport that answers from a script and remembers what it was asked.
struct Stub {
    status: u16,
    body: String,
    transport_error: Option<String>,
    requests: RefCell<Vec<Request>>,
}

#[derive(Debug, Clone, PartialEq)]
struct Request {
    method: &'static str,
    url: String,
    bearer: String,
}

impl Stub {
    fn answering(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            transport_error: None,
            requests: RefCell::new(Vec::new()),
        }
    }

    fn unreachable() -> Self {
        Self {
            status: 0,
            body: String::new(),
            transport_error: Some("connection reset while sending the request".to_string()),
            requests: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.requests.borrow().len()
    }

    fn last(&self) -> Request {
        self.requests
            .borrow()
            .last()
            .cloned()
            .expect("a request was made")
    }
}

impl Transport for Stub {
    fn get(&self, url: &str, bearer: &str) -> Result<HttpResponse, String> {
        self.requests.borrow_mut().push(Request {
            method: "GET",
            url: url.to_string(),
            bearer: bearer.to_string(),
        });
        match &self.transport_error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
            }),
        }
    }

    fn post_json(&self, _url: &str, _bearer: &str, _body: &str) -> Result<HttpResponse, String> {
        unimplemented!("the balance is a GET")
    }
}

const KEY: &str = "test-bearer-key";

/// The response, verbatim, measured live on 2026-09-28.
const MEASURED: &str = r#"{"data":{"total_credits":95,"total_usage":89.736164177}}"#;

fn parsed(body: &str) -> serde_json::Value {
    serde_json::from_str(body).expect("the stub body parses")
}

// ---------------------------------------------------------------------------
// The shape that is understood
// ---------------------------------------------------------------------------

#[test]
fn the_measured_response_yields_the_remaining_balance() {
    let credit = credits::parse_credit(&parsed(MEASURED)).expect("a balance");
    assert_eq!(credit.total_credits, 95.0);
    assert!((credit.total_usage - 89.736164177).abs() < 1e-9);
    assert!((credit.remaining() - 5.263835823).abs() < 1e-9);
}

#[test]
fn a_numeric_string_is_accepted_because_providers_send_money_that_way() {
    // The same tolerance `chat.rs` gives a cost: accept a string rather than report
    // a real figure as zero, which would make the balance quietly wrong.
    let value = serde_json::json!({ "data": { "total_credits": "95", "total_usage": "89.5" } });
    let credit = credits::parse_credit(&value).expect("a balance");
    assert_eq!(credit.total_credits, 95.0);
    assert!((credit.remaining() - 5.5).abs() < 1e-9);
}

#[test]
fn extra_fields_are_tolerated_rather_than_failing_the_read() {
    let value = serde_json::json!({
        "data": { "total_credits": 10, "total_usage": 4, "currency": "USD", "created": 1 },
        "meta": { "whatever": true }
    });
    assert!(credits::parse_credit(&value).is_some());
}

// ---------------------------------------------------------------------------
// Every shape that is not understood produces nothing
// ---------------------------------------------------------------------------

#[test]
fn a_shape_that_is_not_understood_shows_nothing_rather_than_a_wrong_number() {
    let cases = [
        serde_json::json!({}),
        serde_json::json!({ "data": null }),
        serde_json::json!({ "data": {} }),
        serde_json::json!({ "data": { "total_credits": 95 } }),
        serde_json::json!({ "data": { "total_usage": 89.7 } }),
        serde_json::json!({ "data": { "total_credits": "ninety five", "total_usage": 1 } }),
        serde_json::json!({ "data": { "total_credits": true, "total_usage": 1 } }),
        serde_json::json!({ "data": { "total_credits": null, "total_usage": 1 } }),
        serde_json::json!({ "data": { "total_credits": [95], "total_usage": 1 } }),
        serde_json::json!({ "data": { "total_credits": -1, "total_usage": 1 } }),
        serde_json::json!({ "data": { "total_credits": 95, "total_usage": -0.5 } }),
        serde_json::json!({ "data": { "total_credits": 1e12, "total_usage": 1 } }),
    ];
    for case in cases {
        assert!(
            credits::parse_credit(&case).is_none(),
            "this should not be trusted: {case}"
        );
    }
}

#[test]
fn an_unexpected_status_shows_nothing() {
    for status in [401, 402, 403, 404, 429, 500, 503] {
        let stub = Stub::answering(status, MEASURED);
        assert!(
            credits::fetch_credits(&stub, KEY).is_none(),
            "status {status} must not produce a balance"
        );
    }
}

#[test]
fn a_body_that_is_not_json_shows_nothing() {
    let stub = Stub::answering(200, "<html>gateway</html>");
    assert!(credits::fetch_credits(&stub, KEY).is_none());
}

#[test]
fn an_unreachable_read_shows_nothing() {
    let stub = Stub::unreachable();
    assert!(credits::fetch_credits(&stub, KEY).is_none());
}

// ---------------------------------------------------------------------------
// The request, and what the figure is claimed to be
// ---------------------------------------------------------------------------

#[test]
fn the_balance_is_fetched_with_the_key_as_a_bearer_and_the_documented_endpoint() {
    let stub = Stub::answering(200, MEASURED);
    let snapshot = credits::fetch_credits(&stub, KEY).expect("a balance");

    assert_eq!(CREDITS_ENDPOINT, "https://openrouter.ai/api/v1/credits");
    let request = stub.last();
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, "https://openrouter.ai/api/v1/credits");
    assert_eq!(request.bearer, KEY, "the key travels as a bearer, not in a body");
    assert_eq!(stub.calls(), 1);

    // THE DISTINCTION THAT MATTERS. `/credits` is an account figure, shared with
    // every other key on the account. It must never be labelled as this key's.
    assert_eq!(snapshot.scope, CreditScope::Account);
    assert_eq!(snapshot.remaining_display, "$5.26");
}
