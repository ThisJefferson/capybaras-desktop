pub mod paths;
pub mod sidecar;
pub mod single_instance;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use sidecar::Sidecar;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, RunEvent};

pub mod catalog;
pub mod chat;
pub mod connect;
pub mod credential;
pub mod credits;
pub mod exchange;
pub mod http;
pub mod integrity;
pub mod loopback;
pub mod meter;
pub mod oauth;
pub mod usage;

pub struct AppState {
    sidecar: Mutex<Option<Sidecar>>,
    /// Refreshed by the supervision thread. Kept separate from the sidecar lock
    /// so the UI never blocks on a health probe.
    healthy: AtomicBool,
    /// The catalogue as last fetched. Kept for one reason only: `send_message`
    /// asks the CHOSEN model for its own reply ceiling, so a long answer is asked
    /// for in full instead of being truncated at a number picked in the shell.
    /// Display data, never consulted by the gate, and the key never reaches it.
    catalogue: Mutex<Vec<catalog::CatalogEntry>>,
}

#[derive(Serialize)]
pub struct ShellStatus {
    shell_pid: u32,
    exe_dir: String,
    state_dir: String,
    node_path: String,
    node_exists: bool,
    sidecar_running: bool,
    sidecar_healthy: bool,
    sidecar_pid: Option<u32>,
    job_assigned: bool,
    job_error: Option<String>,
}

#[tauri::command]
fn shell_status(state: tauri::State<'_, AppState>) -> ShellStatus {
    let mut guard = state.sidecar.lock().unwrap();
    let node = paths::bundled_node();

    let (running, pid, assigned, err) = match guard.as_mut() {
        Some(s) => (
            s.is_running(),
            s.pid(),
            s.job_assigned(),
            s.job_error().map(str::to_string),
        ),
        None => (false, None, false, None),
    };

    ShellStatus {
        shell_pid: std::process::id(),
        exe_dir: paths::exe_dir().display().to_string(),
        state_dir: paths::state_dir().display().to_string(),
        node_path: node.display().to_string(),
        node_exists: node.is_file(),
        sidecar_running: running,
        sidecar_healthy: state.healthy.load(Ordering::Relaxed),
        sidecar_pid: pid,
        job_assigned: assigned,
        job_error: err,
    }
}

/// Send one message to the sidecar.
///
/// Shared by every command that talks to the agent, so there is exactly one
/// place that decides how the shell reaches it -- and one wording for every
/// failure. Three copies of "no sidecar is running" is three chances to describe
/// the same state differently.
fn send_to_sidecar(
    state: &tauri::State<'_, AppState>,
    message: serde_json::Value,
) -> Result<(), String> {
    let mut guard = state
        .sidecar
        .lock()
        .map_err(|_| "sidecar lock poisoned".to_string())?;
    let sidecar = guard
        .as_mut()
        .ok_or_else(|| "no sidecar is running".to_string())?;

    sidecar.send(&message).map_err(|e| e.to_string())
}

/// Ask the sidecar to consider an action.
///
/// Returns immediately. The outcome arrives as an **event**, not as a return
/// value -- because for a `confirm` action the outcome may not exist for minutes,
/// or ever.
#[tauri::command]
fn propose_action(
    state: tauri::State<'_, AppState>,
    id: String,
    action: serde_json::Value,
    context: Option<serde_json::Value>,
) -> Result<(), String> {
    let mut message = serde_json::json!({
        "v": 1,
        "type": "action.propose",
        "id": id,
        "action": action,
    });
    if let Some(ctx) = context {
        message["context"] = ctx;
    }

    send_to_sidecar(&state, message)
}

/// Carry a human's decision back to the sidecar.
///
/// **This is the only path by which a blocked action may proceed.** There is
/// deliberately no other — no default, no fallback, no trusted-target shortcut.
#[tauri::command]
fn answer_approval(
    state: tauri::State<'_, AppState>,
    id: String,
    decision: String,
) -> Result<(), String> {
    send_to_sidecar(
        &state,
        serde_json::json!({
            "v": 1,
            "type": "approval.answer",
            "id": id,
            "decision": decision,
        }),
    )
}

