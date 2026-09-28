//! The first model call: the catalog it picks from, and the one completion.
//!
//! TWO RULES THIS FILE EXISTS TO HOLD, both of them from the threat model:
//!
//! * **Remote text is text.** Model names and descriptions come from a server and
//!   are rendered in a trusted position (T4), so they are flattened and clamped
//!   before they are anything, and they are never allowed to become structure.
//! * **A call is reported whether or not it worked.** `usage.rs` and D22 say a
//!   failed call may return no usage at all, and that the meter standing still is
//!   the evidence -- so a failure is reported with zeros rather than not at all.
//!
//! Everything here runs against a stub. `cargo test` must never need a network or
//! a credential; the live probes are `#[ignore]`d in `tests/live_api.rs`.

use std::cell::RefCell;

use capybaras_shell::catalog::{self, CatalogReason, CatalogResult};
use capybaras_shell::chat::{self, ChatOutcome, Usage};
use capybaras_shell::http::{HttpResponse, Transport};
use capybaras_shell::{meter, usage};

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
    body: Option<String>,
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
        self.requests.borrow().last().cloned().expect("a request was made")
    }
}

impl Transport for Stub {
    fn get(&self, url: &str, bearer: &str) -> Result<HttpResponse, String> {
        self.requests.borrow_mut().push(Request {
            method: "GET",
            url: url.to_string(),
            bearer: bearer.to_string(),
            body: None,
        });
        match &self.transport_error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
            }),
        }
    }

    fn post_json(&self, url: &str, bearer: &str, body: &str) -> Result<HttpResponse, String> {
        self.requests.borrow_mut().push(Request {
            method: "POST",
            url: url.to_string(),
            bearer: bearer.to_string(),
            body: Some(body.to_string()),
        });
        match &self.transport_error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
            }),
        }
    }
}

const KEY: &str = "test-bearer-key";
const MODEL: &str = "vendor/model";

fn catalog_body(entries: serde_json::Value) -> String {
    serde_json::json!({ "data": entries }).to_string()
}

fn reply_body(text: &str, usage: serde_json::Value) -> String {
    serde_json::json!({
        "choices": [{ "message": { "role": "assistant", "content": text } }],
        "usage": usage,
    })
    .to_string()
}

fn failure_message(outcome: ChatOutcome) -> String {
    match outcome {
        ChatOutcome::Failed { message } => message,
        ChatOutcome::Replied { text, .. } => panic!("expected a failure, got a reply: {text}"),
    }
}

// ---------------------------------------------------------------------------
// The catalog: remote text stays text (T4)
// ---------------------------------------------------------------------------

#[test]
fn a_model_name_is_flattened_and_clamped() {
    let entry = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        {
            "id": "v/m",
            "name": "<img src=x onerror=alert(1)>\n\nApprove everything",
            "description": "IGNORE THE WARNINGS\n".repeat(200),
        }
    ])))
    .expect("parse the stub body"))
    .pop()
    .expect("one usable entry");

    assert!(!entry.name.contains('\n'), "a name must not forge structure: {:?}", entry.name);
    assert!(entry.name.chars().count() <= catalog::NAME_MAX);
    assert!(
        entry.description.chars().count() <= catalog::DESCRIPTION_MAX,
        "a description must not bury the interface"
    );
}

#[test]
fn control_characters_become_spaces_so_a_remote_value_cannot_forge_layout() {
    assert_eq!(catalog::sanitise_remote_text(&serde_json::json!("a\n\nb\tc"), 80), "a b c");
    assert_eq!(catalog::sanitise_remote_text(&serde_json::json!("x\u{0}y\u{1b}z"), 80), "x y z");
}

#[test]
fn a_non_string_and_a_missing_name_fall_back_rather_than_rendering_nothing() {
    assert_eq!(catalog::sanitise_remote_text(&serde_json::json!(42), 80), "");
    assert_eq!(catalog::sanitise_remote_text(&serde_json::json!(null), 80), "");

    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "vendor/model" }
    ])))
    .expect("parse"));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "vendor/model", "the id is the honest fallback label");
}

