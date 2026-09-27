//! The OpenRouter sign-in flow, minus the socket.
//!
//! **WHY THIS IS IN RUST RATHER THAN REUSING `src/onboarding/{pkce,flow}.ts`.**
//! The TypeScript already implements this and is tested. It cannot be used for
//! the real flow, and the reason is where the secrets have to live:
//!
//!   * The verifier must not leave the component that performs the exchange.
//!   * The authorization code must not leave the component that stores the key.
//!   * `docs/plans/M5-onboarding.md` is explicit: the key, the code and the
//!     verifier are **never** written to the protocol stream.
//!
//! The shell is that component — it is the one that owns the Credential Manager
//! and the one whose process the agent does not run inside. So the flow is
//! implemented here. The TypeScript remains the reference, and **both pin the
//! same RFC 7636 appendix B vector**, so the two cannot silently disagree about
//! the hash. A port that drifts would fail the test rather than fail in the
//! field, where the symptom is an unhelpful server error.
//!
//! Everything here is pure or takes its input as an argument, so all of it is
//! testable without a browser, a network, or an account. The socket deliberately
//! lives in `loopback.rs`, because it is the part that cannot be tested this way.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use url::Url;

pub const AUTH_ENDPOINT: &str = "https://openrouter.ai/auth";
pub const CALLBACK_PATH: &str = "/callback";

/// The label shown on the key in the user's **own** OpenRouter dashboard, so it
/// is identifiable rather than anonymous when they go to revoke it. Revocation
/// being theirs is a property worth protecting deliberately.
pub const KEY_LABEL: &str = "Capybaras";

/// 256 bits, as RFC 7636 requires of the verifier. 32 bytes encodes to 43
/// base64url characters, which is also the RFC's minimum length — so a verifier
/// generated here is always structurally legal.
const VERIFIER_BYTES: usize = 32;
const STATE_BYTES: usize = 32;

const MIN_VERIFIER: usize = 43;
const MAX_VERIFIER: usize = 128;

/// A fresh verifier. 256 bits of OS entropy, base64url, no padding.
pub fn create_verifier() -> String {
    let mut bytes = [0u8; VERIFIER_BYTES];
    getrandom::fill(&mut bytes).expect("the OS random number generator failed");
    URL_SAFE_NO_PAD.encode(bytes)
}

/// A fresh `state`, used to reject a callback this app did not start.
pub fn create_state() -> String {
    let mut bytes = [0u8; STATE_BYTES];
    getrandom::fill(&mut bytes).expect("the OS random number generator failed");
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The S256 challenge for a verifier.
///
/// There is deliberately **no `plain` variant**. RFC 7636 permits
/// `code_challenge_method=plain`, which provides essentially no protection
/// against the attack PKCE exists to stop, and the only reason to support it is
/// an old server. We do not have one, so offering it would only be a way to
/// weaken the flow by configuration.
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Constant-time comparison of the returned `state` against the one we sent.
///
/// Timing-safe because the alternative leaks the expected value one byte at a
/// time. It is three lines, and the habit of writing the safe version by default
/// is worth more than the individual attack it prevents.
///
/// `black_box` stops the optimiser collapsing this into an early-exit compare.
/// That is not a formal guarantee — it is the same class of care the TypeScript
/// takes by using `timingSafeEqual`, and the same honesty applies: if this ever
/// guarded something an attacker could time across a network, it should be
/// reviewed rather than trusted.
pub fn states_match(expected: &str, received: &str) -> bool {
    if expected.is_empty() || received.is_empty() {
        return false;
    }
    let left = expected.as_bytes();
    let right = received.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    std::hint::black_box(difference) == 0
}

/// Whether a verifier is structurally acceptable. Unreserved characters only,
/// per RFC 7636 section 4.1.
pub fn is_valid_verifier(verifier: &str) -> bool {
    let length = verifier.len();
    if length < MIN_VERIFIER || length > MAX_VERIFIER {
        return false;
    }
    verifier
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

/// Everything one sign-in attempt needs.
pub struct Attempt {
    /// Stays on this machine. Never sent until the token exchange.
    pub verifier: String,
    /// Sent in the authorization request.
    pub challenge: String,
    /// Sent with the request and checked on the way back.
    pub state: String,
}

/// **`Debug` IS WRITTEN BY HAND, AND THAT IS THE POINT.**
///
/// The derived version would print the verifier, and the single most likely way
/// this value leaks is a log line or a panic message that formats the struct
/// containing it. Redacting here means the accidental path is closed at the
/// source, the same way `Secret` closes it on the TypeScript side. A leak now
/// requires reaching for `.verifier` explicitly, which is one greppable word.
impl std::fmt::Debug for Attempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attempt")
            .field("verifier", &"[redacted]")
            .field("challenge", &self.challenge)
            .field("state", &self.state)
            .finish()
    }
}

/// Everything one sign-in attempt needs.
pub fn create_attempt() -> Attempt {
    let verifier = create_verifier();
    Attempt {
        challenge: challenge_for(&verifier),
        verifier,
        state: create_state(),
    }
}

/// The loopback address the browser is told to come back to.
///
/// `localhost` in the URL because that is what the provider documents and titles
/// the app with. **The listener must nevertheless bind `127.0.0.1` specifically,
/// never `0.0.0.0`** — binding every interface would put an OAuth callback on the
/// local network, where another machine could race the browser to it.
pub fn callback_url(port: u16) -> String {
    format!("http://localhost:{port}{CALLBACK_PATH}")
}