/// The message that asks the sidecar for the remembered choices.
///
/// Public so the shape the shell sends can be pinned by a test: if the shell and
/// the sidecar ever disagree about the name of this message, the list would come
/// back empty and read as "nothing is remembered" rather than as a bug.
pub fn grants_list_message() -> serde_json::Value {
    serde_json::json!({ "v": 1, "type": "grants.list" })
}

/// The message that revokes one remembered choice, by id.
///
/// Revocation is the user's, so it is a first-class message rather than something
/// reachable only by editing a file. Deleting `grants.json` also works, and that
/// is on purpose: a permission you cannot inspect and withdraw by hand is not
/// really one you hold.
pub fn grants_revoke_message(id: &str) -> serde_json::Value {
    serde_json::json!({ "v": 1, "type": "grants.revoke", "scope": { "id": id } })
}

/// Ask what has been remembered. The answer arrives as `capybaras://grants`.
#[tauri::command]
fn list_grants(state: tauri::State<'_, AppState>) -> Result<(), String> {
    send_to_sidecar(&state, grants_list_message())
}

/// Forget one remembered choice. The refreshed list arrives as
/// `capybaras://grants`.
#[tauri::command]
fn revoke_grant(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    send_to_sidecar(&state, grants_revoke_message(&id))
}

/// The current meter.
///
/// A read, not a subscription: the interface calls this once on load and is kept
/// current from then on by `capybaras://usage` (see `meter.rs`).
#[tauri::command]
fn usage_status(meter: tauri::State<'_, meter::UsageMeter>) -> usage::Snapshot {
    meter.snapshot()
}

/// Record a finished model call, and tell the interface what it cost.
///
/// **THIS IS THE ONLY PLACE A CALL ENTERS THE METER.** Anything that completes a
/// model request must report it here; if a path forgets to, the meter is silently
/// wrong rather than visibly broken -- the failure mode `usage.rs` warns about in
/// its own header.
///
/// It returns the new snapshot as well as emitting it, so a caller that wants the
/// figure does not have to race the event to get it.
#[tauri::command]
fn record_model_call(
    app: tauri::AppHandle,
    meter: tauri::State<'_, meter::UsageMeter>,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    cost_usd: f64,
) -> usage::Snapshot {
    crate::meter::record_and_notify(
        meter.inner(),
        usage::Tokens {
            prompt: prompt_tokens,
            completion: completion_tokens,
            total: total_tokens,
        },
        cost_usd,
        |snapshot| {
            // Emitted for EVERY call, not only the ones that move a total: see
            // the note at the top of meter.rs for why standing still is the case
            // worth showing.
            let _ = app.emit(crate::meter::USAGE_EVENT, snapshot);
        },
    )
}

/// The interface events for the first model call.
///
/// Public so the emitter and the frontend cannot drift apart, the same way
/// `meter::USAGE_EVENT` is.
pub const REPLY_EVENT: &str = "capybaras://reply";
pub const MESSAGE_FAILED_EVENT: &str = "capybaras://message-failed";
pub const MODELS_EVENT: &str = "capybaras://models";
pub const MODELS_FAILED_EVENT: &str = "capybaras://models-failed";

/// The stream's own event: one piece of a reply, arriving while it is written.
///
/// **SEPARATE FROM `capybaras://reply` ON PURPOSE.** A delta is *transient* — text on
/// its way to a reply that has not finished — while `reply` is the finished turn. If
/// both arrived on one event, the window could never tell whether what it has is the
/// answer or a fragment of one, and a half-written reply would be indistinguishable
/// from a whole one.
pub const DELTA_EVENT: &str = "capybaras://delta";

/// Set by `stop_generation`, read by the streaming call before every line.
///
/// **A STATIC, BECAUSE THERE IS ONE GENERATION AT A TIME.** The app is
/// single-instance (`single_instance.rs`) and one window drives one model call, so a
/// single flag cannot be raced by a second request. A second concurrent generation
/// would need more than a second flag, and is not something this app does.
static STOP_GENERATION: AtomicBool = AtomicBool::new(false);

