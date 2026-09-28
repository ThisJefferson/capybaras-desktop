//! The meter's core, tested without a network.
//!
//! The arithmetic here is what a person reads to decide whether to keep going, so the
//! tests are about being *honest* rather than merely correct: a number that looks calm
//! while spend continues is worse than no number.

use capybaras_shell::usage::{Credit, CreditScope, Session, Tokens, Totals, format_usd};

fn tokens(prompt: u64, completion: u64, total: u64) -> Tokens {
    Tokens {
        prompt,
        completion,
        total,
    }
}

// ---------------------------------------------------------------------------
// Accumulating
// ---------------------------------------------------------------------------

#[test]
fn recording_a_call_adds_every_counter() {
    let mut totals = Totals::default();
    totals.record(tokens(14, 1, 15), 2.673e-6);

    assert_eq!(totals.calls, 1);
    assert_eq!(totals.tokens.prompt, 14);
    assert_eq!(totals.tokens.completion, 1);
    assert_eq!(totals.tokens.total, 15);
    assert!((totals.cost_usd - 2.673e-6).abs() < 1e-12);
}

#[test]
fn many_calls_sum_without_drifting() {
    // The real shape of a session: hundreds of small calls. A float that loses a
    // fraction of a cent per call would under-report by the end of one.
    let mut totals = Totals::default();
    for _ in 0..1_000 {
        totals.record(tokens(10, 5, 15), 0.0001);
    }

    assert_eq!(totals.calls, 1_000);
    assert_eq!(totals.tokens.total, 15_000);
    assert!(
        (totals.cost_usd - 0.1).abs() < 1e-9,
        "1000 calls at $0.0001 should be $0.10, got {}",
        totals.cost_usd
    );
}

#[test]
fn a_missing_total_is_derived_from_the_parts() {
    let mut totals = Totals::default();
    totals.record(tokens(30, 12, 0), 0.0);
    assert_eq!(totals.tokens.total, 42);
}

#[test]
fn a_provided_total_is_preferred_over_the_derivation() {
    // If the provider sends a total, it wins. Deriving our own and disagreeing with
    // it would put two different numbers on the same screen.
    let mut totals = Totals::default();
    totals.record(tokens(30, 12, 100), 0.0);
    assert_eq!(totals.tokens.total, 100);
}

#[test]
fn a_negative_cost_is_refused_rather_than_subtracted() {
    // A negative cost is not a thing. Letting one through would quietly reduce the
    // session total and hide real spend, which is the worst possible failure for this
    // particular number.
    let mut totals = Totals::default();
    totals.record(tokens(1, 1, 2), 0.5);
    totals.record(tokens(1, 1, 2), -10.0);

    assert_eq!(totals.calls, 2, "the call still happened");
    assert!(
        (totals.cost_usd - 0.5).abs() < 1e-12,
        "a negative cost reduced the total: {}",
        totals.cost_usd
    );
}

// ---------------------------------------------------------------------------
// Showing it honestly
// ---------------------------------------------------------------------------

#[test]
fn small_amounts_are_not_rounded_into_looking_free() {
    // The failure this prevents: a real cost rendered as "$0.00", which reads as
    // "this is free" on the one screen whose job is saying what things cost.
    assert_eq!(format_usd(0.000002673), "$0.000003");
    assert_ne!(format_usd(0.000002673), "$0.00");
    assert_eq!(format_usd(0.0123), "$0.0123");
    assert_eq!(format_usd(1.5), "$1.50");
    assert_eq!(format_usd(123.456), "$123.46");
    assert_eq!(format_usd(0.0), "$0.000000");
}

#[test]
fn a_snapshot_of_nothing_is_still_a_valid_answer() {
    // Before the first call, "nothing yet" must be representable -- not an error and
    // not a blank.
    let snapshot = Session::new().snapshot();
    assert_eq!(snapshot.calls, 0);
    assert_eq!(snapshot.total_tokens, 0);
    assert_eq!(snapshot.cost_usd, 0.0);
    assert_eq!(snapshot.cost_display, "$0.000000");
    assert!(snapshot.credit.is_none());
    assert!(snapshot.session_started_unix > 0);
}

