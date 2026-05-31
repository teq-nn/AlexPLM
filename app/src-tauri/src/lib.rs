pub mod askpass;
pub mod autocommit;
pub mod classifier;
pub mod credentials;
pub mod edges;
pub mod edgestore;
pub mod forgejo;
pub mod gitrunner;
pub mod graph;
pub mod graphread;
pub mod import;
pub mod import_gate;
pub mod lockglue;
pub mod locks;
pub mod projection;
pub mod pushglue;
pub mod registry;
pub mod search;
pub mod setup;
pub mod syncdecider;
pub mod syncglue;
pub mod warden;
pub mod watcher;

use edgestore::{add_persisted_edge, read_edge_view, remove_persisted_edge, EdgeView};
use graph::VersionGraph;
use graphread::{promote_to_milestone, read_graph};
use import::{evaluate_import_gate, import_folder, migrate_history_behind_gate, GateReport, ImportResult};
use locks::{derive_statuses, foreign_locks, ArtifactSignal, LockInfo};
use projection::{project_product, ProductView};
use registry::{add_registered, read_registry, registry_path, remove_registered, RegisteredProduct};
use search::{fan_out, SearchResult};
use setup::{configure_remote, publish_product, read_setup, SetupReport};
use syncglue::{run_sync, SyncOutcome};
use warden::{Checkpoint, WardenAction};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::{Emitter, Manager};
use watcher::{watch_product, WatchHandle};

/// A typed error for the auth-bearing ceremony commands (Issue #22). The `code` lets the frontend
/// react precisely — `"auth"` reopens the credential field, `"keystore"` reports the OS keystore is
/// unreachable — while `message` carries the human German text. Serialised to the frontend as
/// `{ code, message }`.
#[derive(serde::Serialize)]
struct AppError {
    code: String,
    message: String,
}

impl AppError {
    /// Classify an `io::Error` (whose message embeds the failed git's stderr, or a keystore error)
    /// into a typed frontend error. The internal keystore marker is stripped from the visible
    /// message so the user never sees it.
    fn from_io(e: std::io::Error) -> Self {
        let raw = e.to_string();
        let code = match gitrunner::classify_failure(&raw) {
            gitrunner::GitFailure::Auth => "auth",
            gitrunner::GitFailure::KeystoreUnavailable => "keystore",
            gitrunner::GitFailure::Other => "error",
        };
        let message = raw
            .replace(gitrunner::KEYSTORE_UNAVAILABLE_MARKER, "")
            .trim()
            .trim_start_matches(':')
            .trim()
            .to_string();
        AppError {
            code: code.to_string(),
            message,
        }
    }
}

/// Run blocking git / I-O work **off the WebView main thread**. Tauri executes a *synchronous*
/// command body on the main thread, so any networked git call — bounded to `NETWORK_TIMEOUT` (20s)
/// by [`gitrunner::output_bounded`] — would otherwise freeze the entire UI for up to that bound. An
/// `async` command runs on the runtime instead, and `spawn_blocking` keeps the blocking git loop
/// (a `try_wait`/`sleep` poll) from tying up an async worker — important because the status (4s) and
/// sync (8s) ticks can overlap a slow networked call. Returns the closure's `Result`, or a German
/// error string if the background task itself was cancelled/panicked.
async fn on_blocking<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("Hintergrund-Task abgebrochen: {e}"))?
}

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
async fn import_product(path: String) -> Result<ImportResult, String> {
    // Off the main thread: `git init` + first commit of a large folder can take seconds, and a
    // synchronous command body would block the WebView (Tauri runs sync commands on the main
    // thread). See `on_blocking`.
    on_blocking(move || {
        let root = Path::new(&path);
        import_folder(root).map_err(|e| e.to_string())
    })
    .await
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
async fn migrate_history(path: String) -> Result<ImportResult, String> {
    // The `git lfs migrate` rewrite is the heaviest operation in the app; never on the main thread.
    on_blocking(move || {
        let root = Path::new(&path);
        migrate_history_behind_gate(root).map_err(|e| e.to_string())
    })
    .await
}

/// Auto-acquire a `git lfs lock` for a lockable artifact being opened/edited (Issue #6, E31).
/// Mergeable-text paths are a no-op (returns `false`); lockable paths get locked (`true`).
/// The path is product-relative with forward slashes.
#[tauri::command]
async fn lock_artifact(product: String, path: String) -> Result<bool, String> {
    // `git lfs lock` is a networked call (bounded by `output_bounded`); keep it off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        lockglue::acquire_lock(root, &path).map_err(|e| e.to_string())
    })
    .await
}

