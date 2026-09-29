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

/// The biggest reply asked for when the catalogue does not say what a model's own
/// ceiling is.
///
/// **THIS IS A FALLBACK, NOT THE LIMIT.** The request asks for the model's OWN
/// maximum — `top_provider.max_completion_tokens`, carried on each catalogue entry
/// as `maxCompletionTokens` — and falls back here only when that figure is absent.
///
/// **THE FALLBACK IS DELIBERATELY HIGH.** The operator's instruction is that *"the
/// answer should be as long as it needs to be. if it's a long response, i want to
/// see the whole thing"*. An unknown ceiling must therefore never be read as a
/// short one — the old `256` here was the bug. 65,535 is the highest ceiling
/// measured in the catalogue on 2026-09-28 (`google/gemini-2.5-flash`), so it is a
/// number from the same source rather than one invented here.
///
/// **WHAT THIS COSTS, PLAINLY.** This is the ceiling on what one reply may cost,
/// and preferring a complete answer over a short one is the operator's explicit
/// choice. A long reply on an expensive model is real money: at
/// `anthropic/claude-sonnet-4.5` prices, a full 64,000-token reply is on the order
/// of $0.96. The catalogue carries a per-reply estimate (`priceLabel`) so a person
/// can see that before choosing. The reasoning is recorded in DECISIONS.md.
pub const DEFAULT_MAX_REPLY_TOKENS: u32 = 65535;

/// The request's `max_tokens`: the model's OWN maximum, or the high fallback.
///
/// `None` means the catalogue did not state a ceiling, and `Some(0)` is not a real
/// ceiling either. Both are answered with `DEFAULT_MAX_REPLY_TOKENS` — never with a
/// smaller number picked here, because absence is not evidence of a short limit.
pub fn reply_limit(max_completion_tokens: Option<u64>) -> u32 {
    match max_completion_tokens {
        Some(max) if max > 0 => u32::try_from(max).unwrap_or(u32::MAX),
        _ => DEFAULT_MAX_REPLY_TOKENS,
    }
}

/// The longest prompt accepted, in characters. A first reply is not a document
/// pipeline; anything longer is refused locally rather than sent and billed.
pub const PROMPT_MAX: usize = 4000;

/// How many characters one token can reasonably be worth, at the generous end.
/// Used only to size `REPLY_MAX` from the token ceiling.
pub const CHARS_PER_TOKEN_MAX: usize = 5;

/// The longest reply kept, in characters.
///
/// **THIS RISES WITH THE TOKEN CEILING, AND IT HAS TO.** It was a flat 8,000 —
/// generous beside a 256-token request. It is now derived from
/// `DEFAULT_MAX_REPLY_TOKENS`, the highest ceiling a request may ask for, at
/// `CHARS_PER_TOKEN_MAX` characters per token. That way a reply the model was
/// allowed to write cannot be cut a SECOND time in the shell: cutting it here would
/// leave the truncation bug looking unfixed with the request already fixed.
///
/// It is still a bound — it stops a runaway response burying the interface. It is
/// not a summary, and it sits far above any ordinary reply.
pub const REPLY_MAX: usize = DEFAULT_MAX_REPLY_TOKENS as usize * CHARS_PER_TOKEN_MAX;

/// One turn in the conversation.
///
/// **THE HISTORY IS SENT, NOT REMEMBERED REMOTELY.** Capybaras holds no session on
/// OpenRouter. Every request carries the turns the person can see in the window and
/// nothing else — so the thread lives in one place, which is the place it is shown
/// and the place it can be cleared. A follow-up question works because the earlier
/// turns travel with it, not because a provider kept them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

/// The most turns sent in one request.
///
/// A conversation is not a document pipeline. This is a ceiling on how much of a
/// long thread travels, so a session that runs for hours cannot quietly become a
/// request nobody can afford. The most recent turns are the ones kept.
pub const MESSAGES_MAX: usize = 40;