/// One turn, as the interface sends it.
///
/// The interface holds the conversation and sends it whole; the shell keeps none.
#[derive(serde::Deserialize)]
struct Turn {
    role: String,
    content: String,
}

/// Load the stored provider key, or explain plainly why there is none.
///
/// **FAILS CLOSED.** Anything other than a readable credential is "not
/// connected", never "proceed and hope" (M5-onboarding.md §3). The credential
/// error is deliberately not interpolated: a store detail is not a message for a
/// person, and an error string is the last place one should travel.
fn stored_key() -> Result<String, String> {
    match credential::load(connect::CREDENTIAL_NAME) {
        Ok(Some(key)) => Ok(key),
        Ok(None) => Err(
            "Capybaras is not connected to OpenRouter yet. Use Connect first.".to_string(),
        ),
        Err(_) => Err(
            "Capybaras cannot read the Windows credential store, so it cannot use your key. \
             It will not keep one anywhere else."
                .to_string(),
        ),
    }
}

/// Read the account balance and put it on the meter, then tell the interface.
///
/// **WHERE THIS IS CALLED, AND WHY BOTH MOMENTS.** The balance is display only: it
/// is never consulted by a call, never enforced, and never a limit (D26). It
/// changes for exactly two reasons, so it is read at the two moments those happen.
///
/// 1. **After the model list is fetched.** That is the first point at which the
///    shell knows it holds a usable key and is filling the interface in, so the
///    "left" figure arrives with the rest of the panel rather than turning up only
///    after the user has already paid for something.
/// 2. **After a call completes.** This is the moment the number actually moves: a
///    completed call is spend, and spend is the only thing this app does that
///    changes the balance. Reading it here keeps the figure current rather than
///    frozen at launch.
///
/// A read that fails attaches nothing — `meter::apply_credit` leaves the last known
/// figure standing, or the row absent (show nothing rather than something wrong).
/// The key is already in hand at both call sites, so it is passed in rather than
/// read from the OS store a second time.
fn refresh_credits(app: &tauri::AppHandle, transport: &dyn http::Transport, key: &str) {
    let fetched = credits::fetch_credits(transport, key);
    let meter = app.state::<meter::UsageMeter>();
    meter::apply_credit(meter.inner(), fetched, |snapshot| {
        // The same event the meter already pushes on (D22), so the interface needs
        // no new listener: `renderUsage` draws the "left" row whenever a credit is
        // present, and simply leaves it out when one is not.
        let _ = app.emit(meter::USAGE_EVENT, snapshot);
    });
}

/// Load the model list with the stored key.
///
/// The answer arrives as an event rather than a return value, for the same reason
/// the sign-in does: the interface must not freeze for a network round trip.
/// `capybaras://models` carries the entries and the default; `capybaras://models-failed`
/// carries a reason.
#[tauri::command]
fn fetch_models(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let key = match stored_key() {
            Ok(key) => key,
            Err(message) => {
                let _ = app.emit(MODELS_FAILED_EVENT, serde_json::json!({ "message": message }));
                return;
            }
        };

        let transport = match http::HttpTransport::new() {
            Ok(transport) => transport,
            Err(_) => {
                let _ = app.emit(
                    MODELS_FAILED_EVENT,
                    serde_json::json!({ "message": "Capybaras could not start its network client. Restart the app and try again." }),
                );
                return;
            }
        };

        match catalog::fetch_catalog(&transport, &key) {
            catalog::CatalogResult::Models(models) => {
                // Keep the catalogue so a later Send can ask the chosen model for
                // its own reply ceiling. Display data only: it shapes a request
                // bound, never a gate decision.
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut guard) = state.catalogue.lock() {
                        *guard = models.clone();
                    }
                }
                // The default is chosen here, over the real list, so the interface
                // never has to guess which model is the sensible first pick.
                let default = catalog::default_model(&models).map(str::to_string);
                let _ = app.emit(
                    MODELS_EVENT,
                    serde_json::json!({ "models": models, "default": default }),
                );
            }
            catalog::CatalogResult::Failed { detail, .. } => {
                let _ = app.emit(MODELS_FAILED_EVENT, serde_json::json!({ "message": detail }));
            }
        }

        // The catalogue is in hand and the key is proven readable, so this is the
        // moment to read the balance as well: the panel is filled in once, and the
        // "left" figure is part of that rather than pending a first paid call.
        refresh_credits(&app, &transport, &key);

        drop(key);
    });
}

