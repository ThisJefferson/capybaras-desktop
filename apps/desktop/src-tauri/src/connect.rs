//! The Connect command: the shell side of the sign-in.
//!
//! This is the piece that ties the flow together. It exists in the shell, not the
//! sidecar, for one reason: **the verifier and the key must not leave the component
//! that owns the credential store.** See the note at the top of `oauth.rs`.
//!
//! The whole sign-in takes as long as a person takes to log in, so it cannot block
//! the interface. `start_connect` therefore returns as soon as the listener is up
//! and the browser is open, and the outcome arrives as **an event** — the same
//! shape the approval flow already uses, for the same reason.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::credential;
use crate::exchange::{self, EXCHANGE_ENDPOINT, ExchangeResult, HttpPoster};
use crate::loopback::{self, Loopback, Outcome};
use crate::oauth::{self, KEY_LABEL};

/// The name the key is filed under in the OS credential store. Public so the user
/// can find and delete it themselves — revocation being theirs is a property worth
/// protecting, and an unexplained entry helps nobody.
pub const CREDENTIAL_NAME: &str = "capybaras.oauth.openrouter";

pub const EVENT_WAITING: &str = "capybaras://connect-waiting";
pub const EVENT_CONNECTED: &str = "capybaras://connect-connected";
pub const EVENT_FAILED: &str = "capybaras://connect-failed";

/// One sign-in at a time.
///
/// Two concurrent attempts would mean two listeners and two verifiers, and the
/// user would have no way to tell which browser tab belonged to which. Refusing the
/// second is clearer than racing.
#[derive(Default)]
pub struct ConnectState {
    in_flight: Mutex<bool>,
}

#[derive(Serialize)]
pub struct ConnectStatus {
    connected: bool,
    credential_name: String,
    store_available: bool,
    store_detail: Option<String>,
}

/// Is a key already stored, and can the store be used at all?
///
/// **FAILS CLOSED.** Anything other than a readable credential reports "not
/// connected". A store that cannot be read is not a reason to behave as if we have
/// one.
#[tauri::command]
pub fn connect_status() -> ConnectStatus {
    let (connected, store_available, store_detail) = match credential::load(CREDENTIAL_NAME) {
        Ok(Some(_)) => (true, true, None),
        Ok(None) => (false, true, None),
        // The key is never loaded into this function's output, only its presence.
        Err(error) => (false, false, Some(error.to_string())),
    };

    ConnectStatus {
        connected,
        credential_name: CREDENTIAL_NAME.to_string(),
        store_available,
        store_detail,
    }
}

#[tauri::command]
pub fn start_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConnectState>,
) -> Result<u16, String> {
    {
        let mut busy = state
            .in_flight
            .lock()
            .map_err(|_| "the sign-in state is unusable".to_string())?;
        if *busy {
            return Err("a sign-in is already in progress".to_string());
        }
        *busy = true;
    }

    let listener = match Loopback::bind() {
        Ok(listener) => listener,
        Err(error) => {
            release(&state);
            return Err(error.to_string());
        }
    };
    let port = listener.port();
    let callback = listener.callback_url();

    // The verifier is created here and never leaves this process. It is not sent
    // over the protocol stream, not written to a file, and not logged.
    let attempt = oauth::create_attempt();
    let authorization = oauth::authorization_url(&callback, &attempt.challenge, KEY_LABEL);

    if let Err(detail) = open_in_browser(&authorization) {
        release(&state);
        return Err(detail);
    }

    let _ = app.emit(
        EVENT_WAITING,
        serde_json::json!({ "port": port, "callbackUrl": callback }),
    );

    let handle = app.clone();
    std::thread::spawn(move || {
        let outcome = listener.wait_for_code(Some(&attempt.state), loopback::DEFAULT_TIMEOUT);
        finish(&handle, attempt, outcome);
    });

    Ok(port)
}

/// Clear the one-at-a-time flag. Separate so every early return cannot forget it.
fn release(state: &tauri::State<'_, ConnectState>) {
    if let Ok(mut busy) = state.in_flight.lock() {
        *busy = false;
    }
}

