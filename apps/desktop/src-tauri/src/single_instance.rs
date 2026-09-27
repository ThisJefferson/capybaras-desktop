//! Single-instance guard.
//!
//! Two agents running at once is not a cosmetic problem. This project has
//! already been bitten by exactly that pattern elsewhere: two Ollama servers on
//! one machine contended for the same GPU and produced crashes and hangs. A
//! second Capybaras would contend for the same state directory, the same
//! credentials, and the same sidecar port.
//!
//! Mechanism: an exclusively-opened lock file. The handle is held for the
//! lifetime of the process, and Windows releases it when the process dies —
//! including when it dies badly. No stale lock to clean up, no pid to validate.

use std::fs::{File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

/// Held for the lifetime of the process. Dropping it releases the lock.
pub struct InstanceLock {
    // Never read. The guarantee *is* the open handle.
    #[allow(dead_code)]
    file: File,
}

pub fn acquire(lock_path: &Path) -> Option<InstanceLock> {
    if let Some(dir) = lock_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    // `share_mode(0)` means "no sharing". A second process asking for the same
    // file is refused while this handle lives.
    match OpenOptions::new()
        .create(true)
        .write(true)
        .share_mode(0)
        .open(lock_path)
    {
        Ok(file) => Some(InstanceLock { file }),
        // Someone else holds it. There is no useful distinction to make here
        // between "held" and "unreadable" — either way, do not start a second agent.
        Err(_) => None,
    }
}
