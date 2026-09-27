//! Credential storage in the operating system's credential manager.
//!
//! **docs/threat-model.md, T12.** The provider key is the authority to spend money
//! and read data, and the agent runs as the same user account as this app. So a
//! key on disk in a readable file is one the agent can read and then use
//! **directly** — no gate, no approval, no card. The product's central promise
//! becomes optional the moment its credential is reachable.
//!
//! **Why this lives in the Rust shell and not the sidecar.** The sidecar is the
//! component that runs alongside untrusted content; the webview is the component
//! an XSS would reach. Neither should hold the spending authority. The shell is
//! the smallest trusted surface we have.
//!
//! **Why a crate rather than hand-written FFI.** The workspace rule is to prefer a
//! maintained library over a custom build unless it is unsuitable. Writing
//! `CREDENTIALW` marshalling by hand would be more code, more places to get
//! pointer lifetimes wrong, and no better. `keyring` is the maintained wrapper for
//! exactly this, and on Windows it uses the Credential Manager.
//!
//! **What this does and does not protect against**, stated plainly: it moves the
//! secret from "read a file" to "run code as this user and ask the OS". That is a
//! real improvement and it is **not** the same as safe — a process running as the
//! same user can call the same API. The honest claim remains that Capybaras stops
//! *the agent's proposed actions*, not that the machine is trustworthy.

use keyring::Entry;

/// The service name credentials are filed under, so the user can find and delete
/// them in the Windows Credential Manager themselves.
const SERVICE: &str = "Capybaras";

#[derive(Debug)]
pub enum CredentialError {
    /// The OS store exists but refused. Worth surfacing rather than hiding.
    Unavailable { detail: String },
    /// The store worked but the operation did not.
    Failed { detail: String },
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable { detail } => write!(
                f,
                "the operating system credential store is not available ({detail}). \
                 Capybaras will not store a key anywhere else — a key on disk would be \
                 readable by the agent and would make the approval gate optional."
            ),
            Self::Failed { detail } => write!(f, "the credential store refused the operation ({detail})"),
        }
    }
}

impl std::error::Error for CredentialError {}

fn entry(name: &str) -> Result<Entry, CredentialError> {
    Entry::new(SERVICE, name).map_err(|error| CredentialError::Failed {
        detail: error.to_string(),
    })
}

/// Store or replace a credential, identified by `name`.
///
/// There is deliberately **no file fallback.** A convenient file-backed
/// implementation is exactly the `token.json` mistake this module exists to
/// prevent, so the only failure direction available is "not connected".
pub fn save(name: &str, secret: &str) -> Result<(), CredentialError> {
    // REFUSE AN EMPTY SECRET, because the OS store does not.
    //
    // Verified by running it: Windows accepts an empty password and returns it on
    // read, so without this guard `save(name, "")` succeeds and `load` returns
    // `Some("")`. That makes "connected" true while holding nothing -- the worst
    // of both states, because the user believes they are set up and nothing works.
    // The TypeScript `Secret` refuses an empty value for the same reason.
    if secret.is_empty() {
        return Err(CredentialError::Failed {
            detail: "an empty secret is not a credential".to_string(),
        });
    }

    entry(name)?
        .set_password(secret)
        .map_err(|error| CredentialError::Failed {
            detail: error.to_string(),
        })
}

/// Read a credential. `Ok(None)` means nothing is stored — a normal state, not an
/// error, and the state the app should treat as "not connected".
pub fn load(name: &str) -> Result<Option<String>, CredentialError> {
    match entry(name)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CredentialError::Failed {
            detail: error.to_string(),
        }),
    }
}

/// Remove a credential. Removing something absent is not an error, because
/// "disconnect" should be idempotent.
pub fn delete(name: &str) -> Result<(), CredentialError> {
    match entry(name)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CredentialError::Failed {
            detail: error.to_string(),
        }),
    }
}

/// Whether the OS store can be used at all.
///
/// Used at startup so the app can say "connected"/"not connected" honestly rather
/// than offering a Connect button that cannot work.
pub fn available() -> bool {
    // A cheap probe: create an entry and read a name nothing is stored under.
    // `Ok(None)` means the store answered, which is what we are asking.
    matches!(load("capybaras.probe.nonexistent"), Ok(_))
}