/// Build the authorization URL.
pub fn authorization_url(callback_url: &str, challenge: &str, key_label: &str) -> String {
    let mut url = Url::parse(AUTH_ENDPOINT).expect("the auth endpoint is a constant and parses");
    url.query_pairs_mut()
        .append_pair("callback_url", callback_url)
        .append_pair("code_challenge", challenge)
        // Never `plain`. See the note on `challenge_for`.
        .append_pair("code_challenge_method", "S256")
        .append_pair("key_label", key_label);
    url.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackFailure {
    /// The user said no. Not an error in our code, and should read that way.
    Denied,
    /// The provider came back without a code.
    NoCode,
    /// A `state` we did not send, when we sent one.
    StateMismatch,
    /// Not parseable as a URL, or an unrecognised provider error.
    Malformed,
}

impl CallbackFailure {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::NoCode => "no-code",
            Self::StateMismatch => "state-mismatch",
            Self::Malformed => "malformed",
        }
    }
}

pub enum CallbackResult {
    Ok { code: String },
    Failed {
        reason: CallbackFailure,
        detail: Option<String>,
    },
}

/// When to insist on a `state` value coming back.
///
/// **THIS ENUM EXISTS BECAUSE A BOOLEAN-LOOKING `Option` BROKE THE FIRST REAL
/// SIGN-IN.** The original code took `Option<&str>` where `Some` meant "require a
/// match". The caller passed `Some(&attempt.state)` -- which reads as harmless and
/// is exactly backwards, because **OpenRouter sends no `state` at all**. Every real
/// callback was therefore refused as a mismatch, and the only visible symptom was
/// the generic "that did not work" page.
///
/// The tests did not catch it because every one of them supplied a state. The one
/// case that actually happens in the field was the one case not covered. Naming the
/// three behaviours makes the mistake unavailable: there is no way to write "require
/// it" by accident when the word has to appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatePolicy<'a> {
    /// The provider does not send one. Nothing to check.
    NotExpected,
    /// Check it when the provider happens to send one, but never demand it.
    /// **This is the correct policy for OpenRouter.**
    IfPresent(&'a str),
    /// Insist on a match. Only for a provider documented to echo one.
    Required(&'a str),
}

/// Read the browser's redirect.
///
/// **ON `state`:** OpenRouter's documentation does not list a `state` parameter,
/// so we cannot require one. That is survivable, and the reasoning is worth writing
/// down rather than leaving as a shrug -- because "the provider did not give us a
/// defence, so we skipped it" is how a control quietly disappears.
///
/// `state` exists to stop login-CSRF: an attacker handing your app *their*
/// authorization code so you end up signed in as them. **PKCE already defeats
/// that**, because exchanging the code requires the verifier, which never leaves
/// this machine and which the attacker does not have. So `state` is redundant here
/// rather than missing.
///
/// `Some` here means **Required**, which is why the caller should prefer
/// `parse_callback_with` and say which policy it wants out loud.
pub fn parse_callback(request_url: &str, expected_state: Option<&str>) -> CallbackResult {
    parse_callback_with(
        request_url,
        match expected_state {
            Some(expected) => StatePolicy::Required(expected),
            None => StatePolicy::NotExpected,
        },
    )
}

/// Read the browser's redirect under an explicit state policy.
pub fn parse_callback_with(request_url: &str, policy: StatePolicy<'_>) -> CallbackResult {
    let url = match Url::parse(request_url) {
        Ok(url) => url,
        Err(_) => {
            return CallbackResult::Failed {
                reason: CallbackFailure::Malformed,
                detail: Some("the callback was not a valid URL".to_string()),
            };
        }
    };

    // Collect first, then decide, so the checks run in a fixed order rather than
    // in whatever order the query string happens to arrive.
    let mut provider_error: Option<String> = None;
    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "error" => provider_error = Some(value.into_owned()),
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(error) = provider_error {
        // The provider's error code is not a secret, so it can be carried.
        return CallbackResult::Failed {
            reason: if error == "access_denied" {
                CallbackFailure::Denied
            } else {
                CallbackFailure::Malformed
            },
            detail: Some(error),
        };
    }

    let mismatched = || CallbackResult::Failed {
        reason: CallbackFailure::StateMismatch,
        detail: None,
    };
    match policy {
        StatePolicy::NotExpected => {}
        // Check it because it arrived, not because we demanded it. Requiring it is
        // the bug that broke the first live sign-in.
        StatePolicy::IfPresent(expected) => {
            if let Some(received) = state.as_deref() {
                if !states_match(expected, received) {
                    return mismatched();
                }
            }
        }
        StatePolicy::Required(expected) => {
            if !states_match(expected, state.as_deref().unwrap_or_default()) {
                return mismatched();
            }
        }
    }

    match code {
        Some(code) if !code.is_empty() => CallbackResult::Ok { code },
        _ => CallbackResult::Failed {
            reason: CallbackFailure::NoCode,
            detail: None,
        },
    }
}

/// Refuse to echo anything secret back into a page.
///
/// The browser is looking at whatever the listener returns, and the URL it
/// requested contains the authorization code. Writing the request URL into the
/// response — the obvious way to write a "debug" page — would put a live code in
/// the DOM, in the browser's history, and in any screenshot the user takes.
pub fn callback_page(success: bool) -> String {
    let (title, body) = if success {
        (
            "You are connected",
            "You can close this tab and go back to Capybaras.",
        )
    } else {
        (
            "That did not work",
            "Return to Capybaras and try again. No changes were made.",
        )
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>Capybaras</title>\
<style>body{{font-family:system-ui,sans-serif;background:#faf7f2;color:#2b2118;\
display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
main{{text-align:center;max-width:28rem;padding:2rem}}\
h1{{font-size:1.25rem;font-weight:600;margin:0 0 .5rem}}\
p{{margin:0;color:#6b5c4d;line-height:1.5}}</style></head>\
<body><main><h1>{title}</h1><p>{body}</p></main></body></html>"
    )
}
