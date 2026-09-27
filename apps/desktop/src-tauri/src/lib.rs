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

pub mod connect;
pub mod credential;
pub mod exchange;
pub mod integrity;
pub mod loopback;
pub mod oauth;

pub struct AppState {
    sidecar: Mutex<Option<Sidecar>>,
    /// Refreshed by the supervision thread. Kept separate from the sidecar lock
    /// so the UI never blocks on a health probe.
    healthy: AtomicBool,
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
    let mut guard = state
        .sidecar
        .lock()
        .map_err(|_| "sidecar lock poisoned".to_string())?;
    let sidecar = guard
        .as_mut()
        .ok_or_else(|| "no sidecar is running".to_string())?;

    let mut message = serde_json::json!({
        "v": 1,
        "type": "action.propose",
        "id": id,
        "action": action,
    });
    if let Some(ctx) = context {
        message["context"] = ctx;
    }

    sidecar.send(&message).map_err(|e| e.to_string())
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
    let mut guard = state
        .sidecar
        .lock()
        .map_err(|_| "sidecar lock poisoned".to_string())?;
    let sidecar = guard
        .as_mut()
        .ok_or_else(|| "no sidecar is running".to_string())?;

    sidecar
        .send(&serde_json::json!({
            "v": 1,
            "type": "approval.answer",
            "id": id,
            "decision": decision,
        }))
        .map_err(|e| e.to_string())
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
        })
        .manage(connect::ConnectState::default())
        .invoke_handler(tauri::generate_handler![
            shell_status,
            propose_action,
            answer_approval,
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
