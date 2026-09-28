//! The catalogue sweep - a deliberate, one-off maintenance action.
//!
//! **WHY THIS EXISTS.** The catalogue is `/models`, and `/models` is a *list of
//! what OpenRouter advertises*, not a list of what it will actually serve. The
//! two are not the same: an entry can be listed and still be unusable, and the
//! app found out the expensive way - a person picked a model from a 452-entry
//! menu and got back "OpenRouter does not recognise that model. Pick a different
//! one from the list." A menu that offers a change that cannot be made is worse
//! than a shorter menu.
//!
//! **DO NOT TRUST `/models` AS EVIDENCE THAT A KEY WORKS.** `GET /models` is a
//! public endpoint: it answers 200 with no credential at all. A dead key and a
//! live key look identical there, which is exactly how an earlier sweep was run
//! against the *workspace's* OpenRouter key, returned `401 User not found` for
//! all 452 models, and proved nothing about the app. This sweep reads
//! **the app's own stored credential** (`credential::load`, the same call
//! `tests/live_api.rs` makes for its probes), because that is the account whose
//! answer matters.
//!
//! **EVERY TEST THAT SPENDS IS `#[ignore]`d, AND THAT IS THE POINT.** `cargo test`
//! must never need a network or a credential, and must never spend money. The
//! classifier below is pure and its tests run offline; the sweep runs only when
//! asked for by name:
//!
//! ```text
//! cargo test --test catalog_sweep -- --ignored --nocapture
//! ```
//!
//! Set `CAPYBARAS_CATALOG_SWEEP_OUT` to choose where the raw JSON lands. Leave it
//! unset and the file goes to the system temp directory. Either way it is written
//! **outside the repository**: the raw result is a maintenance artefact, not
//! source, and a 452-row dump of provider error strings does not belong in a
//! public tree. (The workspace convention on the author's machine is
//! `~/.openclaw/workspace/.openclaw/tmp/`.)
//!
//! **THE SWEEP IS CHEAP ON PURPOSE.** `max_tokens: 1` and a one-word prompt: the
//! question is "will this model answer at all", not "how well does it write". The
//! total spend is summed from the responses' own `usage.cost` fields and printed,
//! so the cost is a measured number rather than an assurance. `GET /key` is read
//! before and after as an independent cross-check of the same figure.
//!
//! The key is read from the OS credential store and is **never** echoed, logged,
//! written to the JSON, or placed in an assertion message.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use capybaras_shell::credential;
use reqwest::blocking::Client;

const KEY_CREDENTIAL: &str = "capybaras.oauth.openrouter";
const BASE: &str = "https://openrouter.ai/api/v1";

/// The cheapest possible request that still proves a model can serve one.
const PROBE_PROMPT: &str = "hi";
const PROBE_MAX_TOKENS: u32 = 1;

/// How many times one model is tried when the failure looks transient.
const ATTEMPTS: usize = 3;
/// Waits between attempts when the provider did not say. The last value is reused
/// for any further attempts.
const BACKOFF: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(6)];
/// How many models are in flight at once. High enough to finish in minutes, low
/// enough not to manufacture the rate limits the classifier is careful about.
const WORKERS: usize = 8;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
/// How much of a provider's error message is kept. Enough to classify by eye,
/// short enough not to paste a page.
const DETAIL_MAX: usize = 200;

// ---------------------------------------------------------------------------
// Classification - pure, and the part that matters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// A 200 carrying a completion. The model serves.
    Ok,
    /// A SYSTEMATIC refusal: the model is not there, has no endpoints, cannot be
    /// called without a key of your own, or rejects the request shape itself.
    /// This is the removable set - the entry is not a model anybody can use.
    Unavailable,
    /// The evidence does not support a conclusion: a rate limit, a 5xx, a timeout,
    /// or a refusal we cannot name.
    ///
    /// **THIS IS NOT THE SAME AS BROKEN, AND THE DIFFERENCE IS THE WHOLE POINT.**
    /// A 429 on a busy afternoon is a statement about the moment, not about the
    /// model. A sweep that removed everything it could not reach would silently
    /// shrink the menu every time it ran, and nobody would ever see the entry that
    /// disappeared - the most destructive kind of drift, because the damage is
    /// invisible. So anything short of a systematic refusal is kept and reported.
    Inconclusive,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Unavailable => "unavailable",
            Verdict::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Classification {
    verdict: Verdict,
    /// A short, stable name for the kind of answer. This is what a rule would be
    /// built from, so it is deliberately coarse.
    family: String,
    /// The provider's own words, clamped. Never the key, never the request.
    detail: String,
}

