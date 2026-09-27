pub mod paths;
pub mod sidecar;

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use sidecar::Sidecar;
use tauri::{Manager, RunEvent};

pub struct AppState {
    sidecar: Mutex<Option<Sidecar>>,
}

#[derive(Serialize)]
pub struct ShellStatus {
    shell_pid: u32,
    exe_dir: String,
    state_dir: String,
    node_path: String,
    node_exists: bool,
    sidecar_running: bool,
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
                // `s` is dropped here. Its job handle closes, which terminates
                // anything the sidecar left running.
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            sidecar: Mutex::new(None),
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
            let state = paths::state_dir();

            log::info!("shell up pid={}", std::process::id());
            log::info!("  exe dir    : {}", paths::exe_dir().display());
            log::info!("  state dir  : {}", state.display());
            log::info!("  node       : {}", node.display());
            log::info!("  sidecar    : {}", script.display());

            let spawned = match Sidecar::spawn(&node, &script, &state) {
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

            *app.state::<AppState>().sidecar.lock().unwrap() = spawned;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            RunEvent::ExitRequested { .. } | RunEvent::Exit => stop_sidecar(app_handle),
            _ => {}
        });
}
