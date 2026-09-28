//! The meter's **wiring**, tested without a window.
//!
//! `tests/usage.rs` proves the arithmetic: tokens add up, USD is formatted
//! honestly, a session is a valid answer before anything happens. This file
//! proves the seam around it — that a finished call is reported to the interface
//! *every single time*, that the snapshot handed over is the one the call
//! produced rather than the previous one, and that the event and field names the
//! frontend reads are the ones the shell actually sends.
//!
//! None of that is arithmetic. All of it is the kind of mistake that renders as
//! "the meter is stuck" on a screen nobody can debug.

use capybaras_shell::meter::{USAGE_EVENT, UsageMeter, apply_credit, record_and_notify};
use capybaras_shell::usage::{Credit, CreditScope, CreditSnapshot, Snapshot, Tokens};

fn tokens(prompt: u64, completion: u64, total: u64) -> Tokens {
    Tokens {
        prompt,
        completion,
        total,
    }
}

#[test]
fn the_event_name_is_the_one_the_frontend_listens_for() {
    // Pinned because it is a contract across the language boundary, where the
    // compiler cannot help: a rename on one side is an event that silently never
    // arrives on the other.
    assert_eq!(USAGE_EVENT, "capybaras://usage");
}

#[test]
fn the_meter_is_readable_before_the_first_call() {
    // The interface reads the meter on load, before any call has happened.
    // "Nothing yet" must be an answer, not an error and not a blank panel.
    let meter = UsageMeter::new();
    let snapshot = meter.snapshot();
    assert_eq!(snapshot.calls, 0);
    assert_eq!(snapshot.total_tokens, 0);
    assert_eq!(snapshot.cost_display, "$0.000000");
}

#[test]
fn every_finished_call_is_reported_even_when_no_total_visibly_changes() {
    // THE RULE THIS WIRING EXISTS FOR.
    //
    // Emitting only on change would hide the case `usage.rs` documents: a call
    // that fails after the provider began generating reports no usage, so the
    // meter stands still *because something went wrong*. A call that moved
    // nothing is still a call, and the interface must be told about it.
    let meter = UsageMeter::new();
    let mut reports: Vec<Snapshot> = Vec::new();

    for _ in 0..3 {
        record_and_notify(&meter, tokens(0, 0, 0), 0.0, |snapshot| {
            reports.push(snapshot.clone())
        });
    }

    assert_eq!(reports.len(), 3, "each call must notify, not only the first");
    assert_eq!(reports[0].calls, 1);
    assert_eq!(reports[2].calls, 3);
    assert_eq!(meter.snapshot().calls, 3);
}

#[test]
fn the_reported_snapshot_is_the_one_the_call_produced_not_the_previous_one() {
    // Off by one here would make the interface lag by exactly one call, which
    // reads as a broken meter rather than a late one — and it would be invisible
    // until the numbers were wrong for a long time.
    let meter = UsageMeter::new();
    let mut reports: Vec<Snapshot> = Vec::new();

    record_and_notify(&meter, tokens(10, 4, 14), 0.000003, |s| reports.push(s.clone()));
    record_and_notify(&meter, tokens(20, 6, 26), 0.000007, |s| reports.push(s.clone()));

    assert_eq!(reports[0].total_tokens, 14);
    assert_eq!(reports[1].calls, 2);
    assert_eq!(reports[1].total_tokens, 40);
    assert!((reports[1].cost_usd - 0.00001).abs() < 1e-12);
}

#[test]
fn the_snapshot_returned_is_the_snapshot_notified() {
    // The command returns the figure *and* emits it. If those two ever came from
    // different reads, the caller and the interface could disagree about the
    // same call.
    let meter = UsageMeter::new();
    let mut notified: Option<Snapshot> = None;
    let returned = record_and_notify(&meter, tokens(3, 1, 4), 0.5, |s| notified = Some(s.clone()));

    assert_eq!(notified.as_ref(), Some(&returned));
    assert_eq!(returned.calls, 1);
}

#[test]
fn a_credit_figure_travels_with_the_next_report() {
    // The interface shows what is left as well as what is spent. The credit is
    // set once and must then ride along with every report, or the panel would
    // lose the balance the moment a call happened.
    let meter = UsageMeter::new();
    meter.set_credit(CreditSnapshot::from_credit(
        Credit {
            total_credits: 95.0,
            total_usage: 89.05,
        },
        CreditScope::Account,
    ));

    let mut notified: Option<Snapshot> = None;
    record_and_notify(&meter, tokens(1, 1, 2), 0.0, |s| notified = Some(s.clone()));

    let credit = notified.expect("a report").credit.expect("credit attached");
    assert!((credit.remaining_usd - 5.95).abs() < 1e-9);
    assert_eq!(credit.remaining_display, "$5.95");
}

// ---------------------------------------------------------------------------
// Attaching a fetched balance, and what happens when the read fails
// ---------------------------------------------------------------------------

#[test]
fn a_fetched_balance_is_attached_and_handed_to_the_interface() {
    let meter = UsageMeter::new();
    let mut notified: Vec<Snapshot> = Vec::new();

    let snapshot = apply_credit(
        &meter,
        Some(CreditSnapshot::from_credit(
            Credit {
                total_credits: 95.0,
                total_usage: 89.05,
            },
            CreditScope::Account,
        )),
        |s| notified.push(s.clone()),
    );

    assert_eq!(notified.len(), 1, "the interface is told the new figure");
    let credit = snapshot.credit.expect("the balance is attached");
    assert!((credit.remaining_usd - 5.95).abs() < 1e-9);
}

#[test]
fn a_read_that_failed_leaves_the_last_known_figure_standing() {
    // THE INVARIANT THIS FUNCTION EXISTS FOR. A balance is money on a screen; a
    // failed read must not blank a good number and must never invent one. `None`
    // means "nothing new was read", so the previous figure stays exactly as it was.
    let meter = UsageMeter::new();
    meter.set_credit(CreditSnapshot::from_credit(
        Credit {
            total_credits: 95.0,
            total_usage: 89.05,
        },
        CreditScope::Account,
    ));

    let snapshot = apply_credit(&meter, None, |_| {});

    let credit = snapshot.credit.expect("the last known figure stands");
    assert!((credit.remaining_usd - 5.95).abs() < 1e-9);
}

#[test]
fn a_failed_read_before_any_figure_leaves_no_row_rather_than_a_zero() {
    // "Nothing could be read" must render as nothing at all, never as a balance of
    // zero, which would read as "you are out of credit" when nobody knows that.
    let meter = UsageMeter::new();
    let snapshot = apply_credit(&meter, None, |_| {});
    assert!(snapshot.credit.is_none(), "no figure is not a zero balance");
}

#[test]
fn the_reported_shape_carries_the_field_names_the_interface_reads() {
    // The frontend reads these by name. A rename on the Rust side would show an
    // empty panel rather than an error, so the names are pinned here.
    let meter = UsageMeter::new();
    let mut reported: Option<Snapshot> = None;
    record_and_notify(&meter, tokens(7, 2, 9), 0.25, |s| reported = Some(s.clone()));

    let json = serde_json::to_value(reported.expect("a report")).expect("a snapshot serialises");

    for field in [
        "calls",
        "prompt_tokens",
        "completion_tokens",
        "total_tokens",
        "cost_usd",
        "cost_display",
        "session_started_unix",
    ] {
        assert!(json.get(field).is_some(), "the report is missing `{field}`: {json}");
    }
    assert_eq!(json["total_tokens"], 9);
    assert_eq!(json["cost_display"], "$0.2500");
}
