// Prevents an additional console window on Windows in release. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Single-instance guard, acquired before anything else starts. Two agents
    // sharing one state directory and one set of credentials is a safety
    // problem, not a tidiness problem.
    let lock_path = capybaras_shell::paths::state_dir().join("shell.lock");
    let _lock = match capybaras_shell::single_instance::acquire(&lock_path) {
        Some(lock) => lock,
        None => {
            eprintln!(
                "Capybaras is already running (lock held at {}).",
                lock_path.display()
            );
            std::process::exit(0);
        }
    };

    capybaras_shell::run();

    // `_lock` drops here, releasing the lock file.
}