/// Name a systematic refusal from the provider's own message.
///
/// Order matters: "no endpoints found for X" must be read as a missing endpoint
/// before a generic "not found" pattern can claim it, and BYOK is checked first
/// because its message is the most specific about why the entry cannot serve.
fn refusal_family(lower: &str) -> Option<&'static str> {
    if lower.contains("byok")
        || lower.contains("your own provider api key")
        || lower.contains("own api key")
        || lower.contains("provider api key")
        || lower.contains("requires a provider")
    {
        return Some("byok_only");
    }
    if lower.contains("no endpoints")
        || lower.contains("no available endpoints")
        || lower.contains("no provider")
    {
        return Some("no_endpoints");
    }
    if lower.contains("not a valid model")
        || lower.contains("invalid model")
        || lower.contains("unknown model")
        || lower.contains("model not found")
        || lower.contains("does not exist")
        || lower.contains("not a valid slug")
    {
        return Some("not_a_valid_model");
    }
    if lower.contains("not supported") || lower.contains("unsupported") || lower.contains("does not support") {
        return Some("unsupported");
    }
    if lower.contains("no longer available") || lower.contains("deprecated") || lower.contains("retired") {
        return Some("retired");
    }
    None
}

/// The provider's message for this response, from wherever it put it.
fn error_text(value: &serde_json::Value) -> String {
    if let Some(message) = value.pointer("/error/message").and_then(|v| v.as_str()) {
        return message.to_string();
    }
    if let Some(message) = value.get("error").and_then(|v| v.as_str()) {
        return message.to_string();
    }
    if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
        return message.to_string();
    }
    String::new()
}

/// Flatten, clamp and keep one line of the provider's text for the report.
fn clamp_detail(text: &str) -> String {
    let flattened: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let collapsed = flattened.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= DETAIL_MAX {
        return collapsed;
    }
    let kept: String = collapsed.chars().take(DETAIL_MAX - 1).collect();
    format!("{kept}\u{2026}")
}

/// Decide what one response means. Pure: no clock, no network, no key.
fn classify(status: u16, body: &str) -> Classification {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    let message = error_text(&value);
    let lower = message.to_ascii_lowercase();

    let error_envelope = value.get("error").map(|e| !e.is_null()).unwrap_or(false) || value.is_null() && body.trim().is_empty();

    if (200..300).contains(&status) && !error_envelope {
        let served = value
            .get("choices")
            .and_then(|c| c.as_array())
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if served {
            return Classification {
                verdict: Verdict::Ok,
                family: "served".to_string(),
                detail: String::new(),
            };
        }
        // 200 with no choices: the shape is wrong, not the model. Say so rather
        // than calling it a success the meter would then miscount.
        return Classification {
            verdict: Verdict::Inconclusive,
            family: "no_choices".to_string(),
            detail: clamp_detail(&message),
        };
    }

    // A systematic refusal is a statement about the model, whatever status carries it.
    if let Some(family) = refusal_family(&lower) {
        return Classification {
            verdict: Verdict::Unavailable,
            family: family.to_string(),
            detail: clamp_detail(&message),
        };
    }

    if status == 404 {
        return Classification {
            verdict: Verdict::Unavailable,
            family: "not_found".to_string(),
            detail: clamp_detail(&message),
        };
    }

    // Order matters: a 504 is a specific kind of timeout and must be named
    // before the 5xx arm can swallow it.
    let family = match status {
        408 | 504 => "timeout",
        429 => "rate_limited",
        500..=599 => "server_error",
        402 => "out_of_credit",
        401 => "unauthorized",
        403 => "forbidden",
        400 => "unclassified_refusal",
        0 => "transport",
        _ => "unexpected_status",
    };
    Classification {
        verdict: Verdict::Inconclusive,
        family: family.to_string(),
        detail: clamp_detail(&message),
    }
}