/// The Status Reader (Issue #6): read `git lfs locks` + worktree status purely once, then
/// derive the per-artifact LED status (green/grey/orange) for the given product-relative paths.
/// No second source of truth — every call reads git back (E37).
#[tauri::command]
async fn read_status(product: String, paths: Vec<String>) -> Result<Vec<ArtifactSignal>, String> {
    // `snapshot` reads `git lfs locks` (networked, bounded); off the main thread so the 4s status
    // tick can never freeze the UI.
    on_blocking(move || {
        let root = Path::new(&product);
        let snap = lockglue::snapshot(root).map_err(|e| e.to_string())?;
        Ok(derive_statuses(&paths, &snap))
    })
    .await
}

/// The live "fremde Sperren" panel (Issue #6, E37): the locks held by anyone but us, read
/// purely from `git lfs locks`. No presence service.
#[tauri::command]
async fn read_foreign_locks(product: String) -> Result<Vec<ForeignLock>, String> {
    // Same networked `git lfs locks` read as `read_status`; off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        let snap = lockglue::snapshot(root).map_err(|e| e.to_string())?;
        Ok(foreign_locks(&snap).into_iter().map(ForeignLock::from).collect())
    })
    .await
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
/// host/owner/repo/credentials (pure core), store the credentials in the **OS keystore** (Issue
/// #22, never in `.git/config`), configure the git remote with the credential-free URL, and enable
/// `locksverify` for the host. The returned report carries only the credential-free clone URL. A
/// keystore/auth failure surfaces as a typed [`AppError`] so the frontend can reopen the
/// credential field instead of hanging.
#[tauri::command]
async fn connect_server(
    path: String,
    host: String,
    owner: String,
    repo: String,
    user: String,
    token: String,
) -> Result<SetupReport, AppError> {
    // Touches the OS keystore and git config; off the main thread so the ceremony step never freezes
    // the WebView. (Inline `spawn_blocking` rather than `on_blocking` because the error is `AppError`.)
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&path);
        configure_remote(root, &host, &owner, &repo, &user, &token).map_err(AppError::from_io)
    })
    .await
    .map_err(|e| AppError { code: "error".to_string(), message: format!("Hintergrund-Task abgebrochen: {e}") })?
}

/// Perform the **first push** that publishes the product to the connected server (Issue #5,
/// E41), setting the upstream so the later silent daily sync has a tracking ref. Returns the
/// refreshed ceremony state (now `eingerichtet`).
#[tauri::command]
async fn publish_to_server(path: String) -> Result<SetupReport, AppError> {
    // The first publish push is networked (bounded to 20s on a bad credential); off the main thread
    // so this exact ceremony step can no longer freeze the WebView while it runs.
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&path);
        publish_product(root).map_err(AppError::from_io)
    })
    .await
    .map_err(|e| AppError { code: "error".to_string(), message: format!("Hintergrund-Task abgebrochen: {e}") })?
}

/// The Lock Warden checkpoint (Issue #9, E35): at a checkpoint for one artifact, read the world
/// (path kind, lock state, clean/dirty) purely once, let the safety-critical pure Warden decide
/// the single action — `freigabe-push` | `sicherungs-push` | `auto-unlock` | `refuse` — and carry
/// it out. `milestone = true` is a Meilenstein (Freigabe candidate); `false` is a laufender
/// Checkpoint (Sicherungs at most). The action taken is returned in the tool's own vocabulary,
/// never raw git. The Binär-Invariante lives in the pure core: a locked binary change is never
/// published while the lock is held.
#[tauri::command]
async fn run_checkpoint(product: String, path: String, milestone: bool) -> Result<WardenAction, String> {
    // The Warden carries out a push (networked, bounded); off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        let checkpoint = if milestone { Checkpoint::Meilenstein } else { Checkpoint::Laufend };
        pushglue::run_checkpoint(root, &path, checkpoint).map_err(|e| e.to_string())
    })
    .await
}

