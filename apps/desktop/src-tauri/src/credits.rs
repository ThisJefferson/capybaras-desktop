//! The account balance, as the provider reports it.
//!
//! **WHY THIS EXISTS.** `usage.rs` can hold a credit figure and compute what is
//! left; `meter.rs` can attach one and the interface already renders the row. None
//! of them ever *read* one, so `Snapshot.credit` was always `None` and the "left"
//! row never appeared — written, tested, and unreachable. This is the missing
//! reader. It is the whole of the defect.
//!
//! **WHAT THIS FIGURE IS, AND WHAT IT IS NOT.** `GET /credits` reports the balance
//! of the whole **account**, shared with every other key on it, so it carries
//! `CreditScope::Account` and the interface labels it "whole account". It is not a
//! spending limit, and nothing here enforces anything with it: D26 settles that the
//! cap question is open and separate, and this module deliberately does not touch a
//! limit. It answers "how much is left", not "what may Capybaras spend".
//!
//! **A REMOTE NUMBER IN FRONT OF A SPENDING DECISION, SO IT IS TREATED AS REMOTE.**
//! The response is shaped by someone else and will be shown to a person deciding
//! whether to keep going. So anything that is not exactly understood — a missing
//! field, a string where a number belongs, a negative or absurd figure — yields
//! *nothing* rather than a plausible-looking number, and the caller then leaves the
//! last known figure standing or shows no row at all. The rule is the catalogue's
//! rule: show nothing rather than something wrong.

use crate::http::Transport;
use crate::usage::{Credit, CreditScope, CreditSnapshot};

/// Where the account balance lives. `GET`, bearer-authenticated.
pub const CREDITS_ENDPOINT: &str = "https://openrouter.ai/api/v1/credits";

/// `GET /credits` describes the account, never this key. Kept as a named constant
/// so the distinction travels with the fetch rather than being a comment someone
/// could forget to read.
pub const CREDIT_SCOPE: CreditScope = CreditScope::Account;

/// The largest balance the app will believe, in USD.
///
/// A figure beyond this is not a balance a person could be reading; it is a shape
/// we do not understand, and an unexplained number on a money screen is worse than
/// no number. Bounding the value is the "clamp" half of treating it as remote.
pub const MAX_CREDITS_USD: f64 = 1_000_000_000.0;

/// Read a `/credits` body, or nothing if it is not the shape we understand.
///
/// Tolerant of extra fields — the provider may add them without warning — and
/// strict about the two that matter. Both must be finite, non-negative numbers
/// within a believable range. Anything else is `None`.
pub fn parse_credit(raw: &serde_json::Value) -> Option<Credit> {
    let data = raw.get("data")?;
    let total_credits = money(data.get("total_credits")?)?;
    let total_usage = money(data.get("total_usage")?)?;
    Some(Credit {
        total_credits,
        total_usage,
    })
}

/// A money field that is safe to display.
///
/// Accepts a numeric string as well as a number, because providers do send money
/// as a string and the same tolerance is given to `usage.cost` in `chat.rs`.
/// Rejects anything that is not a finite, non-negative number of a believable
/// size. Never guesses, never coerces a non-number into a figure.
fn money(value: &serde_json::Value) -> Option<f64> {
    let number = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse::<f64>().ok()))?;

    if !number.is_finite() || number < 0.0 || number > MAX_CREDITS_USD {
        return None;
    }
    Some(number)
}

/// Fetch the account balance with the stored key.
///
/// `None` on every failure — unreachable, an unexpected status, a body that is not
/// JSON, a shape we do not understand. Silently: a balance is *displayed*, and a
/// read that failed is not a reason to change what is already on screen. The
/// caller (see `meter::apply_credit`) leaves the last known figure standing.
pub fn fetch_credits(transport: &dyn Transport, key: &str) -> Option<CreditSnapshot> {
    let response = transport.get(CREDITS_ENDPOINT, key).ok()?;
    if !(200..300).contains(&response.status) {
        return None;
    }

    let body: serde_json::Value = serde_json::from_str(&response.body).ok()?;
    let credit = parse_credit(&body)?;
    Some(CreditSnapshot::from_credit(credit, CREDIT_SCOPE))
}
