pub mod autocommit;
pub mod classifier;
pub mod edges;
pub mod edgestore;
pub mod graph;
pub mod graphread;
pub mod import;
pub mod import_gate;
pub mod lockglue;
pub mod locks;
pub mod projection;
pub mod setup;
pub mod watcher;

use edgestore::{add_persisted_edge, read_edge_view, remove_persisted_edge, EdgeView};
use graph::VersionGraph;
use graphread::{promote_to_milestone, read_graph};
use import::{evaluate_import_gate, import_folder, migrate_history_behind_gate, GateReport, ImportResult};
use locks::{derive_statuses, foreign_locks, ArtifactSignal, LockInfo};
use projection::{project_product, ProductView};
use setup::{configure_remote, publish_product, read_setup, SetupReport};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;
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

/// Read the product's version tree for the dark "display" zone (Issue #8): Stände as nodes,
/// Meilensteine marked, offloaded markers reserved. Pure read — the git/LFS facts are
/// collected then projected; no mutation.
#[tauri::command]
fn read_version_graph(path: String) -> Result<VersionGraph, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    read_graph(root).map_err(|e| e.to_string())
}

/// Promote a Stand to a **Meilenstein** (Issue #8): persist the human `notes` into
/// `VERSION_NOTES.md` (the only place human text lives — E28) and durably label the version.
/// Returns the refreshed version tree so the UI updates in one round-trip.
#[tauri::command]
fn promote_milestone(
    path: String,
    stand_id: String,
    version: String,
    notes: String,
) -> Result<VersionGraph, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    promote_to_milestone(root, &stand_id, &version, &notes, SystemTime::now())
        .map_err(|e| e.to_string())
}

/// Read the product's manual „abgeleitet von" edges and their Stale-Warnungen (Issue #10).
/// Edges are opt-in: a product with no edge file has zero edges and no warnings (E40). Pure
/// read — the edges + artifact timestamps are gathered then judged by the pure core.
#[tauri::command]
fn read_edges(path: String) -> Result<EdgeView, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    Ok(read_edge_view(root))
}

/// Draw a manual „abgeleitet von" edge (`derived` „stammt aus" `source`) and persist it
/// (Issue #10). Returns the refreshed edge view (edges + Stale-Warnungen) so the UI updates
/// in one round-trip.
#[tauri::command]
fn add_edge(path: String, derived: String, source: String) -> Result<EdgeView, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    add_persisted_edge(root, &derived, &source).map_err(|e| e.to_string())?;
    Ok(read_edge_view(root))
}

/// Remove a manual edge and persist the result (Issue #10). Returns the refreshed edge view.
#[tauri::command]
fn remove_edge(path: String, derived: String, source: String) -> Result<EdgeView, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    remove_persisted_edge(root, &derived, &source).map_err(|e| e.to_string())?;
    Ok(read_edge_view(root))
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

/// Read the one-time **Einrichtungs-Zeremonie** state for a product (Issue #5, E41): is a
/// server connected, has the product been published, and the colleague's credential-free clone
/// URL. Pure read — the UI shows the ceremony only when not yet configured, a settled
/// "eingerichtet" readout otherwise. This ceremony is the rare, explicit exception where
/// git-near wording is allowed; the daily sync stays silent.
#[tauri::command]
fn read_setup_state(path: String) -> Result<SetupReport, String> {
    let root = Path::new(&path);
    read_setup(root).map_err(|e| e.to_string())
}

/// Connect the self-hosted Forgejo/Gitea server (Issue #5, E41): validate + normalize the typed
/// host/owner/repo/credentials (pure core), configure the git remote, and enable `locksverify`
/// for the host. Credentials are embedded in the push URL but never echoed back — the returned
/// report carries only the credential-free clone URL. Returns the refreshed ceremony state.
#[tauri::command]
fn connect_server(
    path: String,
    host: String,
    owner: String,
    repo: String,
    user: String,
    token: String,
) -> Result<SetupReport, String> {
    let root = Path::new(&path);
    configure_remote(root, &host, &owner, &repo, &user, &token).map_err(|e| e.to_string())
}

/// Perform the **first push** that publishes the product to the connected server (Issue #5,
/// E41), setting the upstream so the later silent daily sync has a tracking ref. Returns the
/// refreshed ceremony state (now `eingerichtet`).
#[tauri::command]
fn publish_to_server(path: String) -> Result<SetupReport, String> {
    let root = Path::new(&path);
    publish_product(root).map_err(|e| e.to_string())
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
            read_version_graph,
            promote_milestone,
            read_edges,
            add_edge,
            remove_edge,
            import_product,
            evaluate_gate,
            migrate_history,
            lock_artifact,
            read_status,
            read_foreign_locks,
            read_setup_state,
            connect_server,
            publish_to_server
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
