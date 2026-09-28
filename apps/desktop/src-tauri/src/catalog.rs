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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogEntry {
    /// The identifier sent to the API. Verbatim: it is an opaque key, not display
    /// text. Note what is NOT here: no tier, no trust level, no policy field.
    pub id: String,
    /// Human-facing, sanitised.
    pub name: String,
    pub description: String,
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
