//! Supervision tests: the two guarantees, measured rather than asserted.
//!
//! Spike 001 could not measure these because the host runtime anchors its
//! descendants with its own Job Object, making harness death and product death
//! indistinguishable. These tests exercise the supervisor directly, in-process,
//! so the mechanism under test is the only mechanism in play.
//!
//! 1. The sidecar is assigned to a kill-on-close Job Object.
//! 2. A graceful stop reaches the sidecar's own handler. On Windows a plain
//!    `Child::kill()` is `TerminateProcess` and runs no handler — so this is a
//!    real assertion, not a formality.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use capybaras_shell::paths;
use capybaras_shell::sidecar::Sidecar;

fn state_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "capybaras-test-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("could not create test state dir");
    dir
}

fn log_text(dir: &Path) -> String {
    fs::read_to_string(dir.join("sidecar.log")).unwrap_or_default()
}

fn wait_for(dir: &Path, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if log_text(dir).contains(needle) {
            return true;
        }
        sleep(Duration::from_millis(50));
    }
    false
}

fn spawn(dir: &Path) -> Sidecar {
    Sidecar::spawn(&paths::bundled_node(), &paths::sidecar_script(), dir)
        .expect("sidecar spawn failed")
}

#[test]
fn sidecar_starts_and_is_assigned_to_a_kill_on_close_job() {
    let dir = state_dir("start");
    let mut sidecar = spawn(&dir);

    assert!(
        sidecar.job_assigned(),
        "sidecar was NOT assigned to a job object, so it is not orphan-safe: {:?}",
        sidecar.job_error()
    );
    assert!(
        wait_for(&dir, "sidecar up", Duration::from_secs(20)),
        "sidecar never reported startup. log:\n{}",
        log_text(&dir)
    );

    let _ = sidecar.stop(Duration::from_secs(5));
}

#[test]
fn graceful_stop_reaches_the_sidecars_own_handler() {
    let dir = state_dir("graceful");
    let mut sidecar = spawn(&dir);

    assert!(
        wait_for(&dir, "sidecar up", Duration::from_secs(20)),
        "sidecar never started. log:\n{}",
        log_text(&dir)
    );

    let outcome = sidecar.stop(Duration::from_secs(15));

    assert!(
        outcome.requested,
        "the shutdown request never reached the sidecar's stdin"
    );
    assert!(
        !outcome.forced,
        "the sidecar had to be terminated instead of stopping: {outcome:?}"
    );
    assert!(
        outcome.graceful,
        "the stop was not graceful: {outcome:?}"
    );
    assert_eq!(
        outcome.exit_code,
        Some(0),
        "the sidecar exited with a non-zero code: {outcome:?}"
    );

    // The load-bearing assertion. A forced kill would never produce this line.
    assert!(
        wait_for(&dir, "GRACEFUL STOP RECEIVED", Duration::from_secs(5)),
        "the sidecar's shutdown handler did not run. log:\n{}",
        log_text(&dir)
    );
}

#[test]
fn stopping_an_already_dead_sidecar_is_not_an_error() {
    let dir = state_dir("idempotent");
    let mut sidecar = spawn(&dir);
    assert!(wait_for(&dir, "sidecar up", Duration::from_secs(20)));

    let first = sidecar.stop(Duration::from_secs(15));
    assert!(first.graceful, "first stop was not graceful: {first:?}");

    // Asking twice must be safe — shutdown paths run more than once in practice.
    let second = sidecar.stop(Duration::from_secs(5));
    assert!(
        !second.forced,
        "a second stop should be a no-op, not a kill: {second:?}"
    );
}
