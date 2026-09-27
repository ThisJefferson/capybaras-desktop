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
use tauri::{Manager, RunEvent};

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
        .invoke_handler(tauri::generate_handler![shell_status])
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            let node = paths::bundled_node();
            let script = paths::sidecar_script();
            let state_dir = paths::state_dir();

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