/// A 429 or a 5xx is worth another attempt; nothing else is.
fn is_transient(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

// ---------------------------------------------------------------------------
// The sweep itself
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Probe {
    id: String,
    /// What the app's current filter already drops, so the report does not claim
    /// credit for entries the negative-price rule had already removed.
    already_filtered: bool,
    status: u16,
    verdict: &'static str,
    family: String,
    detail: String,
    attempts: usize,
    cost_usd: f64,
    prompt_tokens: u64,
    completion_tokens: u64,
    served_model: String,
}

fn as_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|t| t.trim().parse::<f64>().ok()))
        .filter(|n| n.is_finite())
}

fn read_usage(body: &str) -> (f64, u64, u64) {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    let cost = value.pointer("/usage/cost").and_then(as_f64).unwrap_or(0.0);
    let prompt = value.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let completion = value.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    (cost, prompt, completion)
}

fn sleep_backoff(attempt: usize, retry_after: Option<u64>) {
    let wait = match retry_after {
        Some(seconds) => Duration::from_secs(seconds.min(30)),
        None => BACKOFF[attempt.min(BACKOFF.len() - 1)],
    };
    std::thread::sleep(wait);
}

/// One model, tried up to `ATTEMPTS` times. Never sees or prints the key.
fn probe(client: &Client, key: &str, id: &str, already_filtered: bool) -> Probe {
    let payload = serde_json::json!({
        "model": id,
        "messages": [{ "role": "user", "content": PROBE_PROMPT }],
        "max_tokens": PROBE_MAX_TOKENS,
    })
    .to_string();

    let mut attempts = 0usize;
    let mut last: Option<Probe> = None;

    while attempts < ATTEMPTS {
        attempts += 1;
        let sent = client
            .post(format!("{BASE}/chat/completions"))
            .bearer_auth(key)
            .header("Content-Type", "application/json")
            .body(payload.clone())
            .send();

        match sent {
            Ok(response) => {
                let status = response.status().as_u16();
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<u64>().ok());
                let body = response.text().unwrap_or_default();
                let classification = classify(status, &body);
                let (cost, prompt, completion) = read_usage(&body);
                let served_model = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string))
                    .unwrap_or_default();

                let probe = Probe {
                    id: id.to_string(),
                    already_filtered,
                    status,
                    verdict: classification.verdict.as_str(),
                    family: classification.family,
                    detail: classification.detail,
                    attempts,
                    cost_usd: cost,
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    served_model,
                };

                if is_transient(status) && attempts < ATTEMPTS {
                    last = Some(probe);
                    sleep_backoff(attempts - 1, retry_after);
                    continue;
                }
                return probe;
            }
            Err(_) => {
                // A timeout or a reset is the moment, not the model. Retry, then
                // keep it as inconclusive rather than removing anything.
                let probe = Probe {
                    id: id.to_string(),
                    already_filtered,
                    status: 0,
                    verdict: Verdict::Inconclusive.as_str(),
                    family: "transport".to_string(),
                    detail: "the request did not complete".to_string(),
                    attempts,
                    cost_usd: 0.0,
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    served_model: String::new(),
                };
                if attempts < ATTEMPTS {
                    last = Some(probe);
                    sleep_backoff(attempts - 1, None);
                    continue;
                }
                return probe;
            }
        }
    }

    // Unreachable: the loop always returns on its last iteration. Kept total.
    last.unwrap_or(Probe {
        id: id.to_string(),
        already_filtered,
        status: 0,
        verdict: Verdict::Inconclusive.as_str(),
        family: "transport".to_string(),
        detail: "the request did not complete".to_string(),
        attempts,
        cost_usd: 0.0,
        prompt_tokens: 0,
        completion_tokens: 0,
        served_model: String::new(),
    })
}

/// The account's cumulative usage, straight from `GET /key`. Used only as an
/// independent cross-check on the sum of the per-response costs.
fn key_usage(client: &Client, key: &str) -> Option<f64> {
    let response = client.get(format!("{BASE}/key")).bearer_auth(key).send().ok()?;
    let body = response.text().ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value.pointer("/data/usage").and_then(as_f64)
}