/// One message, one reply — the first reply.
///
/// Dispatches and returns; the answer arrives as `capybaras://reply` or
/// `capybaras://message-failed`. An `Err` here is something known *before* any
/// network call — not connected, nothing to send, no model chosen — so the person
/// gets the reason immediately rather than waiting for a round trip to tell them.
#[tauri::command]
fn send_message(app: tauri::AppHandle, prompt: String, model: String) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("Type a message first.".to_string());
    }
    if model.trim().is_empty() {
        return Err("Pick a model first — load the model list, then choose one.".to_string());
    }

    let key = stored_key()?;
    let chosen = model.trim().to_string();

    // The request's `max_tokens` is the CHOSEN model's OWN ceiling, read from the
    // catalogue the shell already fetched — not a number picked here, and never a
    // low one. An unknown ceiling resolves to a high fallback (`chat::reply_limit`),
    // because the operator's instruction is that the answer be as long as it needs
    // to be. Display data shaping a request bound: it cannot raise or lower a gate.
    let max_tokens = reply_limit_for(&app, &chosen);

    std::thread::spawn(move || {
        let transport = match http::HttpTransport::new() {
            Ok(transport) => transport,
            Err(_) => {
                let _ = app.emit(
                    MESSAGE_FAILED_EVENT,
                    serde_json::json!({ "message": "Capybaras could not start its network client. Restart the app and try again." }),
                );
                return;
            }
        };

        chat::perform(
            &transport,
            &key,
            &chosen,
            &prompt,
            max_tokens,
            |usage| {
                // THE ONE SEAM (D22). Every finished call — success or failure —
                // reports through `record_model_call`, which moves the totals and
                // emits `capybaras://usage`. A path that skipped this would make
                // the meter silently wrong rather than visibly broken.
                record_model_call(
                    app.clone(),
                    app.state::<meter::UsageMeter>(),
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                    usage.cost_usd,
                );
            },
            |outcome| match outcome {
                chat::ChatOutcome::Replied { text, .. } => {
                    let _ = app.emit(
                        REPLY_EVENT,
                        serde_json::json!({ "text": text, "model": chosen }),
                    );
                }
                chat::ChatOutcome::Failed { message } => {
                    let _ = app.emit(MESSAGE_FAILED_EVENT, serde_json::json!({ "message": message }));
                }
            },
        );

        // The call is finished and paid for, so the balance has moved. Refresh it
        // here — this is the moment the number changes — and push it on the same
        // usage event the call itself already reported on, so the "left" figure
        // catches up with the call that just changed it.
        refresh_credits(&app, &transport, &key);

        // The key's in-memory copy is released here. Not scrubbed — the same known
        // limit `connect.rs` records for the exchange step.
        drop(key);
    });

    Ok(())
}

