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

/// Entries a live sweep PROVED cannot serve, that no rule covers yet.
///
/// **DATED 2026-09-28, AND REGENERABLE.** How to refresh it: run the maintenance
/// sweep that sits beside this module (`tests/catalog_sweep.rs`), which calls
/// every catalogue entry with a one-token prompt and writes its raw result
/// outside this repository -
///
/// ```text
/// cd apps/desktop/src-tauri
/// cargo test --test catalog_sweep -- --ignored --nocapture
/// ```
///
/// Take its `unavailable_ids`, drop the `:batch` entries (the rule above already
/// covers those), and put what is left here. Re-run it rather than trusting this
/// list: an entry can be repaired as easily as it broke.
///
/// **WHY A LIST EXISTS AT ALL, WHEN A RULE IS PREFERRED.** Because two entries
/// refused in a way no field in the catalogue predicts, and neither was flaky:
///
/// * `amazon/nova-premier-v1` answers 404, "Provider returned error": no endpoint
///   will serve it. This is the Amazon failure a user reported.
/// * `openai/gpt-5.2-chat` answers 404 because every candidate endpoint was
///   removed as BYOK-only. `models.ts` scopes the catalogue to models reachable
///   through the OpenRouter OAuth key and says there is nowhere to paste another
///   provider's key, so this entry can never serve here.
///
/// Both are properties of an endpoint rather than of a moment, which is the test
/// for putting an id on this list. A rate limit or a 5xx is NOT: those are kept
/// and reported, because dropping a model for being busy one afternoon would
/// shrink the catalogue invisibly - the most destructive kind of drift, since
/// nobody sees the entry that quietly disappeared.
pub const UNAVAILABLE_MODEL_IDS: [&str; 2] = [
    "amazon/nova-premier-v1",
    "openai/gpt-5.2-chat",
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
    })
}

/// Parse a `/models` response. Tolerant of extra fields, strict about shape.
pub fn parse_catalog(raw: &serde_json::Value) -> Vec<CatalogEntry> {
    raw.get("data")
        .and_then(|data| data.as_array())
        .map(|list| list.iter().filter_map(parse_entry).collect())
        .unwrap_or_default()
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