fn output_path(date: &str) -> PathBuf {
    match std::env::var("CAPYBARAS_CATALOG_SWEEP_OUT") {
        Ok(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => std::env::temp_dir().join(format!("capybaras-catalog-sweep-{date}.json")),
    }
}

/// Days-since-epoch to a civil date, so the artefact carries its own date with no
/// date library. Howard Hinnant's algorithm; the inverse of days_from_civil.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn utc_now() -> (String, String) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let date = format!("{y:04}-{m:02}-{d:02}");
    let (h, mi, s) = ((secs % 86_400) / 3600, (secs % 3600) / 60, secs % 60);
    (format!("{date}T{h:02}:{mi:02}:{s:02}Z"), date)
}

#[test]
#[ignore = "live maintenance sweep: needs the app's stored credential, network access, and spends a small real amount"]
fn sweep_the_catalogue_with_the_apps_own_credential() {
    let key = match credential::load(KEY_CREDENTIAL) {
        Ok(Some(key)) => key,
        Ok(None) => {
            eprintln!("NOT CONNECTED: nothing stored under {KEY_CREDENTIAL}. Use Connect first.");
            return;
        }
        Err(error) => {
            eprintln!("the credential store is unusable here: {error}");
            return;
        }
    };

    let client = Client::builder().timeout(REQUEST_TIMEOUT).build().expect("build an HTTP client");

    let before = key_usage(&client, &key);
    println!("account usage before: {}", before.map(|u| format!("${u:.6}")).unwrap_or_else(|| "(unreadable)".into()));

    // The catalogue, as the app fetches it. `parse_catalog` is the app's own
    // filter, so what it drops here is exactly what the app already drops.
    let body = client
        .get(format!("{BASE}/models"))
        .bearer_auth(&key)
        .send()
        .expect("the model list should reach OpenRouter")
        .text()
        .unwrap_or_default();
    let catalogue: serde_json::Value = serde_json::from_str(&body).expect("the model list should be JSON");
    let kept: HashSet<String> = capybaras_shell::catalog::parse_catalog(&catalogue)
        .into_iter()
        .map(|entry| entry.id)
        .collect();

    let mut ids: Vec<(String, bool)> = Vec::new();
    for entry in catalogue.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default() {
        let Some(id) = entry.get("id").and_then(|v| v.as_str()).map(str::trim) else { continue };
        if id.is_empty() {
            continue;
        }
        ids.push((id.to_string(), !kept.contains(id)));
    }

    println!("catalogue entries: {} ({} already dropped by the app's own filter)", ids.len(), ids.iter().filter(|(_, dropped)| *dropped).count());
    println!("probing {} models, {WORKERS} at a time, max_tokens={PROBE_MAX_TOKENS}...", ids.len());

    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Probe>> = Mutex::new(Vec::with_capacity(ids.len()));

    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            let client = client.clone();
            let key = key.clone();
            let ids = &ids;
            let next = &next;
            let done = &done;
            let results = &results;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= ids.len() {
                    break;
                }
                let (id, already_filtered) = &ids[index];
                let probe = probe(&client, &key, id, *already_filtered);
                let seen = {
                    results.lock().expect("the result list is not poisoned").push(probe);
                    done.fetch_add(1, Ordering::Relaxed) + 1
                };
                if seen % 25 == 0 {
                    println!("  ... {seen}/{}", ids.len());
                }
            });
        }
    });

    let mut results = results.into_inner().expect("the result list is not poisoned");
    results.sort_by(|a, b| a.id.cmp(&b.id));

    let spend: f64 = results.iter().map(|r| r.cost_usd).sum();
    let prompt_tokens: u64 = results.iter().map(|r| r.prompt_tokens).sum();
    let completion_tokens: u64 = results.iter().map(|r| r.completion_tokens).sum();
    let count = |verdict: &str| results.iter().filter(|r| r.verdict == verdict).count();
    let ok = count("ok");
    let unavailable = count("unavailable");
    let inconclusive = count("inconclusive");

    // Group by family, and by provider prefix, to see whether a rule separates
    // the broken entries from the rest.
    let mut families: BTreeMap<String, Vec<&Probe>> = BTreeMap::new();
    for probe in results.iter().filter(|r| r.verdict == "unavailable") {
        families.entry(probe.family.clone()).or_default().push(probe);
    }
    let mut providers: BTreeMap<String, usize> = BTreeMap::new();
    for probe in results.iter().filter(|r| r.verdict == "unavailable") {
        let prefix = probe.id.split_once('/').map(|(p, _)| p).unwrap_or("(none)");
        *providers.entry(prefix.to_string()).or_default() += 1;
    }
    let mut inconclusive_families: BTreeMap<String, usize> = BTreeMap::new();
    for probe in results.iter().filter(|r| r.verdict == "inconclusive") {
        *inconclusive_families.entry(probe.family.clone()).or_default() += 1;
    }

    let (swept_at, date) = utc_now();
    let after = key_usage(&client, &key);
    let delta = match (before, after) {
        (Some(b), Some(a)) => Some(a - b),
        _ => None,
    };

    let to_json = |probe: &Probe| {
        serde_json::json!({
            "id": probe.id,
            "already_filtered": probe.already_filtered,
            "status": probe.status,
            "verdict": probe.verdict,
            "family": probe.family,
            "detail": probe.detail,
            "attempts": probe.attempts,
            "cost_usd": probe.cost_usd,
            "prompt_tokens": probe.prompt_tokens,
            "completion_tokens": probe.completion_tokens,
            "served_model": probe.served_model,
        })
    };

    let document = serde_json::json!({
        "swept_at": swept_at,
        "date": date,
        "source": "apps/desktop/src-tauri/tests/catalog_sweep.rs",
        "endpoint": format!("{BASE}/chat/completions"),
        "probe": { "prompt": PROBE_PROMPT, "max_tokens": PROBE_MAX_TOKENS, "attempts": ATTEMPTS, "workers": WORKERS },
        "catalogue_size": results.len(),
        "counts": { "ok": ok, "unavailable": unavailable, "inconclusive": inconclusive },
        "spend_usd": spend,
        "usage": { "prompt_tokens": prompt_tokens, "completion_tokens": completion_tokens },
        "account_usage_before": before,
        "account_usage_after": after,
        "account_usage_delta": delta,
        "unavailable_families": families.iter().map(|(k, v)| (k.clone(), v.len())).collect::<BTreeMap<_, _>>(),
        "unavailable_providers": providers,
        "inconclusive_families": inconclusive_families,
        "unavailable_ids": results.iter().filter(|r| r.verdict == "unavailable").map(|r| r.id.clone()).collect::<Vec<_>>(),
        "results": results.iter().map(to_json).collect::<Vec<_>>(),
    });

    let path = output_path(&date);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, serde_json::to_string_pretty(&document).expect("serialise the report")).expect("write the report");

    println!();
    println!("================ catalogue sweep ================");
    println!("when        : {swept_at}");
    println!("catalogue   : {} entries", results.len());
    println!("ok          : {ok}");
    println!("unavailable : {unavailable}");
    println!("inconclusive: {inconclusive}");
    println!("spend       : ${spend:.6} (from {} responses' own usage.cost)", results.len());
    if let Some(delta) = delta {
        println!("cross-check : GET /key usage moved ${delta:.6}");
    }
    println!();
    println!("unavailable, by family:");
    for (family, probes) in &families {
        println!("  {family:<20} {}", probes.len());
        for probe in probes.iter().take(5) {
            println!("      {} - HTTP {} - {}", probe.id, probe.status, probe.detail);
        }
    }
    println!("unavailable, by provider prefix:");
    for (prefix, n) in &providers {
        println!("  {prefix:<20} {n}");
    }
    println!();
    println!("inconclusive, by family (KEPT - see the Inconclusive variant):");
    for (family, n) in &inconclusive_families {
        println!("  {family:<20} {n}");
    }
    println!();
    println!("raw result  : {}", path.display());
    println!("================================================");
}

