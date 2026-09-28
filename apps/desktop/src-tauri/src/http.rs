//! The HTTP seam for calls that carry the user's provider key.
//!
//! The same shape as `exchange.rs`, for the same reason: the interesting logic in
//! a model call is not "make a request" — it is turning whatever comes back, and
//! whatever goes wrong, into something the application can act on. That logic is
//! pure, so it is tested without a network against a stub. This module is the thin
//! real client underneath it.
//!
//! **THE KEY IS A BEARER AND NOTHING ELSE.** It is passed as a parameter rather
//! than held here, so no transport instance can hand it out; it is never placed in
//! a body, never logged, and never interpolated into an error. `exchange.rs` makes
//! the same promise for the code and the verifier, and for the same reason: an
//! error message is the most likely thing to end up in a screenshot.

use std::time::Duration;

/// How long to wait on OpenRouter before giving up on one call.
///
/// Longer than the token exchange's 30s because generation takes as long as
/// generation takes, and a first reply that silently dies at 30s reads as a broken
/// app rather than a slow model.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// The one seam. Everything above it is pure and testable without a network.
pub trait Transport {
    /// GET `url`, authenticating with `bearer`.
    fn get(&self, url: &str, bearer: &str) -> Result<HttpResponse, String>;

    /// POST `body` as JSON to `url`, authenticating with `bearer`. The error is a
    /// transport description and is **never** surfaced to the user verbatim.
    fn post_json(&self, url: &str, bearer: &str, body: &str) -> Result<HttpResponse, String>;
}

/// The real client.
///
/// Uses the operating system's TLS stack rather than vendoring one — this app is
/// Windows-only already, and the OS stack is the one that gets updated with the
/// system rather than only when we rebuild.
pub struct HttpTransport {
    client: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }
}

impl Transport for HttpTransport {
    fn get(&self, url: &str, bearer: &str) -> Result<HttpResponse, String> {
        let response = self
            .client
            .get(url)
            .bearer_auth(bearer)
            .send()
            .map_err(|error| error.to_string())?;
        read(response)
    }

    fn post_json(&self, url: &str, bearer: &str, body: &str) -> Result<HttpResponse, String> {
        let response = self
            .client
            .post(url)
            .bearer_auth(bearer)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(|error| error.to_string())?;
        read(response)
    }
}

fn read(response: reqwest::blocking::Response) -> Result<HttpResponse, String> {
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| error.to_string())?;
    Ok(HttpResponse { status, body })
}
