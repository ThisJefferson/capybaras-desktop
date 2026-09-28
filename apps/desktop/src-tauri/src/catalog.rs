//! The model catalog, as the shell sees it.
//!
//! **WHY THIS EXISTS ALONGSIDE `src/onboarding/models.ts`.** That module is the
//! catalog's *display contract*, and it is deliberately pure: it takes a `fetch`
//! so it can be tested with no network. But the fetch it describes needs the
//! user's key, and the key must not leave the shell — T12 (`docs/threat-model.md`)
//! and M5-onboarding.md §3: a token the sidecar or the webview can reach is
//! authority the agent can spend outside the gate, and the token is never to reach
//! the protocol stream. So the shell makes the call, and the parsing it feeds has
//! to live here.
//!
//! **IT FOLLOWS `models.ts` RATHER THAN INVENTING A SECOND CONVENTION.** The rules
//! are the same rules, deliberately, and a reviewer should read them side by side:
//!
//! 1. Remote text is flattened and clamped before it is anything. A model name is
//!    attacker-influenced text in a trusted position — the T4 shape — so control
//!    characters become spaces and the length is bounded (`NAME_MAX`,
//!    `DESCRIPTION_MAX`, identical values).
//! 2. An entry carries **display fields only**. There is nothing in its output the
//!    policy could consume, so a model choice can never reach the gate. A test
//!    asserts exactly that, in both languages.
//! 3. An empty catalog is an **error**, not an empty menu. A menu that renders
//!    nothing reads as "no models exist" rather than "the fetch failed".
//! 4. An entry carries a `priceLabel` — a **pre-formatted, sanitised string** the
//!    interface renders verbatim. Prices arrive as a per-token rate, which means
//!    nothing to a person, so the arithmetic to a cost-per-reply happens HERE and
//!    the interface computes nothing. Unreadable or absent pricing yields an empty
//!    string, never a guess and never "$0.00" by accident.
//! 5. An entry carries `maxCompletionTokens` — the model's OWN ceiling on a reply,
//!    from `top_provider.max_completion_tokens`. The shell uses it as the request's
//!    `max_tokens` (see `chat::reply_limit`), so a long answer is asked for in full
//!    rather than truncated at a number picked here.
//! 6. The catalogue is **sorted by display name, case-insensitively, after
//!    filtering**, so the interface receives it already ordered and both languages
//!    agree on the order.
//!
//! The provider and the detail link follow `models.ts` too, and for the same
//! reason: `top_provider` carries no name, and `links.details` is a relative API
//! path, so neither can be shown as-is. The provider is derived from the id
//! prefix; the link is built locally as `https://openrouter.ai/<id>` and never
//! taken from what the server sent. The id is remote data, so it is
//! charset-restricted and length-bounded before it becomes part of a URL.
//!
//! An `id` is kept verbatim, exactly as `models.ts` does: it is an opaque key sent
//! to the API, not display text, and altering it would break the request.

use serde::Serialize;

use crate::http::Transport;

/// Where the model list lives. Must match `MODELS_ENDPOINT` in models.ts.
pub const MODELS_ENDPOINT: &str = "https://openrouter.ai/api/v1/models";

/// How long a remote string may be before it stops being a label. The same bounds
/// models.ts sets: a value that can be arbitrarily long can bury the interface.
pub const NAME_MAX: usize = 80;
pub const DESCRIPTION_MAX: usize = 240;
/// How long a provider label may be. Derived from the id prefix, so remote.
pub const PROVIDER_MAX: usize = 40;
/// The longest id a link will carry. A link is convenience, not data.
pub const LINK_ID_MAX: usize = 200;
/// The only host a detail link may point at. Fixed locally, never remote.
pub const MODEL_LINK_PREFIX: &str = "https://openrouter.ai/";