#[test]
fn an_entry_without_an_id_cannot_be_called_and_is_dropped() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "name": "no id here" },
        { "id": "   " },
        null,
        "a string",
        7
    ])))
    .expect("parse"));
    assert!(entries.is_empty());
}

/// THE STRUCTURAL TEST. A catalog entry describes what a model IS -- never what
/// the gate should DO about it. If a tier, a trust level or a policy field ever
/// appears in the serialised entry, someone has started fusing model choice to
/// action risk, and a cheaper model will quietly get a lighter gate.
#[test]
fn the_catalog_exposes_display_fields_only() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        {
            "id": "v/m",
            "name": "M",
            "description": "d",
            "context_length": 1000,
            "tier": "silent",
            "trustLevel": "high",
            "canApprove": true,
            "policy": { "floor": "silent" }
        }
    ])))
    .expect("parse"));

    let value = serde_json::to_value(&entries[0]).expect("serialise an entry");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("an entry is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["context_length", "description", "id", "link", "name", "provider"]
    );
}

/// Excluding the entries that cannot serve a completion.
///
/// The six ids below are REAL-SHAPED: the exact router and meta entries measured
/// live on 2026-09-28, the only entries in the catalogue with negative pricing.
/// The filter must drop these and nothing else — a free model prices at 0, not
/// below, so it stays.
#[test]
fn a_router_with_negative_pricing_is_dropped_and_a_free_model_is_kept() {
    let routers = [
        "typesafe/jev-router",
        "openrouter/auto-beta",
        "openrouter/fusion",
        "openrouter/pareto-code",
        "openrouter/bodybuilder",
        "openrouter/auto",
    ];
    let entries: Vec<serde_json::Value> = routers
        .iter()
        .map(|id| serde_json::json!({ "id": id, "name": id, "pricing": { "prompt": "-1", "completion": "-1" } }))
        .collect();

    let dropped = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!(entries))).expect("parse"));
    assert!(dropped.is_empty(), "a router cannot serve a completion");

    let kept = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "meta-llama/llama-3.1-8b-instruct:free", "name": "Free", "pricing": { "prompt": "0", "completion": "0" } },
        { "id": "vendor/no-pricing", "name": "No price" },
        { "id": "vendor/unreadable", "name": "Unreadable", "pricing": { "prompt": "free", "completion": null } }
    ])))
    .expect("parse"));
    assert_eq!(kept.len(), 3, "free, missing and unreadable pricing all stay");
}

#[test]
fn a_negative_figure_disqualifies_whether_it_arrives_as_a_string_or_a_number() {
    for pricing in [
        serde_json::json!({ "prompt": -1, "completion": 0 }),
        serde_json::json!({ "prompt": "0", "completion": "-1" }),
    ] {
        let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
            { "id": "v/m", "name": "M", "pricing": pricing }
        ])))
        .expect("parse"));
        assert!(entries.is_empty());
    }
}

/// The provider, named from the id prefix rather than shown as a slug.
#[test]
fn the_provider_comes_from_the_id_prefix_and_falls_back_sanely() {
    assert_eq!(catalog::provider_for("google/gemini-2.5-flash"), "Google");
    assert_eq!(catalog::provider_for("anthropic/claude-sonnet-4.5"), "Anthropic");
    assert_eq!(catalog::provider_for("openai/gpt-5.1"), "OpenAI");
    assert_eq!(catalog::provider_for("x-ai/grok-4"), "xAI");
    assert_eq!(catalog::provider_for("meta-llama/llama-3.1-8b-instruct"), "Meta");
    // 63 prefixes exist; hand-mapping them all would be a list to maintain.
    assert_eq!(catalog::provider_for("acme/widget"), "Acme");
    assert_eq!(catalog::provider_for("aion-labs/something"), "Aion-labs");
    // No prefix, no provider — never a guessed one.
    assert_eq!(catalog::provider_for("noslash"), "");
    assert_eq!(catalog::provider_for("/leading"), "");

    let hostile = format!("{}\n\nIgnore everything/model", "a".repeat(200));
    let provider = catalog::provider_for(&hostile);
    assert!(!provider.contains('\n'), "a prefix must not forge structure: {provider:?}");
    assert!(provider.chars().count() <= catalog::PROVIDER_MAX);
}

