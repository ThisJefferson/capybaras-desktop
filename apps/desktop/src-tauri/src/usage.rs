//! What this session has spent.
//!
//! **THE NUMBERS COME FROM RESPONSES, NOT FROM POLLING, AND THAT IS THE WHOLE
//! DESIGN.** Every completion already carries an authoritative `usage.cost` in USD
//! from the provider, alongside token counts — verified live against OpenRouter (see
//! `tests/live_api.rs`). Polling an endpoint for the same figure would cost requests,
//! drift out of date, and still be wrong for anything in flight. Counting as a side
//! effect of work the app is already doing is exact, free, and always current.
//!
//! **THIS DELIBERATELY DOES NOT ESTIMATE.** A price table plus arithmetic produces a
//! number that disagrees with the invoice. For anything already incurred, the
//! provider's own figure is strictly better, so there is no estimation code here to
//! drift away from the truth.
//!
//! **TWO TRAPS, WRITTEN DOWN RATHER THAN LEFT TO BE REDISCOVERED:**
//!
//! 1. **Streaming.** With a streaming request, usage arrives only if the request asks
//!    for it. A stream that never receives a usage block reads as zero here — the
//!    meter would look calm and be wrong. Whatever adds streaming must ask for usage.
//! 2. **Failed calls.** A request that fails after the provider has begun generating
//!    may not return usage at all, so it costs money and appears as nothing. The
//!    account-level figure is the backstop for that gap; the session total is exact
//!    for calls that reported, and silent about calls that did not.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub prompt: u64,
    pub completion: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals {
    pub calls: u64,
    pub tokens: Tokens,
    pub cost_usd: f64,
}

impl Totals {
    /// Add one call's usage.
    pub fn record(&mut self, tokens: Tokens, cost_usd: f64) {
        self.calls += 1;
        self.tokens.prompt += tokens.prompt;
        self.tokens.completion += tokens.completion;
        // Prefer the provider's total. Derive only when it did not send one, so we
        // never contradict a figure the provider did send.
        self.tokens.total += if tokens.total > 0 {
            tokens.total
        } else {
            tokens.prompt + tokens.completion
        };
        // Clamped at zero: a negative cost is not a thing, and letting one subtract
        // from the session would quietly hide real spend rather than show an anomaly.
        self.cost_usd += if cost_usd > 0.0 { cost_usd } else { 0.0 };
    }
}

/// Whether a credit figure describes the whole account or just this key.
///
/// **THIS DISTINCTION IS THE MOST IMPORTANT THING ON THIS SCREEN.** The provider's
/// account total is shared with anything else using the same account, so it cannot
/// answer "what has this app spent". Only a limit set on the key narrows it. Showing
/// an account-wide number as if it were the app's budget would be a quiet lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreditScope {
    /// The whole account, shared with every other key on it.
    Account,
    /// Narrowed to this key by a limit set in the provider's dashboard.
    Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Credit {
    pub total_credits: f64,
    pub total_usage: f64,
}

impl Credit {
    /// What is left. Signed, because a negative value means the account went past its
    /// credits and that is a real state worth showing rather than clamping to zero.
    pub fn remaining(&self) -> f64 {
        self.total_credits - self.total_usage
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreditSnapshot {
    pub remaining_usd: f64,
    pub remaining_display: String,
    pub total_credits: f64,
    pub total_usage: f64,
    pub fetched_at_unix: u64,
    pub scope: CreditScope,
}

impl CreditSnapshot {
    pub fn from_credit(credit: Credit, scope: CreditScope) -> Self {
        Self {
            remaining_usd: credit.remaining(),
            remaining_display: format_usd(credit.remaining()),
            total_credits: credit.total_credits,
            total_usage: credit.total_usage,
            fetched_at_unix: now_unix(),
            scope,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub cost_display: String,
    pub session_started_unix: u64,
    pub credit: Option<CreditSnapshot>,
}

/// Format a USD amount with enough precision to be honest.
///
/// A single small call can cost a fraction of a cent. Fixed two decimals would render
/// a real cost as `$0.00`, which reads as "this is free" — the opposite of the truth,
/// on a screen whose whole job is showing what things cost.
pub fn format_usd(amount: f64) -> String {
    let magnitude = amount.abs();
    if magnitude >= 1.0 {
        format!("${amount:.2}")
    } else if magnitude >= 0.01 {
        format!("${amount:.4}")
    } else {
        format!("${amount:.6}")
    }
}

/// The session's running totals, shared across threads.
///
/// The shell makes the provider calls and the interface asks for the numbers, so this
/// is touched from both directions. A mutex rather than atomics because the four
/// counters have to move together — a snapshot that caught tokens updated and cost not
/// yet would be internally inconsistent.
#[derive(Debug)]
pub struct Session {
    started_unix: u64,
    totals: Mutex<Totals>,
    credit: Mutex<Option<CreditSnapshot>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            started_unix: now_unix(),
            totals: Mutex::new(Totals::default()),
            credit: Mutex::new(None),
        }
    }

    pub fn record(&self, tokens: Tokens, cost_usd: f64) {
        match self.totals.lock() {
            Ok(mut totals) => totals.record(tokens, cost_usd),
            Err(poisoned) => poisoned.into_inner().record(tokens, cost_usd),
        }
    }

    pub fn set_credit(&self, credit: CreditSnapshot) {
        match self.credit.lock() {
            Ok(mut held) => *held = Some(credit),
            Err(poisoned) => *poisoned.into_inner() = Some(credit),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let totals = match self.totals.lock() {
            Ok(guard) => *guard,
            Err(poisoned) => *poisoned.into_inner(),
        };
        let credit = match self.credit.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };

        Snapshot {
            calls: totals.calls,
            prompt_tokens: totals.tokens.prompt,
            completion_tokens: totals.tokens.completion,
            total_tokens: totals.tokens.total,
            cost_usd: totals.cost_usd,
            cost_display: format_usd(totals.cost_usd),
            session_started_unix: self.started_unix,
            credit,
        }
    }
}
