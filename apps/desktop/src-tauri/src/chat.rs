//! One chat completion — the first reply.
//!
//! **SMALL ON PURPOSE.** The milestone is *a first reply*, not a chat product.
//! This module builds one request, reads one reply, and reports what it cost.
//! There is no conversation state, no history, and no streaming. Streaming is
//! absent deliberately rather than unbuilt: `usage.rs` names the trap it opens —
//! a stream that never receives a usage block reads as zero, so the meter would
//! look calm and be wrong. Whatever adds streaming must ask for usage first.
//!
//! **REPORTING IS STRUCTURAL, NOT REMEMBERED.** `perform` reports every finished
//! call through one closure, on success and failure alike. That is D22 made
//! impossible to forget: a request that fails after the provider began generating
//! may return no usage, so the call that consumed nothing is reported with zeros.
//! The meter standing still is the evidence of the failure.
//!
//! **A FAILURE IS PLAIN WORDS.** The reader is genuinely non-technical, so no
//! message here carries a status code, a provider body, or an interpolated
//! transport error. The status decides *which* sentence is used; it never appears
//! in one.

use serde::Serialize;

use crate::http::Transport;

/// Where a completion is asked for. Must match the path `tests/live_api.rs` probes.
pub const CHAT_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

/// The most one reply may be. Bounded so a first reply is a first reply and not an
/// invoice — the app does not surprise a new user with a large generation.
pub const MAX_REPLY_TOKENS: u32 = 256;

/// The longest prompt accepted, in characters. A first reply is not a document
/// pipeline; anything longer is refused locally rather than sent and billed.
pub const PROMPT_MAX: usize = 4000;

/// The longest reply kept, in characters. A generous bound against a runaway
/// response burying the interface — not a summary, so it is far above a normal
/// reply at `MAX_REPLY_TOKENS`.
pub const REPLY_MAX: usize = 8000;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatOutcome {
    Replied { text: String, usage: Usage },
    /// `message` is plain words for a non-technical person.
    Failed { message: String },
}

impl ChatOutcome {
    pub fn replied(&self) -> bool {
        matches!(self, Self::Replied { .. })
    }
}

/// What the meter records for this outcome.
///
/// A failure records **zero** — and is still recorded. Returning zeros rather than
/// skipping the report is what makes "the standstill is the signal" true.
pub fn report_of(outcome: &ChatOutcome) -> Usage {
    match outcome {
        ChatOutcome::Replied { usage, .. } => usage.clone(),
        ChatOutcome::Failed { .. } => Usage::default(),
    }
}

/// Build the request body. Contains the prompt; never the key.
pub fn request_body(model: &str, prompt: &str) -> String {
    serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": MAX_REPLY_TOKENS,
    })
    .to_string()
}

/// Make one completion.
pub fn call(transport: &dyn Transport, key: &str, model: &str, prompt: &str) -> ChatOutcome {
    let model = model.trim();
    if model.is_empty() {
        return ChatOutcome::Failed {
            message: "Capybaras has no model chosen yet. Load the model list and pick one."
                .to_string(),
        };
    }

    let prompt = prompt.trim();
    if prompt.is_empty() {
        return ChatOutcome::Failed {
            message: "Type a message first.".to_string(),
        };
    }
    if prompt.chars().count() > PROMPT_MAX {
        return ChatOutcome::Failed {
            message: format!(
                "That message is longer than Capybaras sends in one go. Keep it under {PROMPT_MAX} characters."
            ),
        };
    }

    let response = match transport.post_json(CHAT_ENDPOINT, key, &request_body(model, prompt)) {
        Ok(response) => response,
        Err(_) => {
            return ChatOutcome::Failed {
                message: "Capybaras could not reach OpenRouter. Check your internet connection and try again."
                    .to_string(),
            };
        }
    };

    if !(200..300).contains(&response.status) {
        return ChatOutcome::Failed {
            message: plain_refusal(response.status),
        };
    }

    read_reply(&response.body)
}

