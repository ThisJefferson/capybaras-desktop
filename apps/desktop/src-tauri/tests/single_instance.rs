//! Single-instance guard tests.
//!
//! This is a safety property, not tidiness: two agents sharing one state
//! directory and one set of credentials is exactly the failure this project
//! exists to prevent. It gets a test.

use std::fs;

use capybaras_shell::single_instance;

#[test]
fn a_second_instance_is_refused_while_the_first_holds_the_lock() {
    let dir = std::env::temp_dir().join(format!("capybaras-lock-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("shell.lock");

    let first = single_instance::acquire(&path);
    assert!(first.is_some(), "the first acquire should succeed");

    let second = single_instance::acquire(&path);
    assert!(
        second.is_none(),
        "a second acquire must be refused while the first holds the lock"
    );

    drop(first);

    // The lock is released by the process dying, not by cleanup code, so it
    // must also release when the holder simply goes away.
    let third = single_instance::acquire(&path);
    assert!(
        third.is_some(),
        "the lock must be released when the holder is dropped"
    );
}