/// Run one **silent daily sync pass** (Issue #11, E41): fetch the remote stand, let the pure
/// Sync Decider judge the divergence, and carry out the result. Free, mergeable divergence is
/// merged silently with NO prompt (status `gesichert`); a contradiction over an unmergeable file
/// (binary OR KiCad nominal-text — the #3 buckets) STOPS the sync without merging and returns the
/// single **laute Ausnahme** — a domain-language question ("dein und X' Gehäuse-Stand
/// widersprechen sich — welcher gilt?"), never a git conflict marker. The daily vocabulary is
/// "aktuell / X arbeitet an Y / gesichert"; raw git (push/pull/merge) never surfaces.
#[tauri::command]
async fn sync_product(path: String, other: Option<String>) -> Result<SyncOutcome, String> {
    // The silent daily sync does a `fetch` (networked, bounded); off the main thread so the 8s
    // sync tick can never freeze the UI.
    on_blocking(move || {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {path}"));
        }
        run_sync(root, other).map_err(|e| e.to_string())
    })
    .await
}

/// Resolve the app-level Produkt-Registry file under Tauri's app config dir (Issue #45). The
/// registry lives at app level — NOT inside any product — because it is the one list that spans
/// products. A failure to resolve the config dir surfaces as a German error string.
fn resolve_registry_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("App-Konfigurationsordner nicht ermittelbar: {e}"))?;
    Ok(registry_path(&dir))
}

/// List the registered products (Issue #45). Path-only: each entry is a folder path plus its
/// derived display name — no content is cached. A missing/corrupt registry reads as empty.
#[tauri::command]
fn list_products(app: tauri::AppHandle) -> Result<Vec<RegisteredProduct>, String> {
    let file = resolve_registry_file(&app)?;
    Ok(read_registry(&file))
}

/// Register a product folder into the app-level Produkt-Registry (Issue #45). Stores ONLY the
/// path (de-duplicated, normalized); the content is never copied. Returns the refreshed list.
#[tauri::command]
fn register_product(app: tauri::AppHandle, path: String) -> Result<Vec<RegisteredProduct>, String> {
    let file = resolve_registry_file(&app)?;
    add_registered(&file, &path).map_err(|e| e.to_string())
}

/// Remove a product from the Produkt-Registry (Issue #45). Drops only the registry entry; the
/// product folder on disk is never touched. Returns the refreshed list.
#[tauri::command]
fn unregister_product(
    app: tauri::AppHandle,
    path: String,
) -> Result<Vec<RegisteredProduct>, String> {
    let file = resolve_registry_file(&app)?;
    remove_registered(&file, &path).map_err(|e| e.to_string())
}

/// The produktübergreifende Live-Suche (Issue #45, E45): a live Fan-out over the registry —
/// opens each reachable product and greps live over Dateinamen, `_plm` and `VERSION_NOTES.md`.
/// No central index, no mirror. Unreachable products are reported honestly in the result's
/// `offline` list with searched/total counts — never silently dropped.
#[tauri::command]
async fn search_products(app: tauri::AppHandle, query: String) -> Result<SearchResult, String> {
    // The fan-out walks N product trees off disk; keep it off the WebView main thread so a slow /
    // large registry can never freeze the UI (same reason as the git commands above).
    let file = resolve_registry_file(&app)?;
    on_blocking(move || {
        let registry = read_registry(&file);
        Ok(fan_out(&registry, &query))
    })
    .await
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
            publish_to_server,
            run_checkpoint,
            sync_product,
            list_products,
            register_product,
            unregister_product,
            search_products
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
