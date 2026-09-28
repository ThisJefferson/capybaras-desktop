//! The meter's wiring into the shell.
//!
//! `usage.rs` is the arithmetic, and it deliberately knows nothing about Tauri:
//! it accumulates, it formats, it never emits. This module is the seam between
//! that arithmetic and the interface, and it holds exactly one rule.
//!
//! **EVERY FINISHED MODEL CALL IS REPORTED, NOT ONLY THE ONES THAT MOVE A
//! TOTAL.** The cheaper-looking alternative -- diff the snapshot and emit when
//! something changed -- is worse here, for the reason `usage.rs` gives in its own
//! header: a call that fails after the provider has begun generating may return
//! no usage at all. That call therefore changes nothing, and `no change` is
//! exactly the case a person needs to see, because the meter standing still is
//! the *evidence of the failure*. It also removes a whole class of bug where a
//! counter moves without the display being told, and it means the interface's
//! picture is refreshed by each call rather than by our guess about which calls
//! mattered.
//!
//! The cost is one small JSON message per call on a channel the app already
//! owns, against a call that just crossed the network. That is the cheaper
//! bargain.

use crate::usage::{CreditSnapshot, Session, Snapshot, Tokens};

/// The event the interface listens on.
///
/// Named with the app's other `capybaras://` events, and public so the emitter
/// and the tests cannot drift apart.
pub const USAGE_EVENT: &str = "capybaras://usage";

/// The session's meter, as the shell holds it.
///
/// Holds no Tauri types on purpose: the emitting is done by the command, so the
/// behaviour here stays testable without a window.
#[derive(Debug, Default)]
pub struct UsageMeter {
    session: Session,
}

impl UsageMeter {
    pub fn new() -> Self {
        Self {
            session: Session::new(),
        }
    }

    /// The current picture. Valid before the first call (zero calls, zero spend).
    pub fn snapshot(&self) -> Snapshot {
        self.session.snapshot()
    }

    /// Attach a credit figure fetched elsewhere -- the account balance, as the
    /// provider reports it. Kept here rather than in the interface so the number
    /// on screen and the number reported alongside a call are the same number.
    pub fn set_credit(&self, credit: CreditSnapshot) {
        self.session.set_credit(credit);
    }

    /// Record a finished call and return the new picture.
    pub fn record(&self, tokens: Tokens, cost_usd: f64) -> Snapshot {
        self.session.record(tokens, cost_usd);
        self.session.snapshot()
    }
}

/// Record a finished model call, then hand the **new** snapshot to `notify`.
///
/// The snapshot handed over is taken *after* recording, so what the interface
/// receives is the state the call produced. Handing over the previous snapshot
/// would make the meter lag by exactly one call, which reads as a broken meter
/// rather than a late one.
pub fn record_and_notify<N>(meter: &UsageMeter, tokens: Tokens, cost_usd: f64, mut notify: N) -> Snapshot
where
    N: FnMut(&Snapshot),
{
    let snapshot = meter.record(tokens, cost_usd);
    notify(&snapshot);
    snapshot
}