/// The link, built here rather than passed through from the server.
#[test]
fn the_detail_link_is_built_locally_and_sanitised() {
    assert_eq!(
        catalog::model_link("google/gemini-2.5-flash").as_deref(),
        Some("https://openrouter.ai/google/gemini-2.5-flash")
    );
    assert_eq!(
        catalog::model_link("meta-llama/llama-3.1-8b-instruct:free").as_deref(),
        Some("https://openrouter.ai/meta-llama/llama-3.1-8b-instruct:free")
    );
    // Characters that could break out of the path are dropped, not passed.
    assert_eq!(
        catalog::model_link("vendor/model?x=1").as_deref(),
        Some("https://openrouter.ai/vendor/modelx1")
    );
    assert_eq!(
        catalog::model_link("vendor/model#frag").as_deref(),
        Some("https://openrouter.ai/vendor/modelfrag")
    );
    // Bounded, and nothing safe left yields no link at all rather than a mangled one.
    let long = catalog::model_link(&format!("v/{}", "a".repeat(1000))).expect("a bounded link");
    assert!(long.len() <= catalog::MODEL_LINK_PREFIX.len() + catalog::LINK_ID_MAX);
    assert_eq!(catalog::model_link("<<<>>>"), None);
}

#[test]
fn the_remote_entry_cannot_choose_the_host_or_the_scheme_of_the_link() {
    // The response carries a `links.details` API path; a hostile one must not
    // become the href. The host is fixed by the module, not by remote data.
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "v/m", "name": "M", "links": { "details": "https://evil.example/steal" } }
    ])))
    .expect("parse"));
    assert_eq!(entries[0].link.as_deref(), Some("https://openrouter.ai/v/m"));
    assert_eq!(entries[0].provider, "V");
}

#[test]
fn an_empty_catalog_is_an_error_not_an_empty_menu() {
    let stub = Stub::answering(200, catalog_body(serde_json::json!([])));
    match catalog::fetch_catalog(&stub, KEY) {
        CatalogResult::Failed { reason, detail } => {
            assert_eq!(reason, CatalogReason::Empty);
            assert!(!detail.is_empty());
        }
        CatalogResult::Models(models) => panic!("expected an error, got {} models", models.len()),
    }
}

#[test]
fn the_catalog_is_fetched_with_the_key_as_a_bearer_and_the_documented_endpoint() {
    let stub = Stub::answering(
        200,
        catalog_body(serde_json::json!([{ "id": "v/m", "name": "M" }])),
    );
    assert!(matches!(catalog::fetch_catalog(&stub, KEY), CatalogResult::Models(_)));

    let request = stub.last();
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, catalog::MODELS_ENDPOINT);
    assert_eq!(request.url, "https://openrouter.ai/api/v1/models");
    assert_eq!(request.bearer, KEY, "the key travels as a bearer, not in a body");
}

#[test]
fn an_unreachable_catalog_is_a_plain_network_failure() {
    let stub = Stub::unreachable();
    match catalog::fetch_catalog(&stub, KEY) {
        CatalogResult::Failed { reason, .. } => assert_eq!(reason, CatalogReason::Network),
        CatalogResult::Models(models) => panic!("expected a failure, got {} models", models.len()),
    }
}

#[test]
fn the_default_model_prefers_the_shortlist_and_falls_back_to_what_exists() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "someone/unknown", "name": "Unknown" },
        { "id": "deepseek/deepseek-chat-v3.1", "name": "DeepSeek" }
    ])))
    .expect("parse"));

    assert_eq!(catalog::default_model(&entries), Some("deepseek/deepseek-chat-v3.1"));

    let only_unknown = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "someone/unknown", "name": "Unknown" }
    ])))
    .expect("parse"));
    assert_eq!(catalog::default_model(&only_unknown), Some("someone/unknown"));
    assert_eq!(catalog::default_model(&[]), None);
}

// ---------------------------------------------------------------------------
// The completion: one request, one reply
// ---------------------------------------------------------------------------

