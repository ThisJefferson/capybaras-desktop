//! Trading the authorization code for an API key.
//!
//! The last step of the flow, and the only one that talks to the network.
//!
//! **THE HTTP CALL IS A SEAM, DELIBERATELY.** The interesting logic here is not
//! "make a POST" — it is turning the provider's responses into something the
//! application can act on: an expired code needs a different message from a wrong
//! verifier, and both need to read as recoverable rather than as crashes. That
//! logic is pure, so it is tested exhaustively against every documented failure
//! with no network at all. The real client is a thin implementation of the same
//! trait, and it is exercised against a local server in the tests.
//!
//! **NOTHING SECRET IS EVER PUT IN A `detail` STRING.** Not the code, not the
//! verifier, not the key. An error message is the single most likely thing to end
//! up in a log file or a screenshot, so it is the last place a credential should
//! appear — and it is an easy thing to get wrong while writing a helpful message.
//! The `Network` branch in particular does **not** interpolate the underlying
//! error, because a transport error can carry the request, and the request carries
//! the code and the verifier.

use std::time::Duration;

/// Where the code is exchanged. Public so the tests can point the flow at a local
/// server instead of the real endpoint.
pub const EXCHANGE_ENDPOINT: &str = "https://openrouter.ai/api/v1/auth/keys";

/// How long to wait on OpenRouter before giving up.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// The one seam. Everything above it is pure and testable without a network.
pub trait Poster {
    /// POST `body` as JSON to `url`. The error is a transport description, and it
    /// is **never** surfaced to the user verbatim — see the note above.
    fn post_json(&self, url: &str, body: &str) -> Result<HttpResponse, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeFailure {
    /// The code was issued too long ago. The fix is to start again.
    Expired,
    /// The code and the verifier do not belong together. Also start again.
    BadVerifier,
    /// The two steps disagreed about the challenge method.
    BadMethod,
    /// Anything else the provider refused for a reason we cannot classify.
    BadRequest,
    /// Could not reach the provider at all.
    Network,
    /// A response we could not read.
    Malformed,
}

impl ExchangeFailure {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::BadVerifier => "bad-verifier",
            Self::BadMethod => "bad-method",
            Self::BadRequest => "bad-request",
            Self::Network => "network",
            Self::Malformed => "malformed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeResult {
    /// The only outcome that produces a credential.
    Key(String),
    Failed {
        reason: ExchangeFailure,
        detail: String,
    },
}

/// Build the request body. Contains the code and the verifier — never log it.
fn request_body(code: &str, verifier: &str) -> String {
    serde_json::json!({
        "code": code,
        "code_verifier": verifier,
        // Never `plain`. See the note in oauth.rs: it provides essentially no
        // protection, and the only reason to offer it is an old server.
        "code_challenge_method": "S256",
    })
    .to_string()
}

/// Exchange a code plus its verifier for a key.
///
/// Note this is *not* a refresh flow. The provider returns a long-lived API key
/// rather than an expiring token, so there is nothing to refresh — but there is a
/// revocation case, because the user can delete the key from their own account at
/// any time. That is handled where the key is used, not here.
pub fn exchange(
    endpoint: &str,
    code: &str,
    verifier: &str,
    poster: &dyn Poster,
) -> ExchangeResult {
    if code.is_empty() {
        return ExchangeResult::Failed {
            reason: ExchangeFailure::Malformed,
            detail: "the sign-in came back without a code".to_string(),
        };
    }
    if !crate::oauth::is_valid_verifier(verifier) {
        // Refuse before sending. A malformed verifier can only produce a confusing
        // 403, and the local check is free.
        return ExchangeResult::Failed {
            reason: ExchangeFailure::BadVerifier,
            detail: "this sign-in attempt is malformed; start again".to_string(),
        };
    }

    let response = match poster.post_json(endpoint, &request_body(code, verifier)) {
        Ok(response) => response,
        Err(_) => {
            // The error is intentionally dropped rather than interpolated.
            return ExchangeResult::Failed {
                reason: ExchangeFailure::Network,
                detail:
                    "could not reach OpenRouter; check your connection and try again"
                        .to_string(),
            };
        }
    };

    if !(200..300).contains(&response.status) {
        return classify_refusal(&response);
    }

    parse_key(&response.body)
}

/// Turn a non-2xx response into something a person can act on.
fn classify_refusal(response: &HttpResponse) -> ExchangeResult {
    let failed = |reason, detail: &str| ExchangeResult::Failed {
        reason,
        detail: detail.to_string(),
    };

    match response.status {
        400 => failed(
            ExchangeFailure::BadMethod,
            "OpenRouter rejected the request: the sign-in was started with a different \
             challenge method than it was finished with",
        ),
        403 => {
            // 403 covers both an expired code and a wrong verifier. They need
            // different actions, so try to tell them apart from the body. This is
            // the only place the body is inspected, and only for a keyword.
            if response.body.to_lowercase().contains("expir") {
                failed(
                    ExchangeFailure::Expired,
                    "the sign-in took too long and the code has expired; start again",
                )
            } else {
                failed(
                    ExchangeFailure::BadVerifier,
                    "OpenRouter did not accept this sign-in attempt; start again",
                )
            }
        }
        405 => failed(
            ExchangeFailure::BadRequest,
            "OpenRouter rejected the request method",
        ),
        status => ExchangeResult::Failed {
            reason: ExchangeFailure::BadRequest,
            // The status code is not a secret, so it can be carried.
            detail: format!("OpenRouter returned an unexpected status ({status})"),
        },
    }
}

/// Pull the key out of a successful response.
fn parse_key(body: &str) -> ExchangeResult {
    let parsed: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return ExchangeResult::Failed {
                reason: ExchangeFailure::Malformed,
                detail: "OpenRouter returned a response that could not be read".to_string(),
            };
        }
    };

    match parsed.get("key").and_then(|key| key.as_str()) {
        // Trimmed, because a key with trailing whitespace fails every later request
        // with a 401 that points nowhere near the cause. A key cannot legitimately
        // contain leading or trailing whitespace, so this cannot lose information.
        Some(key) if !key.trim().is_empty() => ExchangeResult::Key(key.trim().to_string()),
        _ => ExchangeResult::Failed {
            reason: ExchangeFailure::Malformed,
            detail: "OpenRouter returned no key".to_string(),
        },
    }
}

/// The real client.
///
/// Uses the operating system's TLS stack rather than vendoring one: this app is
/// Windows-only already, and the OS stack is the one that gets updated with the
/// system rather than only when we rebuild.
pub struct HttpPoster {
    client: reqwest::blocking::Client,
}

impl HttpPoster {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }
}

impl Poster for HttpPoster {
    fn post_json(&self, url: &str, body: &str) -> Result<HttpResponse, String> {
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(|error| error.to_string())?;

        let status = response.status().as_u16();
        let body = response.text().map_err(|error| error.to_string())?;
        Ok(HttpResponse { status, body })
    }
}
