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

/// A request bound for the stubbed calls: the value used when the catalogue has
/// nothing to say. The catalogue's own path is tested separately.
const LIMIT: u32 = 4096;

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
            "top_provider": { "max_completion_tokens": 64_000 },
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
    // WIDENED DELIBERATELY, NOT LOOSENED. Two display fields were added on
    // 2026-09-28 — `priceLabel` (a pre-formatted price the interface renders
    // verbatim) and `maxCompletionTokens` (the model's own reply ceiling, which the
    // shell sends as `max_tokens`). Both describe what a model IS. The assertion is
    // still an EXACT set, so a tier, a trust level or any other field the policy
    // could consume still fails this test loudly.
    assert_eq!(
        keys,
        [
            "context_length",
            "description",
            "id",
            "link",
            "maxCompletionTokens",
            "name",
            "priceLabel",
            "provider",
        ]
    );
    assert_eq!(entries[0].price_label, "", "no pricing in the stub, so no label");
}

/// The reply ceiling the request asks for comes from the MODEL, not from a number
/// picked in the shell. This is the fix for "the answer should be as long as it
/// needs to be".
#[test]
fn the_request_asks_for_the_models_own_ceiling() {
    let body = chat::request_body(MODEL, "hi", 64_000);
    assert!(body.contains("\"max_tokens\":64000"), "the model's own ceiling is sent: {body}");
    assert!(!body.contains("\"max_tokens\":256"), "the old two-paragraph bound is gone");
}

/// An unknown ceiling is answered with the HIGH fallback, never a low one —
/// absence is not evidence of a short limit.
#[test]
fn an_unknown_ceiling_falls_back_high_never_low() {
    assert_eq!(chat::reply_limit(Some(64_000)), 64_000, "the model's own figure wins");
    assert_eq!(chat::reply_limit(None), chat::DEFAULT_MAX_REPLY_TOKENS);
    assert_eq!(chat::reply_limit(Some(0)), chat::DEFAULT_MAX_REPLY_TOKENS, "zero is not a real ceiling");
    assert!(
        chat::DEFAULT_MAX_REPLY_TOKENS >= 16_384,
        "the fallback must be high; the old 256 was the bug"
    );
    // The display cap must rise WITH the request bound, or a long reply is cut a
    // second time in the shell and the bug looks unfixed.
    assert!(
        chat::REPLY_MAX >= chat::DEFAULT_MAX_REPLY_TOKENS as usize * 4,
        "the display cap must not be the old flat 8,000"
    );
    assert!(chat::REPLY_MAX > 8_000);
}

#[test]
fn a_long_reply_is_no_longer_cut_at_the_old_display_cap() {
    // ~40,000 characters — five times the old 8,000-character cap.
    let long = "word ".repeat(8_000);
    let stub = Stub::answering(200, reply_body(&long, serde_json::json!({})));
    match chat::call(&stub, KEY, MODEL, "hi", chat::DEFAULT_MAX_REPLY_TOKENS) {
        ChatOutcome::Replied { text, .. } => {
            assert!(
                text.chars().count() > 8_000,
                "a long reply must survive the display cap, got {}",
                text.chars().count()
            );
            assert!(!text.ends_with('\u{2026}'), "the reply was cut and marked as cut");
        }
        ChatOutcome::Failed { message } => panic!("expected a reply, got: {message}"),
    }
}

/// The price label is pre-formatted HERE so the interface renders it verbatim and
/// computes nothing. The figures are the real per-token prices measured live on
/// 2026-09-28, at the stated per-reply assumption (1,000 tokens in, 1,000 out).
#[test]
fn the_price_label_is_a_human_estimate_per_reply_or_free_or_nothing() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "openai/gpt-4o-mini", "name": "GPT", "pricing": { "prompt": "0.00000015", "completion": "0.0000006" } },
        { "id": "anthropic/claude-sonnet-4.5", "name": "Claude", "pricing": { "prompt": "0.000003", "completion": "0.000015" } },
        { "id": "google/gemini-2.5-flash", "name": "Gemini", "pricing": { "prompt": "0.0000003", "completion": "0.0000025" } },
        { "id": "meta-llama/llama-3.1-8b-instruct:free", "name": "Free", "pricing": { "prompt": "0", "completion": "0" } },
        { "id": "vendor/unreadable", "name": "Unreadable", "pricing": { "prompt": "free", "completion": null } },
        { "id": "vendor/no-price", "name": "No price" }
    ])))
    .expect("parse"));

    let label = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.price_label.clone())
            .expect("the entry was kept")
    };

    assert_eq!(label("openai/gpt-4o-mini"), "~$0.00075 a reply");
    assert_eq!(label("anthropic/claude-sonnet-4.5"), "~$0.018 a reply");
    assert_eq!(label("google/gemini-2.5-flash"), "~$0.0028 a reply");
    assert_eq!(label("meta-llama/llama-3.1-8b-instruct:free"), "Free");
    // Unreadable and absent pricing show NOTHING — never a guess, never "$0.00".
    assert_eq!(label("vendor/unreadable"), "");
    assert_eq!(label("vendor/no-price"), "");
}