/// Provider labels for the prefixes worth naming, so a person sees "Google"
/// rather than "google". Deliberately small: 63 prefixes exist and hand-mapping
/// all of them would be a list to maintain against a catalogue that churns. The
/// fallback is the point — an unmapped prefix still shows, capitalised. The same
/// table `models.ts` carries.
pub const PROVIDER_LABELS: [(&str, &str); 16] = [
    ("anthropic", "Anthropic"),
    ("amazon", "Amazon"),
    ("cohere", "Cohere"),
    ("deepseek", "DeepSeek"),
    ("google", "Google"),
    ("groq", "Groq"),
    ("meta-llama", "Meta"),
    ("microsoft", "Microsoft"),
    ("mistralai", "Mistral"),
    ("moonshotai", "Moonshot AI"),
    ("nvidia", "NVIDIA"),
    ("openai", "OpenAI"),
    ("perplexity", "Perplexity"),
    ("qwen", "Qwen"),
    ("x-ai", "xAI"),
    ("z-ai", "Z.AI"),
];

/// Flatten and clamp a string that came from somewhere else.
///
/// Control characters become spaces so a remote value cannot forge layout, and the
/// length is bounded so it cannot bury what surrounds it. The same treatment
/// `models.ts` gives it, and now the same treatment the reply text gets.
pub fn sanitise_remote_text(value: &serde_json::Value, max: usize) -> String {
    let Some(text) = value.as_str() else {
        return String::new();
    };

    let flattened: String = text
        .chars()
        .map(|c| if is_control(c) { ' ' } else { c })
        .collect();
    let collapsed = flattened.split_whitespace().collect::<Vec<_>>().join(" ");

    if collapsed.chars().count() <= max {
        collapsed
    } else {
        let kept: String = collapsed.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}\u{2026}")
    }
}

/// C0 and C1 control characters — `[\u0000-\u001F\u007F-\u009F]` in models.ts.
fn is_control(c: char) -> bool {
    let n = c as u32;
    n <= 0x001F || (0x007F..=0x009F).contains(&n)
}

/// The prefix of an id — everything before the first `/`. Empty when there is
/// none, so a bare id gets no provider rather than a guessed one.
fn provider_prefix(id: &str) -> &str {
    match id.split_once('/') {
        Some((prefix, _)) if !prefix.is_empty() => prefix,
        _ => "",
    }
}

/// The API that serves a model, from its id prefix.
///
/// `google/gemini-2.5-flash` -> "Google". `top_provider` has no name field, so
/// the prefix is the only honest source. The prefix is REMOTE DATA rendered in the
/// interface, so an unmapped one is flattened and clamped like any other label.
pub fn provider_for(id: &str) -> String {
    let prefix = provider_prefix(id);
    if prefix.is_empty() {
        return String::new();
    }
    if let Some((_, label)) = PROVIDER_LABELS
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(prefix))
    {
        return (*label).to_string();
    }
    // Not one we know: show the raw prefix, sanitised, first letter up so a bare
    // "acme" does not read as a typo rather than a name.
    let safe = sanitise_remote_text(&serde_json::Value::String(prefix.to_string()), PROVIDER_MAX);
    let mut chars = safe.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// A link to OpenRouter's page for this model, BUILT LOCALLY.
///
/// Not `links.details`, which is a relative API path, and never a URL the server
/// sent: a link is an injection surface, and the host is fixed here rather than
/// chosen by remote data. The id is charset-restricted to what an OpenRouter id
/// actually uses and length-bounded, then dropped entirely if nothing safe is
/// left, so a malformed id yields no link rather than a mangled one.
pub fn model_link(id: &str) -> Option<String> {
    let safe: String = id
        .chars()
        .filter(|c| is_link_id_char(*c))
        .take(LINK_ID_MAX)
        .collect();
    if safe.is_empty() {
        return None;
    }
    Some(format!("{MODEL_LINK_PREFIX}{safe}"))
}

/// Exactly the characters an OpenRouter id uses: `[A-Za-z0-9._~:@/-]`. Anything
/// else is dropped before the id becomes part of a URL.
fn is_link_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | ':' | '@' | '/' | '-')
}

/// Whether a price field is NEGATIVE.
///
/// This is not quality filtering. OpenRouter publishes `-1` pricing for its own
/// routers and meta entries (`openrouter/auto`, `openrouter/fusion`, ...), which
/// dispatch to other models rather than serving a completion — they cannot be
/// called as written. Six of the 458 entries measured on 2026-09-28 carried
/// negative pricing and every one was such a router. The rule is deliberately
/// narrow: only a negative figure disqualifies. A free model prices at 0, so it
/// stays, and a value that cannot be read as a number is not evidence either way.
fn is_negative_price(value: &serde_json::Value) -> bool {
    if let Some(number) = value.as_f64() {
        return number < 0.0;
    }
    if let Some(text) = value.as_str() {
        if let Ok(number) = text.trim().parse::<f64>() {
            return number < 0.0;
        }
    }
    false
}

