//! The Tauri shell.
//!
//! No logic lives here. A command deserialises its arguments, calls the core
//! crate, and returns the result. A decision made in a command handler is a
//! decision that cannot be tested on Linux.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use z_i_p_core::{history, jobs::Jobs, settings, Error, Listing, Outcome, Progress, Request};

struct AppState {
    jobs: Jobs,
    /// Archives opened from Finder before the window was ready to hear it.
    opened: Mutex<Vec<String>>,
    settings_file: PathBuf,
    history_file: PathBuf,
}

/// Runs blocking work off the main thread, so a slow archive never freezes
/// the window.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .expect("background task panicked")
}

#[tauri::command]
async fn inspect(path: String, password: Option<String>) -> Result<Listing, Error> {
    blocking(move || z_i_p_core::list(Path::new(&path), password.as_deref())).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    job: u64,
    done: u64,
    total: u64,
    current: String,
}

/// Sends progress to the window at most ten times a second.
struct Emit {
    app: AppHandle,
    job: u64,
    stop: Arc<AtomicBool>,
    last: Cell<Option<Instant>>,
}

impl Progress for Emit {
    fn report(&self, done: u64, total: u64, current: &str) {
        let now = Instant::now();
        if self
            .last
            .get()
            .is_some_and(|t| now - t < Duration::from_millis(100))
        {
            return;
        }
        self.last.set(Some(now));
        let _ = self.app.emit(
            "extract-progress",
            ProgressEvent {
                job: self.job,
                done,
                total,
                current: current.to_string(),
            },
        );
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

#[tauri::command]
async fn extract(app: AppHandle, job: u64, request: Request) -> Result<Outcome, Error> {
    blocking(move || {
        let state = app.state::<AppState>();
        let stop = state.jobs.start(job);
        let progress = Emit {
            app: app.clone(),
            job,
            stop,
            last: Cell::new(None),
        };
        let result = z_i_p_core::extract(&request, &progress);
        state.jobs.finish(job);
        // A password prompt or a stop is not worth remembering.
        if !matches!(
            result,
            Err(Error::PasswordRequired | Error::WrongPassword | Error::Cancelled)
        ) {
            let _ = history::record(&state.history_file, history::Item::of(&request, &result));
        }
        result
    })
    .await
}

#[tauri::command]
fn cancel(state: State<AppState>, job: u64) -> bool {
    state.jobs.cancel(job)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> settings::Settings {
    settings::load(&state.settings_file)
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: settings::Settings) -> Result<(), String> {
    settings::save(&state.settings_file, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn destination_for(state: State<AppState>, archive: String) -> Option<String> {
    let s = settings::load(&state.settings_file);
    settings::destination_for(&s, Path::new(&archive)).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn get_history(state: State<AppState>) -> Vec<history::Item> {
    history::load(&state.history_file)
}

#[tauri::command]
fn clear_history(state: State<AppState>) -> Result<(), String> {
    history::clear(&state.history_file).map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    z_i_p_core::platform::reveal(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    z_i_p_core::platform::open(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn take_opened(state: State<AppState>) -> Vec<String> {
    std::mem::take(&mut *state.opened.lock().expect("opened lock"))
}

/// Build and run the application.
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(AppState {
                jobs: Jobs::default(),
                opened: Mutex::new(Vec::new()),
                settings_file: dir.join("settings.json"),
                history_file: dir.join("history.json"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inspect,
            extract,
            cancel,
            get_settings,
            save_settings,
            destination_for,
            get_history,
            clear_history,
            reveal,
            open_path,
            take_opened
        ])
        .build(tauri::generate_context!())
        .expect("failed to start the application");

    app.run(|_app, _event| {
        // Finder's "Open With", a double-click, or a drop on the Dock icon.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = _event {
            let paths: Vec<String> = urls
                .iter()
                .filter_map(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if let Some(state) = _app.try_state::<AppState>() {
                state
                    .opened
                    .lock()
                    .expect("opened lock")
                    .extend(paths.clone());
            }
            let _ = _app.emit("files-opened", paths);
            if let Some(w) = _app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }
    });
}