/// A sentence for a person who does not know what a status code is.
///
/// The status picks the sentence. It is never printed, never interpolated, and the
/// provider's own body is never echoed — it is the piece most likely to quote the
/// request back.
fn plain_refusal(status: u16) -> String {
    match status {
        400 => "OpenRouter did not accept the request. If this keeps happening, pick a different model."
            .to_string(),
        401 | 403 => {
            "OpenRouter refused the request. The sign-in may have been revoked, or that model may not be available to you — use Connect again."
                .to_string()
        }
        402 => "Your OpenRouter account is out of credit, so the message was not sent. Add credit in your OpenRouter account and try again."
            .to_string(),
        404 => "OpenRouter does not recognise that model. Pick a different one from the list."
            .to_string(),
        408 | 504 => "OpenRouter took too long to answer. Try again.".to_string(),
        429 => "OpenRouter is asking Capybaras to slow down. Wait a moment and try again."
            .to_string(),
        500..=599 => "OpenRouter had a problem on its side. Try again in a moment.".to_string(),
        _ => "OpenRouter could not complete the request. Try again in a moment.".to_string(),
    }
}

/// Read the reply text and its usage, or explain that there was none.
fn read_reply(body: &str) -> ChatOutcome {
    let parsed: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return ChatOutcome::Failed {
                message: "OpenRouter's reply could not be read. Try again in a moment.".to_string(),
            };
        }
    };

    // A proxy can answer 200 with an error envelope. Showing that as the model's
    // answer would be the meter's "looks calm and is wrong" failure, in speech.
    if parsed.get("error").map(|error| !error.is_null()).unwrap_or(false) {
        return ChatOutcome::Failed {
            message: "OpenRouter reported an error instead of a reply. Try again in a moment."
                .to_string(),
        };
    }

    let text = parsed
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|list| list.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(clamp_reply)
        .unwrap_or_default();

    if text.is_empty() {
        return ChatOutcome::Failed {
            message: "OpenRouter returned an empty reply. Try again.".to_string(),
        };
    }

    ChatOutcome::Replied {
        text,
        usage: read_usage(&parsed),
    }
}

/// Prepare model output for the screen.
///
/// Newlines are legitimate in a reply and are kept; carriage returns are dropped so
/// they cannot move the cursor on their own, and the length is bounded so a runaway
/// response cannot bury the interface. The frontend renders this with
/// `textContent`, so it is text and stays text (T3/T4).
fn clamp_reply(text: &str) -> String {
    let cleaned = text.replace('\r', "");
    let trimmed = cleaned.trim();
    if trimmed.chars().count() <= REPLY_MAX {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(REPLY_MAX.saturating_sub(1)).collect();
    format!("{kept}\u{2026}")
}

/// The provider's own figures, or zero. This app does not estimate.
fn read_usage(parsed: &serde_json::Value) -> Usage {
    let usage = parsed.get("usage");

    let count = |name: &str| usage.and_then(|u| u.get(name)).and_then(as_u64).unwrap_or(0);
    let prompt_tokens = count("prompt_tokens");
    let completion_tokens = count("completion_tokens");
    let sent_total = count("total_tokens");
    let cost_usd = usage
        .and_then(|u| u.get("cost"))
        .and_then(as_f64)
        .filter(|cost| *cost > 0.0)
        .unwrap_or(0.0);

    Usage {
        prompt_tokens,
        completion_tokens,
        // Prefer the provider's total; derive only when it sent none, so we never
        // contradict a figure the provider did send.
        total_tokens: if sent_total > 0 {
            sent_total
        } else {
            prompt_tokens + completion_tokens
        },
        cost_usd,
    }
}

fn as_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_u64()
}

/// Some providers send money as a string. Accept either rather than reporting zero
/// for a real cost, which would make the meter quietly wrong.
fn as_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse::<f64>().ok()))
        .filter(|number| number.is_finite())
}

/// Run one call and report it — **always**.
///
/// `on_usage` is called exactly once, for a success and for a failure alike, with
/// the figure the meter should record. There is no path through this function that
/// skips the report, which is the point: the caller cannot forget the case the
/// meter most needs to see. `on_outcome` receives the outcome itself, so the shell
/// decides how to show it and this module stays free of any UI concern.
pub fn perform<U, D>(
    transport: &dyn Transport,
    key: &str,
    model: &str,
    prompt: &str,
    mut on_usage: U,
    mut on_outcome: D,
) -> ChatOutcome
where
    U: FnMut(&Usage),
    D: FnMut(&ChatOutcome),
{
    let outcome = call(transport, key, model, prompt);
    on_usage(&report_of(&outcome));
    on_outcome(&outcome);
    outcome
}
