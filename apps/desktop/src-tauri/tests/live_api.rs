//! Live probes against OpenRouter, using the credential the app already stored.
//!
//! **EVERY TEST HERE IS `#[ignore]`d, AND THAT IS THE POINT.** `cargo test` must
//! never need a network or a credential: CI has neither, and a suite that quietly
//! depends on the internet fails for reasons that have nothing to do with the code.
//!
//! Run them on purpose:
//!
//! ```text
//! cargo test --test live_api -- --ignored --nocapture
//! ```
//!
//! These print only non-secret fields. The key is read from the OS credential store
//! and is never echoed, logged, or placed in an assertion message — the same rule
//! the flow itself follows, and it matters just as much in a probe, because probe
//! output is the kind of thing that ends up pasted into a chat window.

use std::time::{Duration, Instant};

use capybaras_shell::credential;

const KEY_CREDENTIAL: &str = "capybaras.oauth.openrouter";
const BASE: &str = "https://openrouter.ai/api/v1";

/// Read the stored key, or explain why there is none. Never prints it.
fn api_key() -> Option<String> {
    match credential::load(KEY_CREDENTIAL) {
        Ok(Some(key)) => Some(key),
        Ok(None) => {
            eprintln!("NOT CONNECTED: nothing stored under {KEY_CREDENTIAL}. Use Connect first.");
            None
        }
        Err(error) => {
            eprintln!("the credential store is unusable here: {error}");
            None
        }
    }
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .expect("build an HTTP client")
}

fn get(path: &str, key: &str) -> (u16, String, Duration) {
    let started = Instant::now();
    let response = client()
        .get(format!("{BASE}{path}"))
        .bearer_auth(key)
        .send()
        .expect("the request should reach OpenRouter");
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    (status, body, started.elapsed())
}

fn ms(duration: Duration) -> String {
    format!("{} ms", duration.as_millis())
}

/// What does the key know about itself? This is where a spending limit shows up.
#[test]
#[ignore = "needs a live credential and network access"]
fn probe_key_info() {
    let Some(key) = api_key() else { return };
    let (status, body, took) = get("/key", &key);
    println!("GET /key -> HTTP {status} in {}", ms(took));

    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let data = &value["data"];
    if data.is_null() {
        println!("  no `data` in the response; body was: {body}");
        return;
    }
    for field in [
        "label",
        "usage",
        "limit",
        "limit_remaining",
        "is_free_tier",
        "rate_limit",
    ] {
        println!("  {field:>16}: {}", data[field]);
    }
}

/// Total credits and total usage. This is the "how much is left" question, if the
/// endpoint will answer it for an ordinary inference key.
#[test]
#[ignore = "needs a live credential and network access"]
fn probe_credits() {
    let Some(key) = api_key() else { return };
    let (status, body, took) = get("/credits", &key);
    println!("GET /credits -> HTTP {status} in {}", ms(took));
    println!("  body: {}", body.trim());
    if status == 403 {
        println!("  NOTE: 403 is the documented answer when this endpoint wants a");
        println!("        provisioning key rather than an inference key.");
    }
}

/// The model list, and how long it takes. Also the source of per-model pricing,
/// which is what a spend figure needs.
#[test]
#[ignore = "needs a live credential and network access"]
fn probe_models() {
    let Some(key) = api_key() else { return };
    let (status, body, took) = get("/models", &key);
    println!("GET /models -> HTTP {status} in {}", ms(took));

    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let models = value["data"].as_array().cloned().unwrap_or_default();
    println!("  models listed: {}", models.len());

    if let Some(first) = models.first() {
        // Show the shape, not the whole catalogue -- the point is to learn which
        // fields exist for pricing.
        println!("  first entry keys: {}", first.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default());
        println!("  first id: {}", first["id"]);
        println!("  first pricing: {}", first["pricing"]);
    }
}

/// A real completion. Proves the key works for its actual purpose, and shows where
/// token counts come from.
#[test]
#[ignore = "needs a live credential, network access, and spends a tiny amount"]
fn probe_tiny_completion() {
    let Some(key) = api_key() else { return };
    let model = std::env::var("CAPYBARAS_PROBE_MODEL")
        .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());

    let payload = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": "Reply with the single word: ready" }],
        "max_tokens": 6,
    });

    let started = Instant::now();
    let response = client()
        .post(format!("{BASE}/chat/completions"))
        .bearer_auth(&key)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .send()
        .expect("the request should reach OpenRouter");
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    let took = started.elapsed();

    println!("POST /chat/completions ({model}) -> HTTP {status} in {}", ms(took));

    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    if let Some(error) = value.get("error") {
        println!("  error: {error}");
        return;
    }
    println!("  reply  : {}", value["choices"][0]["message"]["content"]);
    println!("  usage  : {}", value["usage"]);
    println!("  model  : {}", value["model"]);
    println!("  id     : {}", value["id"]);
}

/// The app's OWN path to a first reply, live.
///
/// `probe_tiny_completion` above talks to the endpoint directly. This one runs
/// `chat::call` — the same code the Send button runs — so the request it builds,
/// the reply it reads and the usage it extracts are the shipped ones, not a
/// parallel implementation written for a test. That is the point: a probe that
/// hand-writes its own request proves the endpoint works, not the app.
#[test]
#[ignore = "needs a live credential, network access, and spends a tiny amount"]
fn probe_the_first_reply_through_the_apps_own_path() {
    let Some(key) = api_key() else { return };
    let model = std::env::var("CAPYBARAS_PROBE_MODEL")
        .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());

    let transport = capybaras_shell::http::HttpTransport::new()
        .expect("build the same client the app uses");

    // The same bound the app resolves for a model the catalogue does not know:
    // the high fallback. A probe that asked for less would not be the shipped path.
    let max_tokens = capybaras_shell::chat::DEFAULT_MAX_REPLY_TOKENS;
    match capybaras_shell::chat::call(&transport, &key, &model, "Reply with the single word: ready", max_tokens) {
        capybaras_shell::chat::ChatOutcome::Replied { text, usage } => {
            println!("reply  : {text}");
            println!("tokens : prompt={} completion={} total={}", usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
            println!("cost   : ${:.6}", usage.cost_usd);
        }
        capybaras_shell::chat::ChatOutcome::Failed { message } => {
            // Printed, not asserted: a live probe reports what happened rather
            // than failing the build on a provider's bad day.
            println!("failed : {message}");
        }
    }
}