// ---------------------------------------------------------------------------
// Offline tests: the classifier never needs a network
// ---------------------------------------------------------------------------

#[test]
fn a_completion_is_ok() {
    let body = serde_json::json!({
        "model": "vendor/model",
        "choices": [{ "message": { "role": "assistant", "content": "hi" } }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 1, "cost": 0.000001 },
    })
    .to_string();
    let result = classify(200, &body);
    assert_eq!(result.verdict, Verdict::Ok);
    assert_eq!(result.family, "served");
}

#[test]
fn a_404_is_unavailable() {
    let body = serde_json::json!({ "error": { "message": "No endpoints found.", "code": 404 } }).to_string();
    assert_eq!(classify(404, &body).verdict, Verdict::Unavailable);
}

#[test]
fn the_named_refusals_are_unavailable_and_are_told_apart() {
    let cases = [
        (400, "vendor/model is not a valid model ID", "not_a_valid_model"),
        (404, "No endpoints found for vendor/model.", "no_endpoints"),
        (403, "This model requires your own provider API key (BYOK).", "byok_only"),
        (400, "This model does not support the max_tokens parameter", "unsupported"),
        (404, "This model has been retired.", "retired"),
    ];
    for (status, message, family) in cases {
        let body = serde_json::json!({ "error": { "message": message } }).to_string();
        let result = classify(status, &body);
        assert_eq!(result.verdict, Verdict::Unavailable, "{message}");
        assert_eq!(result.family, family, "{message}");
    }
}