/// Send a conversation and stream the answer back.
///
/// **THE WHOLE THREAD TRAVELS, NOT JUST THE LATEST LINE.** The interface holds the
/// conversation; the shell holds none. That keeps the thread in the one place it is
/// shown and can be cleared, and it means a follow-up carries its own context without
/// a remote session to go stale.
///
/// Pieces arrive as `capybaras://delta` while the model writes; the finished turn as
/// `capybaras://reply`; a failure as `capybaras://message-failed`. **A failure is still
/// reported to the meter** — D22 — because a call that consumed something must not
/// vanish from the figures just because it went wrong.
#[tauri::command]
fn send_conversation(
    app: tauri::AppHandle,
    messages: Vec<Turn>,
    model: String,
) -> Result<(), String> {
    if messages.is_empty() {
        return Err("Type a message first.".to_string());
    }
    let chosen = model.trim().to_string();
    if chosen.is_empty() {
        return Err("Pick a model first — load the model list, then choose one.".to_string());
    }

    let key = stored_key()?;
    let turns: Vec<chat::Message> = messages
        .into_iter()
        .map(|turn| chat::Message { role: turn.role, content: turn.content })
        .collect();
    let max_tokens = reply_limit_for(&app, &chosen);

    // A new generation clears the last Stop. Without this, the Stop that ended the
    // previous answer would end this one before it had written a word.
    STOP_GENERATION.store(false, Ordering::SeqCst);

    std::thread::spawn(move || {
        let transport = match http::HttpTransport::new() {
            Ok(transport) => transport,
            Err(_) => {
                let _ = app.emit(
                    MESSAGE_FAILED_EVENT,
                    serde_json::json!({ "message": "Capybaras could not start its network client. Restart the app and try again." }),
                );
                return;
            }
        };

        let piece_app = app.clone();
        let outcome = chat::call_streaming(
            &transport,
            &key,
            &chosen,
            &turns,
            max_tokens,
            &|| STOP_GENERATION.load(Ordering::SeqCst),
            |delta| {
                let _ = piece_app.emit(DELTA_EVENT, serde_json::json!({ "text": delta }));
            },
        );

        // THE ONE SEAM (D22). Every finished call — streamed or not, success or
        // failure — reports through `record_model_call`, which moves the totals and
        // emits `capybaras://usage`.
        let usage = chat::report_of(&outcome);
        record_model_call(
            app.clone(),
            app.state::<meter::UsageMeter>(),
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            usage.cost_usd,
        );

        match &outcome {
            chat::ChatOutcome::Replied { text, .. } => {
                let _ = app.emit(REPLY_EVENT, serde_json::json!({ "text": text, "model": chosen }));
            }
            chat::ChatOutcome::Failed { message } => {
                let _ = app.emit(MESSAGE_FAILED_EVENT, serde_json::json!({ "message": message }));
            }
        }

        // The call is finished and paid for, so the balance has moved.
        refresh_credits(&app, &transport, &key);

        // The key's in-memory copy is released here. Not scrubbed — the same known
        // limit `connect.rs` records for the exchange step.
        drop(key);
    });

    Ok(())
}

/// Stop the generation in flight.
///
/// Sets the flag `call_streaming` reads before every line; the read then ends and the
/// connection closes, so the model is not left writing to a reader that has gone.
/// Returns at once — the calling thread unwinds on its own, and the interface already
/// holds every piece it was sent.
#[tauri::command]
fn stop_generation() {
    STOP_GENERATION.store(true, Ordering::SeqCst);
}

/// The chosen model's own reply ceiling, or the high fallback.
///
/// Looks the model up in the catalogue the shell last fetched. A model the
/// catalogue does not know (or an entry with no stated ceiling) resolves to
/// `chat::reply_limit(None)` — the high fallback, never a short limit.
fn reply_limit_for(app: &tauri::AppHandle, model: &str) -> u32 {
    let ceiling = app.try_state::<AppState>().and_then(|state| {
        state.catalogue.lock().ok().and_then(|guard| {
            guard
                .iter()
                .find(|entry| entry.id == model)
                .and_then(|entry| entry.max_completion_tokens)
        })
    });
    chat::reply_limit(ceiling)
}

/// Turn protocol messages into interface events.
///
/// Before this existed the shell had no way to tell the interface anything — its
/// only route was a command the frontend polled every two seconds. This is the
/// push path, and without it the protocol built in M4.1 goes nowhere.
fn spawn_protocol_forwarder(app: &tauri::AppHandle, rx: std::sync::mpsc::Receiver<String>) {
    let handle = app.clone();
    std::thread::spawn(move || {
        while let Ok(line) = rx.recv() {
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue; // a malformed line costs one message, never the stream
            };

            let event = match message["type"].as_str().unwrap_or_default() {
                "approval.required" => "capybaras://approval-required",
                "approval.resolved" => "capybaras://approval-resolved",
                "action.proceeded" => "capybaras://action-proceeded",
                "action.dry_run" => "capybaras://action-dry-run",
                "agents.state" => "capybaras://agents",
                "grants.listed" => "capybaras://grants",
                "error" => "capybaras://protocol-error",
                "ready" => "capybaras://ready",
                // Heartbeats and anything unrecognised: nothing worth showing.
                _ => continue,
            };

            let _ = handle.emit(event, message);
        }
    });
}

