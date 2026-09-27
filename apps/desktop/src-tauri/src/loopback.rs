//! The loopback listener the browser redirects to.
//!
//! This is the one part of the sign-in flow that cannot be tested without a
//! socket, which is why it is separated from `oauth.rs` rather than mixed in.
//!
//! **WHY A LIBRARY AND NOT A HAND-WRITTEN SERVER.** The workspace rule is to prefer
//! a maintained library over a custom build unless it is unsuitable. Parsing HTTP
//! by hand means writing request-line handling, header folding, chunked bodies and
//! connection teardown — none of it interesting, all of it a place to be subtly
//! wrong, and a browser sends more than the GET we expect. `tiny_http` is the
//! maintained blocking implementation for exactly this, and adds no async runtime
//! to a shell that has none.
//!
//! **FOUR DECISIONS THAT ARE DELIBERATE:**
//!
//! 1. **Bind `127.0.0.1`, never `0.0.0.0`.** Binding every interface would put an
//!    OAuth callback on the local network, where another machine could race the
//!    browser to it and take the authorization code for itself.
//! 2. **An ephemeral port.** A fixed port is a port conflict on a stranger's
//!    machine, and a support burden is worse than a cosmetically stable title.
//! 3. **Keep serving after an unexpected request.** A browser asks for
//!    `/favicon.ico` on its own, without being told to. Exiting after the first
//!    request would abandon a sign-in that was about to succeed.
//! 4. **The request URL is never logged and never echoed.** It contains a live
//!    authorization code. It reaches `parse_callback` and nothing else.

use std::net::{SocketAddr, TcpListener};
use std::time::{Duration, Instant};

use tiny_http::{Header, Response, Server, StatusCode};

use crate::oauth::{self, CALLBACK_PATH, CallbackFailure, CallbackResult, StatePolicy};

/// The default patience, matching the provider's own limit: authorization codes
/// expire 10 minutes after issue, so waiting longer than that can only produce a
/// code that will be refused. Stopping and saying so is better than hanging.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug)]
pub enum LoopbackError {
    /// The socket itself failed — could not bind, or the listener errored.
    /// Distinct from a refused sign-in, which is a normal outcome.
    Socket(String),
}

impl std::fmt::Display for LoopbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Socket(detail) => write!(
                f,
                "could not listen for the sign-in to come back ({detail}). \
                 Capybaras will not fall back to another port or another interface."
            ),
        }
    }
}

impl std::error::Error for LoopbackError {}

/// What came back from the browser.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Something we can exchange. The only outcome that continues the flow.
    Code(String),
    /// The browser came back and we refused it. The reason shares the flow's own
    /// vocabulary so the caller does not invent a second one.
    Refused(CallbackFailure),
    /// Nobody came back in time.
    TimedOut,
}

pub struct Loopback {
    server: Server,
    addr: SocketAddr,
}

impl Loopback {
    /// Bind an ephemeral port on loopback.
    pub fn bind() -> Result<Self, LoopbackError> {
        // Bind with std first so the chosen port is known explicitly, rather than
        // depending on however the HTTP library reports it.
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| LoopbackError::Socket(e.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|e| LoopbackError::Socket(e.to_string()))?;

        let server = Server::from_listener(listener, None)
            .map_err(|e| LoopbackError::Socket(e.to_string()))?;

        Ok(Self { server, addr })
    }

    /// The address actually bound. Exposed so a test can assert it is loopback.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// The address to hand the provider. Uses `localhost` because that is what the
    /// provider documents and titles the app with; the listener itself is bound to
    /// `127.0.0.1` regardless.
    pub fn callback_url(&self) -> String {
        oauth::callback_url(self.port())
    }

    /// Wait for the browser, serving anything else it asks for along the way.
    ///
    /// `state` is a POLICY, not a required value, and that distinction is load
    /// bearing: OpenRouter sends no `state` at all, so demanding one makes every real
    /// sign-in fail. The original signature took `Option<&str>` where `Some` meant
    /// "require" -- it looked harmless and it broke the first live attempt.
    pub fn wait_for_code(
        self,
        state: StatePolicy<'_>,
        timeout: Duration,
    ) -> Result<Outcome, LoopbackError> {
        let deadline = Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(Outcome::TimedOut);
            }

            let request = match self.server.recv_timeout(remaining) {
                Ok(Some(request)) => request,
                Ok(None) => return Ok(Outcome::TimedOut),
                Err(error) => return Err(LoopbackError::Socket(error.to_string())),
            };

            // `url()` is the request target: path plus query. Split before building
            // anything, so a non-callback path never reaches the parser at all.
            let target = request.url().to_string();
            let path = target.split('?').next().unwrap_or_default();

            if path != CALLBACK_PATH {
                // NOT AN ERROR. The browser asks for /favicon.ico unprompted, and a
                // probe or a prefetch looks the same. Answer politely and keep
                // waiting -- abandoning a sign-in that was about to succeed would be
                // a bug with a confusing symptom.
                respond(request, 404, not_found_page());
                continue;
            }

            // The code lives in this string and nowhere else. It is not logged, not
            // stored, and not echoed into the response.
            let full_url = format!("http://localhost:{}{}", self.addr.port(), target);
            match oauth::parse_callback_with(&full_url, state) {
                CallbackResult::Ok { code } => {
                    respond(request, 200, oauth::callback_page(true));
                    return Ok(Outcome::Code(code));
                }
                CallbackResult::Failed { reason, .. } => {
                    // 200 rather than 4xx: a person is looking at this page, and a
                    // browser error page would explain nothing. The page itself
                    // says what happened.
                    respond(request, 200, oauth::callback_page(false));
                    return Ok(Outcome::Refused(reason));
                }
            }
        }
    }
}

fn respond(request: tiny_http::Request, status: u16, body: String) {
    let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
        .expect("a static header is valid");
    let response = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(header);
    let _ = request.respond(response);
}

fn not_found_page() -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>Capybaras</title></head>\
<body style=\"font-family:system-ui,sans-serif;background:#faf7f2;color:#2b2118\">\
<p>Capybaras is waiting for a sign-in to finish.</p></body></html>"
    )
}