/// THE HONESTY TEST. A rate limit, a 5xx and a timeout are statements about the
/// afternoon, not about the model. Removing on that basis would shrink the menu
/// invisibly, which is the failure this whole file exists to avoid.
#[test]
fn a_rate_limit_or_a_server_error_is_inconclusive_never_removable() {
    let body = serde_json::json!({ "error": { "message": "Rate limit exceeded: free-models-per-day" } }).to_string();
    assert_eq!(classify(429, &body).verdict, Verdict::Inconclusive);
    assert_eq!(classify(429, &body).family, "rate_limited");
    assert_eq!(classify(503, "upstream unavailable").verdict, Verdict::Inconclusive);
    assert_eq!(classify(502, "").verdict, Verdict::Inconclusive);
    assert_eq!(classify(0, "").verdict, Verdict::Inconclusive);
}

/// A 400 we cannot name is NOT evidence that the model is unusable. It stays.
#[test]
fn an_unclassifiable_refusal_is_kept_rather_than_removed() {
    let body = serde_json::json!({ "error": { "message": "Something unexpected happened" } }).to_string();
    assert_eq!(classify(400, &body).verdict, Verdict::Inconclusive);
    assert_eq!(classify(400, &body).family, "unclassified_refusal");
    assert_eq!(classify(403, &serde_json::json!({ "error": { "message": "Revoked" } }).to_string()).verdict, Verdict::Inconclusive);
}

/// Some proxies answer 200 with an error envelope. That is a refusal, not a reply.
#[test]
fn an_error_inside_a_200_is_not_a_completion() {
    let body = serde_json::json!({ "error": { "message": "vendor/model is not a valid model ID" } }).to_string();
    let result = classify(200, &body);
    assert_eq!(result.verdict, Verdict::Unavailable);
    assert_eq!(result.family, "not_a_valid_model");
}

#[test]
fn a_200_with_no_choices_is_inconclusive_not_a_success() {
    assert_eq!(classify(200, "{}").verdict, Verdict::Inconclusive);
    assert_eq!(classify(200, "not json at all").verdict, Verdict::Inconclusive);
}

#[test]
fn only_a_rate_limit_or_a_server_error_is_worth_another_attempt() {
    for status in [429, 500, 502, 503, 504] {
        assert!(is_transient(status), "{status} should be retried");
    }
    for status in [200, 400, 401, 402, 403, 404] {
        assert!(!is_transient(status), "{status} should not be retried");
    }
}

#[test]
fn the_detail_is_flattened_and_clamped() {
    let detail = clamp_detail(&"a\n\nb\t".repeat(200));
    assert!(!detail.contains('\n'));
    assert!(detail.chars().count() <= DETAIL_MAX);
}

#[test]
fn the_utc_formatter_agrees_with_known_dates() {
    // 2026-09-28T00:00:00Z = 1790553600 (a date this sweep actually ran on)
    let (date, _) = {
        let secs = 1_790_553_600u64;
        let (y, m, d) = civil_from_days((secs / 86_400) as i64);
        (format!("{y:04}-{m:02}-{d:02}"), ())
    };
    assert_eq!(date, "2026-09-28");
    assert_eq!(civil_from_days(0), (1970, 1, 1));
}
