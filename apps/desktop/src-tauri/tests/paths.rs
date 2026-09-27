//! Path resolution tests.
//!
//! These encode spike 002, rule 1: the packaged app is launched with
//! `cwd = C:\Windows\system32`, so nothing may be resolved relative to the
//! working directory. A `cwd`-relative path is broken the moment the app is
//! installed, and it fails *quietly* — which is the worst kind of failure.

use capybaras_shell::paths;

#[test]
fn resolved_paths_are_absolute_never_cwd_relative() {
    assert!(
        paths::exe_dir().is_absolute(),
        "exe_dir must be absolute, got {:?}",
        paths::exe_dir()
    );
    assert!(
        paths::state_dir().is_absolute(),
        "state_dir must be absolute, got {:?}",
        paths::state_dir()
    );
    assert!(
        paths::sidecar_script().is_absolute(),
        "sidecar script must be absolute, got {:?}",
        paths::sidecar_script()
    );
}

#[test]
fn sidecar_script_resolves_to_a_real_file() {
    let script = paths::sidecar_script();
    assert!(
        script.is_file(),
        "sidecar script does not exist at the resolved path: {}",
        script.display()
    );
}

#[test]
fn explicit_environment_override_wins() {
    // Guard: the override exists for tests and development. If it silently
    // stopped working, tests would start exercising the wrong file.
    let key = "CAPYBARAS_STATE_DIR";
    let previous = std::env::var(key).ok();
    unsafe { std::env::set_var(key, r"C:\capybaras-test-override") };

    let resolved = paths::state_dir();

    match previous {
        Some(v) => unsafe { std::env::set_var(key, v) },
        None => unsafe { std::env::remove_var(key) },
    }

    assert_eq!(
        resolved,
        std::path::PathBuf::from(r"C:\capybaras-test-override")
    );
}