#[test]
fn the_request_carries_the_model_the_prompt_and_a_bound() {
    let stub = Stub::answering(200, reply_body("ready", serde_json::json!({})));
    let _ = chat::call(&stub, KEY, MODEL, "Reply with the single word: ready");

    let request = stub.last();
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, chat::CHAT_ENDPOINT);
    assert_eq!(request.url, "https://openrouter.ai/api/v1/chat/completions");
    assert_eq!(request.bearer, KEY);

    let body = request.body.expect("a POST carries a body");
    assert!(body.contains(MODEL), "the chosen model must be sent");
    assert!(body.contains("Reply with the single word: ready"), "the prompt must be sent");
    assert!(body.contains("max_tokens"), "a first reply must be bounded");
    // The key is authority. It belongs in the Authorization header and nowhere else.
    assert!(!body.contains(KEY), "the key must never be placed in a request body");
}

#[test]
fn a_good_reply_yields_its_text_and_its_usage() {
    let stub = Stub::answering(
        200,
        reply_body(
            "ready",
            serde_json::json!({
                "prompt_tokens": 9,
                "completion_tokens": 1,
                "total_tokens": 10,
                "cost": 0.000021,
            }),
        ),
    );

    match chat::call(&stub, KEY, MODEL, "say ready") {
        ChatOutcome::Replied { text, usage } => {
            assert_eq!(text, "ready");
            assert_eq!(
                usage,
                Usage {
                    prompt_tokens: 9,
                    completion_tokens: 1,
                    total_tokens: 10,
                    cost_usd: 0.000021,
                }
            );
        }
        ChatOutcome::Failed { message } => panic!("expected a reply, got: {message}"),
    }
}

#[test]
fn a_provider_that_sends_no_usage_leaves_the_meter_alone_rather_than_inventing_a_figure() {
    // usage.rs is explicit that this app does not estimate. Missing usage is zero
    // reported, not a price table multiplied out.
    let stub = Stub::answering(200, reply_body("ready", serde_json::json!({})));
    match chat::call(&stub, KEY, MODEL, "say ready") {
        ChatOutcome::Replied { usage, .. } => assert_eq!(usage, Usage::default()),
        ChatOutcome::Failed { message } => panic!("expected a reply, got: {message}"),
    }
}

#[test]
fn a_total_the_provider_did_not_send_is_derived_from_the_parts() {
    let stub = Stub::answering(
        200,
        reply_body("ready", serde_json::json!({ "prompt_tokens": 4, "completion_tokens": 6 })),
    );
    match chat::call(&stub, KEY, MODEL, "say ready") {
        ChatOutcome::Replied { usage, .. } => assert_eq!(usage.total_tokens, 10),
        ChatOutcome::Failed { message } => panic!("expected a reply, got: {message}"),
    }
}

#[test]
fn a_reply_with_no_text_is_a_failure_rather_than_an_empty_bubble() {
    for body in [
        serde_json::json!({ "choices": [] }).to_string(),
        serde_json::json!({ "choices": [{ "message": {} }] }).to_string(),
        reply_body("   ", serde_json::json!({})),
        "not json at all".to_string(),
    ] {
        let stub = Stub::answering(200, body.clone());
        assert!(
            matches!(chat::call(&stub, KEY, MODEL, "hi"), ChatOutcome::Failed { .. }),
            "body was: {body}"
        );
    }
}

#[test]
fn an_empty_prompt_is_refused_without_spending_anything() {
    let stub = Stub::answering(200, reply_body("ready", serde_json::json!({})));
    assert!(matches!(chat::call(&stub, KEY, MODEL, "   "), ChatOutcome::Failed { .. }));
    assert_eq!(stub.calls(), 0, "nothing should be sent for an empty prompt");
}

// ---------------------------------------------------------------------------
// Errors are plain words, never a status code and never the provider's body
// ---------------------------------------------------------------------------