/// Whether an entry advertises output this app cannot use.
///
/// **A RULE, NOT A LIST, AND THE MARKER IS THE CATALOGUE'S OWN.** The app makes a
/// chat completion and renders `choices[0].message.content` as TEXT. An entry whose
/// `architecture.output_modalities` names a modality other than `text` is not a
/// text chat model for it: `google/lyria-3-clip-preview` and
/// `google/lyria-3-pro-preview` are music-generation models with
/// `output_modalities: ["text", "audio"]`, priced at zero, and were offered among
/// the free models until this rule.
///
/// Measured live on 2026-09-28: 15 of the 458 entries advertise a non-text output
/// (audio or image), and this rule catches every zero-priced one. It replaces a
/// hand-typed list of two ids that would rot the moment the catalogue churns, which
/// is exactly why a rule is preferred.
///
/// NARROW ON PURPOSE: only an entry that STATES a non-text output modality is
/// dropped. An entry with no `architecture` at all is kept — absence is not
/// evidence, the same principle the pricing filter follows.
fn has_non_text_output(entry: &serde_json::Map<String, serde_json::Value>) -> bool {
    let Some(modalities) = entry
        .get("architecture")
        .and_then(|value| value.as_object())
        .and_then(|architecture| architecture.get("output_modalities"))
        .and_then(|value| value.as_array())
    else {
        return false;
    };
    modalities
        .iter()
        .any(|m| m.as_str().is_some_and(|name| !name.eq_ignore_ascii_case("text")))
}

/// How many tokens a representative reply is assumed to use, for the per-reply
/// estimate the interface shows. Prices are per token, which means nothing to a
/// person, so the display is a cost per reply instead.
///
/// The basis is stated rather than hidden: a question and an answer of about a page
/// in total. It is an ESTIMATE and the label says so with a leading `~`.
pub const PRICE_ASSUMED_PROMPT_TOKENS: f64 = 1000.0;
pub const PRICE_ASSUMED_REPLY_TOKENS: f64 = 1000.0;

/// One price field as a per-token USD figure, or `None` when it cannot be read.
/// Absence is not zero: an unreadable price yields no label rather than a wrong one.
fn price_field(pricing: &serde_json::Map<String, serde_json::Value>, name: &str) -> Option<f64> {
    let value = pricing.get(name)?;
    let number = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse::<f64>().ok()))?;
    (number.is_finite() && number >= 0.0).then_some(number)
}

