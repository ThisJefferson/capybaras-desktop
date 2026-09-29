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

/// How long to allow a **streamed** generation.
///
/// **LONGER THAN `REQUEST_TIMEOUT`, AND IT HAS TO BE.** The 60s figure was chosen
/// for a call whose reply arrives in one piece. A streamed reply is the opposite
/// case: the answer is being written while it is read, so a long answer is a long
/// connection by design. Cutting it at 60s would truncate exactly the long replies
/// the operator asked to see in full.
pub const STREAM_TIMEOUT: Duration = Duration::from_secs(600);

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

    /// POST `body` as JSON and deliver the answer **as it arrives**, one line at a
    /// time, through `on_line`. Returns the whole body once the stream ends.
    ///
    /// **`on_line` RETURNS WHETHER TO CONTINUE, AND THAT IS THE STOP CONTROL.** A
    /// callback that answers `false` ends the read, which drops the response and closes
    /// the connection — so halting a generation is a decision the caller can make
    /// *mid-stream* rather than a button that only stops the screen updating. A Stop
    /// that lets the model keep writing is not a Stop.
    ///
    /// **THE DEFAULT IS DELIBERATELY NAIVE, AND THAT IS THE POINT.** A transport that
    /// cannot stream still works: the buffered body is handed over line by line, so
    /// the caller needs no second code path and the parse is exercised in tests. Only
    /// the real client overrides this, and it does so for one reason — so the lines
    /// arrive *while the model is still writing* rather than all at the end.
    ///
    /// Carries the same promise as `post_json`: the key is a bearer, never a body,
    /// never logged, never interpolated into an error.
    fn post_json_stream(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<HttpResponse, String> {
        let response = self.post_json(url, bearer, body)?;
        for line in response.body.lines() {
            if !on_line(line) {
                break;
            }
        }
        Ok(response)
    }
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

    fn post_json_stream(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<HttpResponse, String> {
        self.stream(url, bearer, body, on_line)
    }
}

fn read(response: reqwest::blocking::Response) -> Result<HttpResponse, String> {
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| error.to_string())?;
    Ok(HttpResponse { status, body })
}

impl HttpTransport {
    /// The streaming path, where lines are handed over as the socket delivers them.
    ///
    /// **A REQUEST-LEVEL TIMEOUT, NOT THE CLIENT'S.** `REQUEST_TIMEOUT` is sized for a
    /// reply that arrives whole; a streamed generation is a long connection on
    /// purpose, so this one raises it rather than inheriting the shorter bound.
    fn stream(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<HttpResponse, String> {
        use std::io::BufRead;

        let response = self
            .client
            .post(url)
            .bearer_auth(bearer)
            .header("Content-Type", "application/json")
            .timeout(STREAM_TIMEOUT)
            .body(body.to_string())
            .send()
            .map_err(|error| error.to_string())?;

        let status = response.status().as_u16();
        let mut collected = String::new();
        for line in std::io::BufReader::new(response).lines() {
            let line = line.map_err(|error| error.to_string())?;
            // The caller's answer decides whether the read continues. Returning here
            // drops the response, which closes the connection: that is how a Stop
            // actually stops a generation rather than merely disguising it.
            if !on_line(&line) {
                break;
            }
            collected.push_str(&line);
            collected.push('\n');
        }
        Ok(HttpResponse { status, body: collected })
    }
}