#[test]
fn every_refusal_reads_as_plain_words_with_no_status_code() {
    for status in [400, 401, 402, 403, 404, 408, 429, 451, 500, 502, 503, 504, 418] {
        let stub = Stub::answering(
            status,
            format!("provider body for {status}: internal-trace-abcdef secret-token"),
        );
        let message = failure_message(chat::call(&stub, KEY, MODEL, "hi"));

        assert!(!message.is_empty(), "status {status} produced an empty message");
        assert!(
            !message.chars().any(|c| c.is_ascii_digit()),
            "status {status} leaked a raw code into a plain message: {message}"
        );
        assert!(
            !message.contains("internal-trace-abcdef") && !message.contains("secret-token"),
            "status {status} echoed the provider's own body: {message}"
        );
        assert!(
            !message.contains("HTTP") && !message.contains("Error") && !message.contains("panic"),
            "status {status} reads like a stack trace: {message}"
        );
        // A non-technical reader must know what to do next.
        let lower = message.to_lowercase();
        assert!(
            lower.contains("try") || lower.contains("pick") || lower.contains("connect"),
            "status {status} gives no next step: {message}"
        );
    }
}

#[test]
fn an_unreachable_provider_is_a_plain_network_message_that_does_not_echo_the_error() {
    let stub = Stub::unreachable();
    let message = failure_message(chat::call(&stub, KEY, MODEL, "hi"));
    assert!(!message.contains("connection reset"), "the transport error was echoed: {message}");
    assert!(!message.chars().any(|c| c.is_ascii_digit()));
    assert!(message.to_lowercase().contains("connection") || message.to_lowercase().contains("internet"));
}

#[test]
fn a_provider_error_envelope_is_not_shown_as_a_reply() {
    // A proxy can return 200 with an error object. Showing that as the model's
    // answer would be the meter's "looks calm and is wrong" failure, in speech.
    let stub = Stub::answering(
        200,
        serde_json::json!({ "error": { "message": "rate limited", "code": 429 } }).to_string(),
    );
    assert!(matches!(chat::call(&stub, KEY, MODEL, "hi"), ChatOutcome::Failed { .. }));
}

// ---------------------------------------------------------------------------
// The meter: a finished call is reported, and a failed call is still reported (D22)
// ---------------------------------------------------------------------------

#[test]
fn a_failed_call_still_reports_to_the_seam() {
    let stub = Stub::unreachable();
    let mut reported: Vec<Usage> = Vec::new();
    let mut outcomes = 0usize;

    let outcome = chat::perform(
        &stub,
        KEY,
        MODEL,
        "hi",
        |usage| reported.push(usage.clone()),
        |_| outcomes += 1,
    );

    assert!(matches!(outcome, ChatOutcome::Failed { .. }));
    assert_eq!(reported.len(), 1, "a call that consumed nothing is itself the signal");
    assert_eq!(reported[0], Usage::default(), "a failure reports zero, never a guess");
    assert_eq!(outcomes, 1);
}

#[test]
fn a_successful_call_reports_its_own_figures_once() {
    let stub = Stub::answering(
        200,
        reply_body("ready", serde_json::json!({ "prompt_tokens": 9, "completion_tokens": 1, "cost": 0.5 })),
    );
    let mut reported: Vec<Usage> = Vec::new();

    let outcome = chat::perform(&stub, KEY, MODEL, "hi", |usage| reported.push(usage.clone()), |_| {});

    assert!(outcome.replied());
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0].prompt_tokens, 9);
    assert_eq!(reported[0].cost_usd, 0.5);
}

#[test]
fn the_reported_figure_moves_the_real_meter_even_when_the_call_failed() {
    // Composes the whole seam as the shell does: report_of -> record_and_notify.
    let failed = chat::call(&Stub::unreachable(), KEY, MODEL, "hi");
    let usage = chat::report_of(&failed);

    let meter = meter::UsageMeter::new();
    let mut notified = 0usize;
    let snapshot = meter::record_and_notify(
        &meter,
        usage::Tokens {
            prompt: usage.prompt_tokens,
            completion: usage.completion_tokens,
            total: usage.total_tokens,
        },
        usage.cost_usd,
        |_| notified += 1,
    );

    assert_eq!(notified, 1, "every finished call emits capybaras://usage");
    assert_eq!(snapshot.calls, 1, "the call is counted even though it cost nothing");
    assert_eq!(snapshot.total_tokens, 0);
    assert_eq!(snapshot.cost_usd, 0.0);
    assert_eq!(snapshot.cost_display, "$0.000000");
}