/// Ask the sidecar to stop, then let the job handle close as the backstop.
fn stop_sidecar(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.sidecar.lock() {
            if let Some(mut s) = guard.take() {
                let outcome = s.stop(Duration::from_secs(5));
                log::info!(
                    "sidecar stop: requested={} graceful={} forced={} exit={:?} waited={}ms",
                    outcome.requested,
                    outcome.graceful,
                    outcome.forced,
                    outcome.exit_code,
                    outcome.waited_ms
                );
                // `s` drops here. Its job handle closes, which terminates
                // anything the sidecar left running.
                state.healthy.store(false, Ordering::Relaxed);
            }
        }
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Capybaras", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Capybaras", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("capybaras")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Capybaras")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Watches the sidecar so the interface can report a dead agent rather than
/// showing a window that quietly stopped working.
fn spawn_health_thread(app: &tauri::AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3));
        let Some(state) = handle.try_state::<AppState>() else {
            return; // app is shutting down
        };
        let alive = match state.sidecar.lock() {
            Ok(mut guard) => guard.as_mut().map(|s| s.is_running()).unwrap_or(false),
            Err(_) => false,
        };
        let was = state.healthy.swap(alive, Ordering::Relaxed);
        if was && !alive {
            log::error!("sidecar is no longer running");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            sidecar: Mutex::new(None),
            healthy: AtomicBool::new(false),
            catalogue: Mutex::new(Vec::new()),
        })
        .manage(connect::ConnectState::default())
        // The meter. Kept out of `AppState` on purpose: it needs no lock of its
        // own beyond the one it already has, and holding it separately means a
        // slow snapshot can never contend with the sidecar lock.
        .manage(meter::UsageMeter::new())
        .invoke_handler(tauri::generate_handler![
            shell_status,
            propose_action,
            answer_approval,
            list_grants,
            revoke_grant,
            usage_status,
            record_model_call,
            fetch_models,
            send_message,
            send_conversation,
            stop_generation,
            connect::connect_status,
            connect::start_connect
        ])
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            let node = paths::bundled_node();
            let script = paths::sidecar_script();
            let state_dir = paths::state_dir();

            // ---- sidecar integrity (docs/threat-model.md, T2) --------------
            // The decision logic is a plain, writable file. Refuse to launch a
            // bundle that is not the one this binary was built against, rather
            // than running whatever happens to be there.
            if let Err(problem) = integrity::verify(&script) {
                log::error!("refusing to start: {problem}");
                return Err(problem.into());
            }
            log::info!("sidecar integrity: verified");

            log::info!("shell up pid={}", std::process::id());
            log::info!("  exe dir   : {}", paths::exe_dir().display());
            log::info!("  state dir : {}", state_dir.display());
            log::info!("  node      : {}", node.display());
            log::info!("  sidecar   : {}", script.display());

            let sidecar = match Sidecar::spawn(&node, &script, &state_dir) {
                Ok(s) => {
                    log::info!(
                        "sidecar spawned pid={:?} job_assigned={}",
                        s.pid(),
                        s.job_assigned()
                    );
                    if let Some(e) = s.job_error() {
                        log::error!("job object problem: {e}");
                    }
                    Some(s)
                }
                Err(e) => {
                    log::error!("sidecar spawn failed: {e}");
                    None
                }
            };

            let healthy = sidecar.is_some();
            let state = app.state::<AppState>();
            *state.sidecar.lock().unwrap() = sidecar;
            state.healthy.store(healthy, Ordering::Relaxed);
            drop(state);

            // Hand the protocol stream to the forwarder: after this, the
            // sidecar's messages become interface events.
            let receiver = app
                .state::<AppState>()
                .sidecar
                .lock()
                .unwrap()
                .as_mut()
                .and_then(|s| s.take_receiver());
            if let Some(rx) = receiver {
                spawn_protocol_forwarder(&app.handle().clone(), rx);
            }

            build_tray(app)?;
            spawn_health_thread(&app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            RunEvent::ExitRequested { .. } | RunEvent::Exit => stop_sidecar(app_handle),
            _ => {}
        });
}