/// The token ceiling is carried through from `top_provider.max_completion_tokens`,
/// and a missing one is absent rather than zero.
#[test]
fn the_models_own_ceiling_is_carried_on_the_entry() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "v/m", "name": "M", "top_provider": { "max_completion_tokens": 64_000 } },
        { "id": "v/noc", "name": "N" },
        { "id": "v/zero", "name": "Z", "top_provider": { "max_completion_tokens": 0 } }
    ])))
    .expect("parse"));
    let ceiling = |id: &str| entries.iter().find(|entry| entry.id == id).and_then(|entry| entry.max_completion_tokens);
    assert_eq!(ceiling("v/m"), Some(64_000));
    assert_eq!(ceiling("v/noc"), None, "absence is not zero");
    assert_eq!(ceiling("v/zero"), None, "a zero ceiling is not a real one");
}

/// A non-text chat model is excluded by a RULE — the catalogue's own
/// `architecture.output_modalities` marker — rather than by a hand-typed id.
#[test]
fn an_entry_that_advertises_non_text_output_is_dropped_by_rule() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "google/lyria-3-clip-preview", "name": "Lyria Clip", "architecture": { "output_modalities": ["text", "audio"] } },
        { "id": "google/lyria-3-pro-preview", "name": "Lyria Pro", "architecture": { "output_modalities": ["text", "audio"] } },
        { "id": "vendor/chat", "name": "Chat", "architecture": { "output_modalities": ["text"] } },
        { "id": "vendor/silent", "name": "Silent" }
    ])))
    .expect("parse"));
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["vendor/chat", "vendor/silent"], "the music models are gone; no-architecture entries stay");
}

/// Sorted by display name, case-insensitively, AFTER filtering — and the input
/// order here differs from the name order, so a no-op would fail.
#[test]
fn the_catalogue_arrives_sorted_by_display_name_ignoring_case() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "v/zeta", "name": "Zeta" },
        { "id": "v/apple", "name": "apple" },
        { "id": "v/banana", "name": "Banana" },
        { "id": "v/cherry", "name": "Cherry" }
    ])))
    .expect("parse"));
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["v/apple", "v/banana", "v/cherry", "v/zeta"]);
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

/// Excluding the entries a LIVE SWEEP proved cannot serve a completion.
///
/// Two mechanisms, and the split matters. A RULE covers what the catalogue
/// itself tells us: a `:batch` id is a batch endpoint, and a batch endpoint
/// cannot answer `chat/completions` at all. A short DATED list covers the rest.
/// See `catalog::is_batch_only` and `catalog::UNAVAILABLE_MODEL_IDS` for the
/// provenance of both, and for how to refresh them.
#[test]
fn a_batch_only_endpoint_is_dropped_by_rule() {
    let entries = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "openai/gpt-5.2:batch", "name": "Batch" },
        { "id": "anthropic/claude-opus-4.1:batch", "name": "Batch" },
        { "id": "google/gemini-2.5-pro", "name": "Fine" },
        { "id": "meta-llama/llama-3.1-8b-instruct:free", "name": "Free" }
    ])))
    .expect("parse"));
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["google/gemini-2.5-pro", "meta-llama/llama-3.1-8b-instruct:free"]);

    assert!(catalog::is_batch_only("openai/gpt-5.2:batch"));
    assert!(catalog::is_batch_only("OPENAI/GPT-5.2:BATCH"), "the suffix is not case-bound");
    assert!(!catalog::is_batch_only("openai/gpt-5.2"), "a plain id is not a batch id");
    assert!(!catalog::is_batch_only(":batch"), "a bare suffix is not a model");
}

#[test]
fn the_dated_unavailable_list_drops_exactly_what_it_names() {
    let entries: Vec<serde_json::Value> = catalog::UNAVAILABLE_MODEL_IDS
        .iter()
        .map(|id| serde_json::json!({ "id": id, "name": id }))
        .collect();
    let dropped = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!(entries))).expect("parse"));
    assert!(dropped.is_empty(), "every listed id is dropped, and the list names only ids");

    let survivors = catalog::parse_catalog(&serde_json::from_str(&catalog_body(serde_json::json!([
        { "id": "amazon/nova-lite-v1", "name": "a different Amazon model" },
        { "id": "openai/gpt-5.2", "name": "the non-BYOK sibling" }
    ])))
    .expect("parse"));
    assert_eq!(survivors.len(), 2, "a list of ids must not become a prefix ban");
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
    let _ = chat::call(&stub, KEY, MODEL, "Reply with the single word: ready", LIMIT);

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

    match chat::call(&stub, KEY, MODEL, "say ready", LIMIT) {
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
    match chat::call(&stub, KEY, MODEL, "say ready", LIMIT) {
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
    match chat::call(&stub, KEY, MODEL, "say ready", LIMIT) {
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
            matches!(chat::call(&stub, KEY, MODEL, "hi", LIMIT), ChatOutcome::Failed { .. }),
            "body was: {body}"
        );
    }
}