#[test]
fn a_snapshot_reflects_everything_recorded() {
    let session = Session::new();
    session.record(tokens(10, 4, 14), 0.000003);
    session.record(tokens(20, 6, 26), 0.000007);

    let snapshot = session.snapshot();
    assert_eq!(snapshot.calls, 2);
    assert_eq!(snapshot.prompt_tokens, 30);
    assert_eq!(snapshot.completion_tokens, 10);
    assert_eq!(snapshot.total_tokens, 40);
    assert!((snapshot.cost_usd - 0.00001).abs() < 1e-12);
    assert_eq!(snapshot.cost_display, "$0.000010");
}

// ---------------------------------------------------------------------------
// Credit, and being clear about what it describes
// ---------------------------------------------------------------------------

#[test]
fn remaining_is_the_difference() {
    let credit = Credit {
        total_credits: 95.0,
        total_usage: 89.05,
    };
    assert!((credit.remaining() - 5.95).abs() < 1e-9);
}

#[test]
fn going_past_the_credits_stays_visible_rather_than_clamping_to_zero() {
    // Clamping would show "$0.000000 left" when the truth is "you are over". The
    // number exists to inform a decision; a clamped one misinforms it.
    let credit = Credit {
        total_credits: 10.0,
        total_usage: 12.5,
    };
    assert!((credit.remaining() + 2.5).abs() < 1e-9);
}

#[test]
fn an_account_wide_figure_is_labelled_as_account_wide() {
    // THE POINT OF THE SCOPE FIELD. An account total is shared with every other key on
    // the account, so it cannot answer "what has this app spent". Presenting it as the
    // app's own budget would be a quiet lie, so the distinction is carried in the data
    // rather than left to a caption someone might forget to write.
    let account = capybaras_shell::usage::CreditSnapshot::from_credit(
        Credit {
            total_credits: 95.0,
            total_usage: 89.05,
        },
        CreditScope::Account,
    );
    assert_eq!(account.scope, CreditScope::Account);
    // Two decimals above a dollar: a balance is a balance.
    assert_eq!(account.remaining_display, "$5.95");
    assert!(account.fetched_at_unix > 0);

    let key = capybaras_shell::usage::CreditSnapshot::from_credit(
        Credit {
            total_credits: 5.0,
            total_usage: 1.25,
        },
        CreditScope::Key,
    );
    assert_eq!(key.scope, CreditScope::Key);
    assert_eq!(key.remaining_display, "$3.75");
}

#[test]
fn a_credit_snapshot_attaches_to_the_session() {
    let session = Session::new();
    assert!(session.snapshot().credit.is_none());

    session.set_credit(capybaras_shell::usage::CreditSnapshot::from_credit(
        Credit {
            total_credits: 95.0,
            total_usage: 89.05,
        },
        CreditScope::Account,
    ));

    let snapshot = session.snapshot();
    let credit = snapshot.credit.expect("credit should be attached");
    assert!((credit.remaining_usd - 5.95).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------------

#[test]
fn concurrent_recording_sums_correctly() {
    // The shell records from the thread making a call while the interface reads. If
    // the counters could be read half-updated, the interface would show a total that
    // never existed.
    let session = std::sync::Arc::new(Session::new());
    let mut handles = Vec::new();

    for _ in 0..8 {
        let session = std::sync::Arc::clone(&session);
        handles.push(std::thread::spawn(move || {
            for _ in 0..250 {
                session.record(tokens(4, 2, 6), 0.000001);
            }
        }));
    }
    for handle in handles {
        handle.join().expect("thread");
    }

    let snapshot = session.snapshot();
    assert_eq!(snapshot.calls, 2_000);
    assert_eq!(snapshot.total_tokens, 12_000);
    assert_eq!(snapshot.prompt_tokens, 8_000);
    assert_eq!(snapshot.completion_tokens, 4_000);
    assert!(
        (snapshot.cost_usd - 0.002).abs() < 1e-9,
        "2000 calls at $0.000001 should be $0.002, got {}",
        snapshot.cost_usd
    );
}
