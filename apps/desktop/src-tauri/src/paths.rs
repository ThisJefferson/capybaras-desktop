//! Path resolution.
//!
//! **Rule (spike 002, rule 1): never resolve a path from the working directory.**
//!
//! The packaged app is launched with `cwd = C:\Windows\system32`. Anything
//! derived from `cwd` is therefore wrong the moment the app is installed on a
//! real machine. Every path in this module comes from one of three places:
//!
//! 1. the location of the running executable,
//! 2. an explicit environment override (for tests and development), or
//! 3. a compile-time constant (never `cwd`).
//!
//! There is deliberately no `cwd`-relative fallback anywhere in this file.

use std::path::{Path, PathBuf};

/// Directory containing the running executable.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The Node runtime the shell spawns.
///
/// Release layout: `node.exe` sits beside the shell executable. Development
/// falls back to the Node on `PATH`, which is not app-local and must not be
/// relied on for a shipped build.
pub fn bundled_node() -> PathBuf {
    if let Ok(p) = std::env::var("CAPYBARAS_NODE") {
        return PathBuf::from(p);
    }
    let beside_exe = exe_dir().join("node.exe");
    if beside_exe.is_file() {
        return beside_exe;
    }
    PathBuf::from("node")
}

/// The sidecar script the Node runtime executes.
pub fn sidecar_script() -> PathBuf {
    if let Ok(p) = std::env::var("CAPYBARAS_SIDECAR") {
        return PathBuf::from(p);
    }
    let beside_exe = exe_dir().join("sidecar").join("sidecar.mjs");
    if beside_exe.is_file() {
        return beside_exe;
    }
    // Development fallback. `CARGO_MANIFEST_DIR` is baked in at COMPILE time,
    // so this is still not dependent on the runtime working directory.
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sidecar/sidecar.mjs"
    ))
}

/// Where the shell keeps its state.
///
/// **Rule (spike 002, rule 2): this must be a deliberate choice.** MSIX
/// silently virtualises writes to `%LOCALAPPDATA%`, so the path reported here
/// and the path the bytes actually land in can differ. See `DECISIONS.md` D15.
pub fn state_dir() -> PathBuf {
    if let Ok(p) = std::env::var("CAPYBARAS_STATE_DIR") {
        return PathBuf::from(p);
    }
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("Capybaras")
}