/// Two significant figures, plain decimal, never an exponent. 0.00075 -> "0.00075".
fn money(value: f64) -> String {
    let exponent = value.abs().log10().floor() as i32;
    let decimals = (1 - exponent).max(2) as usize;
    let text = format!("{value:.decimals$}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.').to_string();
    let shown = trimmed.split('.').nth(1).map(str::len).unwrap_or(0);
    if shown < 2 {
        format!("{trimmed}0")
    } else {
        trimmed
    }
}

/// A pre-formatted price for the interface to render verbatim with `textContent`.
///
/// Four cases, and the interface computes nothing:
///
/// * a genuinely free model (both sides priced at zero) -> `"Free"`;
/// * otherwise an estimate **per reply**, human-readable, e.g. `"~$0.0001 a reply"`
///   — never a raw per-token rate;
/// * unreadable or absent pricing -> `""`, never a guess and never `"$0.00"` by
///   accident. Same rule the balance row already follows;
/// * a figure that cannot be read as a real number is treated as absent.
///
/// The string is built from numbers here and cannot carry remote text, but it is
/// still produced by this module rather than the interface, so nothing downstream
/// has to parse a price.
pub fn price_label(entry: &serde_json::Map<String, serde_json::Value>) -> String {
    let Some(pricing) = entry.get("pricing").and_then(|value| value.as_object()) else {
        return String::new();
    };
    let (Some(prompt), Some(completion)) =
        (price_field(pricing, "prompt"), price_field(pricing, "completion"))
    else {
        return String::new();
    };

    if prompt == 0.0 && completion == 0.0 {
        return "Free".to_string();
    }

    let estimate = prompt * PRICE_ASSUMED_PROMPT_TOKENS + completion * PRICE_ASSUMED_REPLY_TOKENS;
    if !estimate.is_finite() || estimate <= 0.0 {
        return String::new();
    }
    format!("~${} a reply", money(estimate))
}

/// The model's own ceiling on one reply, from `top_provider.max_completion_tokens`.
///
/// Display-safe, and carried for two reasons: the interface may show it, and the
/// shell uses it as the request's `max_tokens`. A missing or zero figure is absent,
/// not zero — absence is answered with a high fallback rather than a small limit
/// (`chat::reply_limit`).
fn max_completion_tokens(entry: &serde_json::Map<String, serde_json::Value>) -> Option<u64> {
    entry
        .get("top_provider")
        .and_then(|value| value.as_object())
        .and_then(|provider| provider.get("max_completion_tokens"))
        .and_then(|value| value.as_u64())
        .filter(|n| *n > 0)
}

/// Whether an entry's pricing disqualifies it — see `is_negative_price`.
fn has_negative_price(entry: &serde_json::Map<String, serde_json::Value>) -> bool {
    let Some(pricing) = entry.get("pricing").and_then(|value| value.as_object()) else {
        return false;
    };
    ["prompt", "completion"]
        .iter()
        .any(|field| pricing.get(*field).is_some_and(is_negative_price))
}

/// The suffix OpenRouter gives a BATCH-ONLY endpoint.
///
/// A `:batch` id is a batch endpoint: it is served by a batch adapter and cannot
/// answer a `chat/completions` request at all. OpenRouter says so plainly when
/// asked - "cannot be used with the chat/completions endpoint (adapter
/// AnthropicBatchAdapter)" - and this app only ever makes chat requests, so the
/// entry cannot serve here however healthy it is as a model.
///
/// **THIS IS A RULE RATHER THAN A LIST, AND THAT IS THE POINT.** Measured live on
/// 2026-09-28 (see `UNAVAILABLE_MODEL_IDS`): all 72 `:batch` entries in the
/// catalogue refused, and no entry without the suffix refused that way. The
/// suffix is the catalogue's own vocabulary, so the rule survives the catalogue
/// churning, where a hand-typed list of 72 ids would rot within a week.
pub const BATCH_ONLY_SUFFIX: &str = ":batch";

/// Whether an id names a batch-only endpoint - see `BATCH_ONLY_SUFFIX`.
///
/// Compared on BYTES so a non-ASCII id cannot make the slice panic: an id is
/// remote data and may be anything a server sends.
pub fn is_batch_only(id: &str) -> bool {
    let bytes = id.as_bytes();
    let suffix = BATCH_ONLY_SUFFIX.as_bytes();
    bytes.len() > suffix.len() && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// Entries measured live that the catalogue will not let this app use.
///
/// **DATED 2026-09-28, AND REGENERABLE.** Two kinds of entry now sit here, and the
/// provenance of each is stated so a future reader can tell which is which. Both
/// come from the maintenance harness beside this module (`tests/catalog_sweep.rs`),
/// which writes its raw result outside this repository:
///
/// ```text
/// cd apps/desktop/src-tauri
/// cargo test --test catalog_sweep -- --ignored --nocapture
/// cargo test --test catalog_sweep retest_the_free_models -- --ignored --nocapture
/// ```
///
/// **1. Endpoints the full sweep proved cannot serve.** Take its `unavailable_ids`,
/// drop the `:batch` entries (the rule above already covers those), and put what is
/// left here. Two entries qualify: `amazon/nova-premier-v1` answers 404, "Provider
/// returned error" — the Amazon failure a user reported; and `openai/gpt-5.2-chat`
/// answers 404 because every candidate endpoint was removed as BYOK-only, and there
/// is nowhere in this app to paste another provider's key. Both are properties of an
/// endpoint rather than of a moment.
///
/// **2. Free models that failed the reliability retest.** The operator's instruction
/// was explicit — *"no flaky models just legit free models that work"* — and, against
/// the earlier recommendation, **flaky models are removed too**. The retest's
/// criterion is the corrected one: HTTP 200 **and** no error envelope **and**
/// non-empty content, three attempts per model, and only a model that answers every
/// time is reliable. A 200 with an empty reply is the failure the operator met
/// (`chat.rs` names it), and the first sweep's "any 200" rule missed it. Measured
/// 2026-09-28: of 20 zero-priced entries, 4 were reliable, 4 flaky and 12 broken;
/// the 15 free ids below are the flaky and broken ones.
///
/// **THE TWO POLICIES DIFFER, AND THAT IS DELIBERATE.** For a PAID model, a rate
/// limit or a 5xx is still NOT grounds for removal — that is a statement about the
/// afternoon, and dropping a model for being busy once would shrink the catalogue
/// invisibly, the most destructive kind of drift. For the FREE list the operator has
/// chosen reliability as the bar, so a free model that cannot answer every time is
/// not offered as one that works. The 15 ids below are mostly rate limits and empty
/// replies; they are removed because a person picking a "free model that works" must
/// not get a 429.
///
/// **THE TWO MUSIC MODELS ARE NOT HERE, ON PURPOSE.** `google/lyria-3-*` are removed
/// by the `has_non_text_output` RULE, not by this list — a rule survives the
/// catalogue churning where a hand-typed id does not. Note that
/// `google/lyria-3-pro-preview` actually answered the retest reliably; it is gone
/// because it is not a text chat model, which is a capability fact rather than a
/// reliability one.
pub const UNAVAILABLE_MODEL_IDS: [&str; 17] = [
    "amazon/nova-premier-v1",
    "openai/gpt-5.2-chat",
    // Free models that failed the 2026-09-28 reliability retest (flaky or broken).
    "cohere/north-mini-code:free",
    "dots-studio/dots-3-note-preview:free",
    "google/gemma-4-26b-a4b-it:free",
    "google/gemma-4-31b-it:free",
    "liquid/lfm-2.5-2.6b:free",
    "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
    "nvidia/nemotron-3-super-120b-a12b:free",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "nvidia/nemotron-3.5-content-safety:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "poolside/laguna-xs-2.1:free",
    "qwen/qwen3.8-27b:free",
    "thinkingmachines/inkling-small:free",
    "thinkingmachines/inkling:free",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogEntry {
    /// The identifier sent to the API. Verbatim: it is an opaque key, not display
    /// text. Note what is NOT here: no tier, no trust level, no policy field.
    pub id: String,
    /// Human-facing, sanitised.
    pub name: String,
    pub description: String,
    /// Which API serves this model, from the id prefix. Display only.
    pub provider: String,
    /// OpenRouter's page for this model, built locally. Absent when the id cannot
    /// safely form one. Display only — the interface opens it, nothing consumes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// Reported context window, or absent when the server did not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    /// A pre-formatted price for the interface to render VERBATIM with
    /// `textContent`. `"Free"`, or an estimate per reply such as
    /// `"~$0.0001 a reply"`, or empty when the price is absent or unreadable — never
    /// a guess and never `"$0.00"` by accident. Display only: the interface computes
    /// nothing, and nothing here is consumable by the policy.
    #[serde(rename = "priceLabel")]
    pub price_label: String,
    /// The model's own ceiling on one reply, from
    /// `top_provider.max_completion_tokens`. The shell sends it as the request's
    /// `max_tokens` so a long answer is not truncated at a number picked here.
    /// Display-safe: a property of the model, not of the gate.
    #[serde(rename = "maxCompletionTokens", skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogReason {
    Network,
    Malformed,
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogResult {
    Models(Vec<CatalogEntry>),
    Failed { reason: CatalogReason, detail: String },
}

/// Turn one raw entry into a display-safe one, or reject it.
fn parse_entry(raw: &serde_json::Value) -> Option<CatalogEntry> {
    let entry = raw.as_object()?;

    let id = entry.get("id").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
    if id.is_empty() {
        return None;
    }

    // A router or meta entry cannot serve a completion. Drop it, narrowly.
    if has_negative_price(entry) {
        return None;
    }

    // An entry that advertises a non-text output is not a text chat model for this
    // app — the rule that replaced a hand-typed list for the music models.
    if has_non_text_output(entry) {
        return None;
    }

    // An entry the live sweep proved cannot serve, by rule or by dated list. Kept
    // apart from the pricing rule because the reason is different: these are
    // callable shapes the API refuses, not entries that are not models at all.
    if is_batch_only(id) || UNAVAILABLE_MODEL_IDS.contains(&id) {
        return None;
    }

    let context_length = entry
        .get("context_length")
        .or_else(|| entry.get("contextLength"))
        .and_then(|v| v.as_u64())
        .filter(|n| *n > 0);

    let named = sanitise_remote_text(
        entry.get("name").unwrap_or(&serde_json::Value::Null),
        NAME_MAX,
    );
    let name = if named.is_empty() {
        // Fall back to the id, which is at least true, rather than showing a blank
        // row the user cannot choose between.
        sanitise_remote_text(&serde_json::Value::String(id.to_string()), NAME_MAX)
    } else {
        named
    };

    let description = sanitise_remote_text(
        entry.get("description").unwrap_or(&serde_json::Value::Null),
        DESCRIPTION_MAX,
    );

    Some(CatalogEntry {
        id: id.to_string(),
        name,
        description,
        provider: provider_for(id),
        link: model_link(id),
        context_length,
        price_label: price_label(entry),
        max_completion_tokens: max_completion_tokens(entry),
    })
}

/// Parse a `/models` response. Tolerant of extra fields, strict about shape.
///
/// **SORTED BY DISPLAY NAME, CASE-INSENSITIVELY, AFTER FILTERING.** The interface
/// receives the catalogue already ordered, so the order is decided here once and
/// both languages agree rather than each re-sorting. Ties fall back to the id so the
/// order is total and stable.
pub fn parse_catalog(raw: &serde_json::Value) -> Vec<CatalogEntry> {
    let mut entries: Vec<CatalogEntry> = raw
        .get("data")
        .and_then(|data| data.as_array())
        .map(|list| list.iter().filter_map(parse_entry).collect())
        .unwrap_or_default();

    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    entries
}

/// Fetch the catalog with the stored key.
pub fn fetch_catalog(transport: &dyn Transport, key: &str) -> CatalogResult {
    let response = match transport.get(MODELS_ENDPOINT, key) {
        Ok(response) => response,
        // Deliberately does not interpolate the transport error: the habit of never
        // putting a request-shaped string into a message is the point.
        Err(_) => {
            return CatalogResult::Failed {
                reason: CatalogReason::Network,
                detail: "could not reach OpenRouter to list models".to_string(),
            };
        }
    };

    if !(200..300).contains(&response.status) {
        return CatalogResult::Failed {
            reason: CatalogReason::Malformed,
            detail: format!(
                "OpenRouter returned an unexpected status ({})",
                response.status
            ),
        };
    }

    let body: serde_json::Value = match serde_json::from_str(&response.body) {
        Ok(value) => value,
        Err(_) => {
            return CatalogResult::Failed {
                reason: CatalogReason::Malformed,
                detail: "the model list could not be read".to_string(),
            };
        }
    };

    let models = parse_catalog(&body);
    if models.is_empty() {
        // An empty catalog is an error, not an empty menu — see the header.
        return CatalogResult::Failed {
            reason: CatalogReason::Empty,
            detail: "OpenRouter returned no usable models".to_string(),
        };
    }
    CatalogResult::Models(models)
}

/// The shortlist `src/onboarding/models.ts` carries, in the same order.
///
/// Duplicated here because the shell owns the fetch, and a *default* chosen by a
/// different rule would be a second convention. It chooses a default only — it does
/// not filter the menu, because a model this list has never heard of is still a
/// model the user may pick.
pub const PREFERRED_MODEL_IDS: [&str; 4] = [
    "anthropic/claude-sonnet-4.5",
    "openai/gpt-5.1",
    "google/gemini-2.5-pro",
    "deepseek/deepseek-chat-v3.1",
];

/// The model to offer first: the best-known shortlist entry the catalog still
/// offers, or the first thing it does offer. Never a model the catalog lacks — a
/// default that fails when used is worse than no default.
pub fn default_model(entries: &[CatalogEntry]) -> Option<&str> {
    for preferred in PREFERRED_MODEL_IDS {
        if let Some(found) = entries.iter().find(|entry| entry.id == preferred) {
            return Some(found.id.as_str());
        }
    }
    entries.first().map(|entry| entry.id.as_str())
}