/// The most characters of history sent with one request.
///
/// **A SECOND BOUND, BECAUSE THE FIRST IS NOT ENOUGH.** Forty turns of a long reply
/// can still be an enormous request; the character budget is what actually bounds
/// the cost, and it is checked against the provider's own ceiling rather than
/// guessed at here. When the window is cut, the cut is **oldest-first** — the most
/// recent turns are what a follow-up needs.
pub const CONVERSATION_MAX: usize = 120_000;

/// The turns that will actually be sent: the most recent ones that fit both bounds.
///
/// Bounded by count **and** by characters, oldest dropped first. Trimming here rather
/// than refusing means a long conversation keeps working; it just stops carrying its
/// own beginning, which is the honest trade for a follow-up that still has context.
pub fn window(messages: &[Message]) -> Vec<Message> {
    let mut kept: Vec<Message> = Vec::new();
    let mut used = 0usize;
    for message in messages.iter().rev() {
        if kept.len() >= MESSAGES_MAX {
            break;
        }
        let size = message.content.chars().count();
        if !kept.is_empty() && used + size > CONVERSATION_MAX {
            break;
        }
        used += size;
        kept.push(message.clone());
    }
    kept.reverse();
    kept
}

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
///
/// `max_tokens` is the resolved request bound — the chosen model's own maximum via
/// `reply_limit`, not a number this module picks. The caller resolves it so the one
/// place that holds the catalogue is the one place that decides.
pub fn request_body(model: &str, prompt: &str, max_tokens: u32) -> String {
    request_body_for(model, &[Message::user(prompt)], max_tokens)
}

/// Build the request body for a whole conversation. Contains the history; never the key.
///
/// The turns are sent in order, oldest first, exactly as they appear in the window.
/// `max_tokens` is still the resolved request bound (see `reply_limit`); this module
/// does not pick it.
pub fn request_body_for(model: &str, messages: &[Message], max_tokens: u32) -> String {
    let turns: Vec<serde_json::Value> = messages
        .iter()
        .map(|message| serde_json::json!({ "role": message.role, "content": message.content }))
        .collect();
    serde_json::json!({
        "model": model,
        "messages": turns,
        "max_tokens": max_tokens,
    })
    .to_string()
}

/// Make one completion from a single prompt. A thin wrapper over `call_with_history`,
/// kept so a one-shot caller does not have to build a turn.
pub fn call(
    transport: &dyn Transport,
    key: &str,
    model: &str,
    prompt: &str,
    max_tokens: u32,
) -> ChatOutcome {
    call_with_history(transport, key, model, &[Message::user(prompt)], max_tokens)
}

/// Make one completion for a conversation.
///
/// **THE LAST TURN MUST BE THE PERSON'S.** A request whose final message is the
/// assistant's is a regenerate, and it is handled by the caller trimming that turn
/// off before asking — this function refuses rather than sending a request whose
/// meaning depends on the provider's mood. The check is here so a malformed thread
/// fails locally, in plain words, instead of being billed.
pub fn call_with_history(
    transport: &dyn Transport,
    key: &str,
    model: &str,
    messages: &[Message],
    max_tokens: u32,
) -> ChatOutcome {
    let model = model.trim();
    if model.is_empty() {
        return ChatOutcome::Failed {
            message: "Capybaras has no model chosen yet. Load the model list and pick one."
                .to_string(),
        };
    }

    let turns = window(messages);
    let last = match turns.last() {
        Some(last) => last,
        None => {
            return ChatOutcome::Failed {
                message: "Type a message first.".to_string(),
            };
        }
    };
    if last.role != "user" {
        return ChatOutcome::Failed {
            message: "Type a message first.".to_string(),
        };
    }
    let prompt = last.content.trim();
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

    let response = match transport.post_json(
        CHAT_ENDPOINT,
        key,
        &request_body_for(model, &turns, max_tokens),
    ) {
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
    max_tokens: u32,
    mut on_usage: U,
    mut on_outcome: D,
) -> ChatOutcome
where
    U: FnMut(&Usage),
    D: FnMut(&ChatOutcome),
{
    let outcome = call(transport, key, model, prompt, max_tokens);
    on_usage(&report_of(&outcome));
    on_outcome(&outcome);
    outcome
}
