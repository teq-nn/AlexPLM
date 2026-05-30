pub mod autocommit;
pub mod classifier;
pub mod import;
pub mod import_gate;
pub mod lockglue;
pub mod locks;
pub mod projection;
pub mod watcher;

use import::{evaluate_import_gate, import_folder, migrate_history_behind_gate, GateReport, ImportResult};
use locks::{derive_statuses, foreign_locks, ArtifactSignal, LockInfo};
use projection::{project_product, ProductView};
use std::path::Path;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use watcher::{watch_product, WatchHandle};

/// Open a product folder read-only and project it for the UI Shell.
/// No commits, no pushes, no locks — pure read path (Issue #2).
#[tauri::command]
fn open_product(path: String) -> Result<ProductView, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    project_product(root).map_err(|e| e.to_string())
}

/// Holds the active watcher so it lives as long as we are watching a product.
#[derive(Default)]
struct WatcherState(Mutex<Option<WatchHandle>>);

/// Begin silently watching the product folder for settled saves (Issue #4). Each settled
/// save produces a new **Stand**, emitted to the UI as a `stand-created` event. Replaces any
/// previous watch. The user is never prompted; no git vocabulary surfaces.
#[tauri::command]
fn start_watching(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }

    let emit_app = app.clone();
    let handle = watch_product(root, move |stand| {
        // The only thing that leaves the auto-commit layer: a new Stand. No "commit".
        let _ = emit_app.emit("stand-created", stand);
    })
    .map_err(|e| e.to_string())?;

    let state = app.state::<WatcherState>();
    *state.0.lock().unwrap() = Some(handle); // dropping the old handle stops the old watch
    Ok(())
}

/// Stop watching the current product folder, if any.
#[tauri::command]
fn stop_watching(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<WatcherState>();
    *state.0.lock().unwrap() = None; // drop -> stop + join
    Ok(())
}

/// Import a chosen folder as a product via the clean, non-destructive path (Issue #3, E38):
/// `git init` if needed (existing repo left as-is), write `.gitattributes` lockable markers
/// from the Mergeability Classifier, make the first commit, then project it for the shell.
#[tauri::command]
fn import_product(path: String) -> Result<ImportResult, String> {
    let root = Path::new(&path);
    import_folder(root).map_err(|e| e.to_string())
}

/// Probe a folder and run the pure Import Gate (Issue #7, E38). No mutation: returns the one
/// decision (clean-init | migrate-behind-gate | refuse) plus the facts it rests on, so the UI
/// can explain the stakes before any history is touched.
#[tauri::command]
fn evaluate_gate(path: String) -> Result<GateReport, String> {
    let root = Path::new(&path);
    evaluate_import_gate(root).map_err(|e| e.to_string())
}

/// Run the destructive `git lfs migrate` history rewrite — only reachable after the user
/// crosses the "Historie anfassen" gate in the UI, and only honoured when the live repo still
/// decides `migrate-behind-gate` (re-checked server-side; a shared repo is always refused).
#[tauri::command]
fn migrate_history(path: String) -> Result<ImportResult, String> {
    let root = Path::new(&path);
    migrate_history_behind_gate(root).map_err(|e| e.to_string())
}

/// Auto-acquire a `git lfs lock` for a lockable artifact being opened/edited (Issue #6, E31).
/// Mergeable-text paths are a no-op (returns `false`); lockable paths get locked (`true`).
/// The path is product-relative with forward slashes.
#[tauri::command]
fn lock_artifact(product: String, path: String) -> Result<bool, String> {
    let root = Path::new(&product);
    lockglue::acquire_lock(root, &path).map_err(|e| e.to_string())
}

/// The Status Reader (Issue #6): read `git lfs locks` + worktree status purely once, then
/// derive the per-artifact LED status (green/grey/orange) for the given product-relative paths.
/// No second source of truth — every call reads git back (E37).
#[tauri::command]
fn read_status(product: String, paths: Vec<String>) -> Result<Vec<ArtifactSignal>, String> {
    let root = Path::new(&product);
    let snap = lockglue::snapshot(root).map_err(|e| e.to_string())?;
    Ok(derive_statuses(&paths, &snap))
}

/// The live "fremde Sperren" panel (Issue #6, E37): the locks held by anyone but us, read
/// purely from `git lfs locks`. No presence service.
#[tauri::command]
fn read_foreign_locks(product: String) -> Result<Vec<ForeignLock>, String> {
    let root = Path::new(&product);
    let snap = lockglue::snapshot(root).map_err(|e| e.to_string())?;
    Ok(foreign_locks(&snap).into_iter().map(ForeignLock::from).collect())
}

/// A foreign lock as sent to the UI (serializable view of [`LockInfo`] plus the ready tooltip).
#[derive(serde::Serialize)]
struct ForeignLock {
    path: String,
    owner: String,
    locked_at: String,
    tooltip: String,
}

impl From<LockInfo> for ForeignLock {
    fn from(l: LockInfo) -> Self {
        let tooltip = locks::lock_tooltip(&l.owner, &l.locked_at);
        ForeignLock {
            path: l.path,
            owner: l.owner,
            locked_at: l.locked_at,
            tooltip,
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(WatcherState::default())
        .invoke_handler(tauri::generate_handler![
            open_product,
            start_watching,
            stop_watching,
            import_product,
            evaluate_gate,
            migrate_history,
            lock_artifact,
            read_status,
            read_foreign_locks
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