#[test]
fn an_empty_prompt_is_refused_without_spending_anything() {
    let stub = Stub::answering(200, reply_body("ready", serde_json::json!({})));
    assert!(matches!(chat::call(&stub, KEY, MODEL, "   ", LIMIT), ChatOutcome::Failed { .. }));
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
        let message = failure_message(chat::call(&stub, KEY, MODEL, "hi", LIMIT));

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
    let message = failure_message(chat::call(&stub, KEY, MODEL, "hi", LIMIT));
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
    assert!(matches!(chat::call(&stub, KEY, MODEL, "hi", LIMIT), ChatOutcome::Failed { .. }));
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
        LIMIT,
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

    let outcome = chat::perform(&stub, KEY, MODEL, "hi", LIMIT, |usage| reported.push(usage.clone()), |_| {});

    assert!(outcome.replied());
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0].prompt_tokens, 9);
    assert_eq!(reported[0].cost_usd, 0.5);
}

#[test]
fn the_reported_figure_moves_the_real_meter_even_when_the_call_failed() {
    // Composes the whole seam as the shell does: report_of -> record_and_notify.
    let failed = chat::call(&Stub::unreachable(), KEY, MODEL, "hi", LIMIT);
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

// ---------------------------------------------------------------------------
// The conversation: the history travels with the turn
// ---------------------------------------------------------------------------

#[test]
fn a_conversation_is_sent_oldest_first_and_in_full() {
    let turns = vec![
        chat::Message::user("first"),
        chat::Message::assistant("reply"),
        chat::Message::user("second"),
    ];
    let body: serde_json::Value =
        serde_json::from_str(&chat::request_body_for(MODEL, &turns, LIMIT)).expect("json");

    assert_eq!(body["model"], MODEL);
    assert_eq!(body["max_tokens"], LIMIT);
    let sent = body["messages"].as_array().expect("messages");
    assert_eq!(sent.len(), 3, "every turn the person can see is sent");
    assert_eq!(sent[0]["role"], "user");
    assert_eq!(sent[0]["content"], "first");
    assert_eq!(sent[1]["role"], "assistant");
    assert_eq!(sent[1]["content"], "reply");
    assert_eq!(sent[2]["content"], "second");
}

#[test]
fn the_window_keeps_the_most_recent_turns_when_the_count_is_over() {
    let mut turns: Vec<chat::Message> = Vec::new();
    for i in 0..(chat::MESSAGES_MAX + 5) {
        turns.push(chat::Message::user(format!("turn {i}")));
    }
    let kept = chat::window(&turns);
    assert_eq!(kept.len(), chat::MESSAGES_MAX);
    assert_eq!(kept.first().expect("non-empty").content, "turn 5", "oldest dropped first");
    assert_eq!(
        kept.last().expect("non-empty").content,
        format!("turn {}", chat::MESSAGES_MAX + 4),
        "the most recent turn always survives"
    );
}

#[test]
fn the_window_drops_the_oldest_when_the_character_budget_is_exceeded() {
    let big = "x".repeat(chat::CONVERSATION_MAX);
    let turns = vec![
        chat::Message::user(big.clone()),
        chat::Message::assistant(big),
        chat::Message::user("the follow-up"),
    ];
    let kept = chat::window(&turns);
    assert_eq!(kept.len(), 1, "only the follow-up fits beside a full-budget earlier turn");
    assert_eq!(kept[0].content, "the follow-up");
}

#[test]
fn a_thread_whose_last_turn_is_the_assistants_is_refused_without_a_call() {
    let stub = Stub::answering(200, reply_body("unused", serde_json::json!({})));
    let outcome = chat::call_with_history(
        &stub,
        KEY,
        MODEL,
        &[chat::Message::user("hi"), chat::Message::assistant("hello")],
        LIMIT,
    );
    assert_eq!(failure_message(outcome), "Type a message first.");
    assert_eq!(stub.calls(), 0, "a malformed thread is refused locally, not billed");
}

#[test]
fn an_empty_conversation_is_refused() {
    let stub = Stub::answering(200, reply_body("unused", serde_json::json!({})));
    let outcome = chat::call_with_history(&stub, KEY, MODEL, &[], LIMIT);
    assert_eq!(failure_message(outcome), "Type a message first.");
    assert_eq!(stub.calls(), 0);
}

#[test]
fn a_follow_up_carries_the_earlier_turns_to_the_provider() {
    let stub = Stub::answering(200, reply_body("the second answer", serde_json::json!({})));
    let outcome = chat::call_with_history(
        &stub,
        KEY,
        MODEL,
        &[
            chat::Message::user("what is a capybara"),
            chat::Message::assistant("a large rodent"),
            chat::Message::user("and its name"),
        ],
        LIMIT,
    );
    assert!(outcome.replied(), "expected a reply");
    let sent: serde_json::Value =
        serde_json::from_str(stub.last().body.as_deref().expect("a body")).expect("json");
    let messages = sent["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 3, "the follow-up arrives with its context");
    assert_eq!(messages[0]["content"], "what is a capybara");
    assert_eq!(messages[2]["content"], "and its name");
}