/// The tail of the flow, off the main thread.
fn finish(
    app: &tauri::AppHandle,
    attempt: oauth::Attempt,
    outcome: Result<Outcome, loopback::LoopbackError>,
) {
    let failure = |reason: &str, detail: String| {
        let _ = app.emit(
            EVENT_FAILED,
            serde_json::json!({ "reason": reason, "detail": detail }),
        );
    };

    match outcome {
        Ok(Outcome::Code(code)) => match store_key(&attempt.verifier, &code) {
            Ok(()) => {
                // Deliberately carries no key, no prefix, no length.
                let _ = app.emit(
                    EVENT_CONNECTED,
                    serde_json::json!({ "credentialName": CREDENTIAL_NAME }),
                );
            }
            Err((reason, detail)) => failure(&reason, detail),
        },
        Ok(Outcome::Refused(reason)) => failure(reason.as_str(), refusal_detail(reason).to_string()),
        Ok(Outcome::TimedOut) => failure(
            "timed-out",
            "the sign-in was not finished in time; start again".to_string(),
        ),
        Err(error) => failure("listener", error.to_string()),
    }

    if let Some(state) = app.try_state::<ConnectState>() {
        if let Ok(mut busy) = state.in_flight.lock() {
            *busy = false;
        }
    }
}

/// Exchange the code and put the key in the OS store.
///
/// Returns `(reason, detail)` on failure, where `reason` stays in the vocabulary
/// the rest of the flow uses.
///
/// **THE KEY'S IN-MEMORY COPY IS NOT SCRUBBED, AND THAT IS A KNOWN LIMIT.** The
/// key exists as a `String` in this process for as long as it takes to store it;
/// `drop` releases the allocation without overwriting it. Rust does not zeroize by
/// default, and adding that properly means a zeroizing wrapper on every path the
/// value travels. Written down rather than left implied: this is the same-user
/// boundary the threat model already describes, and it is worth closing before 1.0.
fn store_key(verifier: &str, code: &str) -> Result<(), (String, String)> {
    let poster = HttpPoster::new().map_err(|detail| ("network".to_string(), detail))?;

    match exchange::exchange(EXCHANGE_ENDPOINT, code, verifier, &poster) {
        ExchangeResult::Key(key) => {
            let stored = credential::save(CREDENTIAL_NAME, &key)
                .map_err(|error| ("store".to_string(), error.to_string()));
            drop(key);
            stored
        }
        ExchangeResult::Failed { reason, detail } => Err((reason.as_str().to_string(), detail)),
    }
}

fn refusal_detail(reason: oauth::CallbackFailure) -> &'static str {
    match reason {
        oauth::CallbackFailure::Denied => {
            "you chose not to authorize Capybaras; nothing was changed"
        }
        oauth::CallbackFailure::NoCode => "OpenRouter came back without a code; try again",
        oauth::CallbackFailure::StateMismatch => {
            "that sign-in did not match the one Capybaras started; try again"
        }
        oauth::CallbackFailure::Malformed => {
            "OpenRouter came back with something Capybaras could not read; try again"
        }
    }
}

/// Open the authorization URL in the user's default browser.
///
/// **THE SYSTEM BROWSER, NOT A WEBVIEW, AND THAT IS A SECURITY CHOICE.** A sign-in
/// page rendered inside our own webview means the user types their credentials into
/// a window our code controls, and the app could read them even if it does not. The
/// system browser keeps the provider's page in the provider's trust domain. It also
/// keeps the provider's JavaScript out of our webview, which is what lets the CSP
/// stay `script-src 'self'`.
fn open_in_browser(url: &str) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let verb = wide("open");
    let target = wide(url);

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecute reports failure as a value of 32 or less. Carrying the code is
    // more useful than "could not open your browser" with no way to find out why.
    let code = result.0 as isize;
    if code <= 32 {
        return Err(format!(
            "could not open your browser to finish signing in (code {code})"
        ));
    }
    Ok(())
}
