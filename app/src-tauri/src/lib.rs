pub mod artstore;
pub mod askpass;
pub mod aufgabenblock;
pub mod aufgabenblockglue;
pub mod autocommit;
pub mod autolock;
pub mod baustein;
pub mod bibliothek;
pub mod classifier;
pub mod credentials;
pub mod defaultkanten;
pub mod edges;
pub mod edgestore;
pub mod forgejo;
pub mod freigabegate;
pub mod freigabegateglue;
pub mod gitlog;
pub mod gitrunner;
pub mod graph;
pub mod graphread;
pub mod import;
pub mod import_gate;
pub mod kartenstatus;
pub mod knotenverben;
pub mod konto;
pub mod lockglue;
pub mod locks;
pub mod markerblock;
pub mod onboardglue;
pub mod projection;
pub mod pushglue;
pub mod registry;
pub mod search;
pub mod setup;
pub mod stackstore;
pub mod stilllegen;
pub mod syncdecider;
pub mod syncglue;
pub mod taskstore;
pub mod tasks;
pub mod warden;
pub mod watcher;
pub mod werkbank;
pub mod worktreeglue;
pub mod zuordnung;
pub mod zuordnungstore;

use aufgabenblock::BlockDecision;
use aufgabenblockglue::block_for_art;
use baustein::{Baustein, Toolstack};
use bibliothek::Bibliothek;
use edgestore::{
    add_persisted_edge, confirm_pair_edge, onboard_default_edges, read_edge_view,
    remove_persisted_edge, EdgeView,
};
use freigabegate::GateVerdict;
use freigabegateglue::gate_for_art;
use graph::{RevisionArt, VersionGraph};
use graphread::{promote_to_revision, read_graph, toggle_revision_freigabe};
use konto::{
    clear_konto as clear_konto_file, konto_path, read_konto as read_konto_file, write_konto,
    KontoConfig, KontoView,
};
use import::{evaluate_import_gate, import_folder, migrate_history_behind_gate, GateReport, ImportResult};
use locks::{derive_statuses, foreign_locks, ArtifactSignal, LockInfo};
use projection::{project_product, ProductView};
use registry::{
    add_registered, read_registry, registry_path, relink_registered, remove_registered,
    RegisteredProduct,
};
use search::{fan_out, SearchResult};
use setup::{configure_remote, publish_product, read_setup, PublishOutcome, SetupReport};
use stackstore::{create_product_stack, extend_product_stack, read_stack, ProduktStack};
use syncdecider::StandChoice;
use syncglue::{resolve_sync, run_sync, SyncOutcome};
use taskstore::{create_task, delete_task, read_tasks, set_task_status, update_task};
use tasks::{NewTask, Task, TaskEdit, TaskKind, TaskLink, TaskStatus};
use warden::{Checkpoint, WardenAction};
use werkbank::{read_werkbank, WerkbankView};
use worktreeglue::{als_ordner_oeffnen, von_hier_abzweigen, zurueckwerfen, GeoeffneterOrdner};
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

    /// Classify a **publish** failure (Issue #44) like [`AppError::from_io`] for the typed `code`
    /// — so the ceremony still routes an `auth`/`keystore` failure back to the credential step —
    /// but **replace the message with the tool's own vocabulary**. The raw git/LFS rejection
    /// (`master -> master (fetch first)`, push/pull hints, the `locksverify` notice) must never
    /// reach the user; integrating a non-empty Server-Repo avoids the rejection in the first place,
    /// and any residual failure collapses to one neutral domain sentence here.
    fn publish_failure(e: std::io::Error) -> Self {
        let (code, message) = match gitrunner::classify_failure(&e.to_string()) {
            gitrunner::GitFailure::Auth => (
                "auth",
                "Der Server hat die Zugangsdaten abgelehnt — bitte Zugangs-Token prüfen.",
            ),
            gitrunner::GitFailure::KeystoreUnavailable => (
                "keystore",
                "Der sichere Schlüsselspeicher ist nicht erreichbar — bitte erneut anmelden.",
            ),
            gitrunner::GitFailure::Other => (
                "error",
                "Veröffentlichen ließ sich nicht abschließen — bitte erneut versuchen.",
            ),
        };
        AppError { code: code.to_string(), message: message.to_string() }
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

/// Begin silently watching the product folder for settled saves (Issue #4) and watcher-side
/// Auto-Lock (Issue #42). Each settled save produces a new **Stand**, emitted as a `stand-created`
/// event; the first save to a lockable path auto-acquires its lock (closing the
/// Binär-Invarianten-Fenster before any checkpoint) and emits a `lock-acquired` event carrying the
/// product-relative path so the UI re-reads the LED signal. Replaces any previous watch. The user
/// is never prompted; no git vocabulary surfaces.
#[tauri::command]
fn start_watching(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }

    let stand_app = app.clone();
    let lock_app = app.clone();
    let handle = watch_product(
        root,
        move |stand| {
            // The only thing that leaves the auto-commit layer: a new Stand. No "commit".
            let _ = stand_app.emit("stand-created", stand);
        },
        move |locked_path| {
            // The watcher took a lock on the first dirty lockable path — tell the UI to re-read
            // the per-artifact LED signals so the card reflects it. Just the path; no git word.
            let _ = lock_app.emit("lock-acquired", locked_path);
        },
    )
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
/// Revisionen marked, offloaded markers reserved. Pure read — the git/LFS facts are
/// collected then projected; no mutation.
#[tauri::command]
fn read_version_graph(path: String) -> Result<VersionGraph, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    read_graph(root).map_err(|e| e.to_string())
}

/// Promote a Stand to a **Revision** (Issue #8): persist the human `notes` into
/// `VERSION_NOTES.md` (the only place human text lives — E28) and durably label the version.
/// Returns the refreshed version tree so the UI updates in one round-trip.
#[tauri::command]
fn promote_revision(
    path: String,
    stand_id: String,
    version: String,
    notes: String,
) -> Result<VersionGraph, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    promote_to_revision(root, &stand_id, &version, &notes, SystemTime::now())
        .map_err(|e| e.to_string())
}

/// Toggle a Revision's **Art** between Prototyp and Freigabe (Issue #41, E42). Raising to
/// Freigabe write-protects the tag (E8); toggling back is the deliberate reversible
/// „Un-Release". A new Revision is Prototyp by default; this is the one act that releases
/// it. Returns the refreshed version tree so the UI updates in one round-trip.
///
/// The dreistufige Freigabe-Gate block-check that will run on raising to Freigabe is a
/// separate slice (Issue #52); its seam lives in [`toggle_revision_freigabe`].
#[tauri::command]
fn toggle_revision_art(path: String, version: String) -> Result<VersionGraph, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    toggle_revision_freigabe(root, &version).map_err(|e| e.to_string())
}

/// **Als Ordner öffnen** (Issue #55, E27/E3 — Default-Knoten-Verb des Graph-Raums): materialisiert
/// den Stand `stand_id` als *separaten, schreibgeschützten* Ordner neben dem Produkt (ein detached
/// `git worktree`). Die Werkbank (Jetzt-Zustand) bleibt unberührt — ein Klick auf einen alten Knoten
/// bewegt sie nie still. Idempotent: ein schon materialisierter Ordner wird nur zurückgegeben. Die
/// UI übergibt den zurückgegebenen Pfad dem OS zum Öffnen.
#[tauri::command]
async fn knoten_als_ordner(
    path: String,
    stand_id: String,
    label: String,
) -> Result<GeoeffneterOrdner, String> {
    on_blocking(move || {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {path}"));
        }
        als_ordner_oeffnen(root, &stand_id, &label).map_err(|e| e.to_string())
    })
    .await
}

/// **Von hier abzweigen** (Issue #55, E27/E8/E43): ein bewusster neuer Branch ab dem Stand
/// `stand_id`. Bevor die Werkbank bewegt wird, sichert das Werkzeug jede laufende Arbeit als
/// gewöhnlichen Stand (E8) — kein `stash`, nichts geht verloren —, dann `checkout -b`. Gibt den
/// frisch projizierten Versionsbaum zurück, damit die neue Linie sofort erscheint.
#[tauri::command]
async fn knoten_abzweigen(
    path: String,
    stand_id: String,
    branch: String,
) -> Result<VersionGraph, String> {
    on_blocking(move || {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {path}"));
        }
        von_hier_abzweigen(root, &stand_id, &branch, SystemTime::now()).map_err(|e| e.to_string())
    })
    .await
}

/// **Zurückwerfen** (Issue #55, E27 — destruktiv, hinter der schwarzen „Historie anfassen"-Gate,
/// nie der Default): springt auf den alten Stand `stand_id`, aber **sicher** — keine versteckte
/// `reset --hard`/`rebase`-Mechanik (E43). Es sichert erst die laufende Arbeit (E8), holt dann den
/// alten Inhalt in die Werkbank und schreibt ihn als **neuen, vorwärts gerichteten Stand** fest
/// („behalten, nie umschreiben", E9 — voll reversibel). Gibt den frischen Versionsbaum zurück.
#[tauri::command]
async fn knoten_zurueckwerfen(path: String, stand_id: String) -> Result<VersionGraph, String> {
    on_blocking(move || {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {path}"));
        }
        zurueckwerfen(root, &stand_id, SystemTime::now()).map_err(|e| e.to_string())
    })
    .await
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

/// Einen **Baustein-Paar-Default-Vorschlag bestätigen** (Issue #56, E20): legt eine Kante mit
/// Herkunft `PaarDefault` zwischen `derived` und `source` an und persistiert sie. Vorschläge werden
/// nie automatisch angelegt — erst dieser Klick bestätigt sie. Gibt die frische Kantensicht zurück
/// (samt verbleibender Vorschläge und Stale-Warnungen).
#[tauri::command]
fn confirm_pair_edge_cmd(path: String, derived: String, source: String) -> Result<EdgeView, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    confirm_pair_edge(root, &derived, &source).map_err(|e| e.to_string())?;
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

/// The live "Belegte Bausteine" panel (Issue #6, E37): the locks held by anyone but us, read
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

/// Connect the self-hosted Forgejo/Gitea server (Issue #5, E41; credential-free since ADR 0004 /
/// Issue #92). Draws everything server-related from the app-wide **Konto**: the Base-URL and the
/// owner-default (the Konto username) come from `konto::read_konto`, and the frontend supplies only
/// the optional `owner` (Besitzer/Team) + `repo` (Produkt-Name). Configures the git remote with the
/// credential-free clone URL and enables `locksverify`; writes **no** credentials (the Konto is the
/// sole writer of those, host-keyed in the OS keystore at Konto-save time). With **no Konto set** it
/// refuses with a clear typed `error` so the frontend points the user at the Konto panel instead of
/// asking for credentials. The returned report carries only the credential-free clone URL.
#[tauri::command]
async fn connect_server(
    app: tauri::AppHandle,
    path: String,
    owner: String,
    repo: String,
) -> Result<SetupReport, AppError> {
    // Resolve the app-wide Konto file before going off-thread; a missing config dir is a plain error.
    let konto_file = resolve_konto_file(&app).map_err(|message| AppError {
        code: "error".to_string(),
        message,
    })?;
    // Configures git config; off the main thread so the ceremony step never freezes the WebView.
    // (Inline `spawn_blocking` rather than `on_blocking` because the error is `AppError`.)
    tauri::async_runtime::spawn_blocking(move || {
        // The Konto is the single source for the server address + owner-default. No Konto → refuse
        // with a clear domain message; the frontend opens the Konto panel instead of a credential
        // field (ADR 0004: "global login, not per repo").
        let Some(konto) = read_konto_file(&konto_file) else {
            return Err(AppError {
                code: "error".to_string(),
                message: "Kein Konto eingerichtet — bitte zuerst im Konto den Server anmelden."
                    .to_string(),
            });
        };
        let root = Path::new(&path);
        configure_remote(root, &konto.base_url, &owner, &repo, &konto.account)
            .map_err(AppError::from_io)
    })
    .await
    .map_err(|e| AppError { code: "error".to_string(), message: format!("Hintergrund-Task abgebrochen: {e}") })?
}

/// Perform the **first push** that publishes the product to the connected server (Issue #5,
/// E41), setting the upstream so the later silent daily sync has a tracking ref. Returns the
/// refreshed ceremony state (now `eingerichtet`).
#[tauri::command]
async fn publish_to_server(path: String, other: Option<String>) -> Result<PublishOutcome, AppError> {
    // The first publish push is networked (bounded to 20s on a bad credential); off the main thread
    // so this exact ceremony step can no longer freeze the WebView while it runs. A non-empty
    // Server-Repo is integrated first (Issue #44); a real contradiction returns `LauteAusnahme`
    // instead of failing, and any genuine failure speaks the tool's own vocabulary (no raw git text).
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&path);
        publish_product(root, other).map_err(AppError::publish_failure)
    })
    .await
    .map_err(|e| AppError { code: "error".to_string(), message: format!("Hintergrund-Task abgebrochen: {e}") })?
}

/// The Lock Warden checkpoint (Issue #9, E35): at a checkpoint for one artifact, read the world
/// (path kind, lock state, clean/dirty) purely once, let the safety-critical pure Warden decide
/// the single action — `freigabe-push` | `sicherungs-push` | `auto-unlock` | `refuse` — and carry
/// it out. `revision = true` is a Revision (Freigabe candidate); `false` is a laufender
/// Checkpoint (Sicherungs at most). The action taken is returned in the tool's own vocabulary,
/// never raw git. The Binär-Invariante lives in the pure core: a locked binary change is never
/// published while the lock is held.
#[tauri::command]
async fn run_checkpoint(product: String, path: String, revision: bool) -> Result<WardenAction, String> {
    // The Warden carries out a push (networked, bounded); off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        let checkpoint = if revision { Checkpoint::Revision } else { Checkpoint::Laufend };
        pushglue::run_checkpoint(root, &path, checkpoint).map_err(|e| e.to_string())
    })
    .await
}

/// **Freigeben** (Issue #54-Folge): the explicit „ich bin fertig"-act of a Revision. Publishes
/// the whole current branch to the *actually shared* branch of the remote (Issue #64) and then
/// self-heals — auto-unlocks every held-clean binary now published ("unlock at push", E35). This
/// replaces the per-path Revision checkpoint for the publish: at revision time the work is
/// already committed, so the per-path Warden always saw a clean path and `Refuse`d, leaving the
/// branch never pushed to the shared stand. Returns `freigabe-push` so the readout lights
/// „freigegeben"; the per-path Warden stays unchanged for the silent laufend backup rhythm.
#[tauri::command]
async fn freigeben(product: String) -> Result<WardenAction, String> {
    // Pushes the branch + releases locks (networked, bounded); off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        pushglue::publish_branch(root).map_err(|e| e.to_string())?;
        // Unlock-at-push for the revision: release every held-clean binary now on the shared stand.
        let _ = lockglue::auto_unlock_clean_paths(root);
        Ok(WardenAction::FreigabePush)
    })
    .await
}

/// **Sichern** (Issue #54): the visible, manual **Sicherungs-Push** — a personal backup of the
/// current branch into the personal namespace `refs/personal/<user>/<branch>` on the remote. This
/// is the explicit press of the toolbar's „Sichern"-Knopf: a private backup (incl. half-finished
/// binaries under an active lock) that, by construction of the ref, can **NEVER** reach the shared
/// `main`. It does not release any lock — backup yes, Freigabe no (E35). Returns `sicherungs-push`
/// so the Sicherungsstatus readout lights „gesichert". Distinct from the silent laufend rhythm: the
/// daily Auto-Commit stays quiet; this is the user's deliberate, visible backup gesture.
#[tauri::command]
async fn sichern(product: String) -> Result<WardenAction, String> {
    // The Sicherungs-Push reaches the network (bounded); off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {product}"));
        }
        pushglue::sicherungs_push(root).map_err(|e| e.to_string())?;
        Ok(WardenAction::SicherungsPush)
    })
    .await
}

/// At a checkpoint, **auto-unlock every held lock whose path is locally clean** (Issue #42,
/// E31/E35 self-healing). Reuses the pure Lock Warden decision per held lock — the lock policy is
/// decided in one place, never duplicated. Returns the product-relative paths that were freed so
/// the UI can re-read the LED signals; a freed binary rests read-only (frei) again. Best-effort:
/// an LFS/network hiccup must never break the silent rhythm.
#[tauri::command]
async fn sweep_clean_locks(product: String) -> Result<Vec<String>, String> {
    // Reads `git lfs locks` + status and may release locks (networked, bounded); off the main thread.
    on_blocking(move || {
        let root = Path::new(&product);
        let swept = lockglue::auto_unlock_clean_paths(root).map_err(|e| e.to_string())?;
        Ok(swept.into_iter().filter(|s| s.released).map(|s| s.path).collect())
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

/// The Bibliothek root for this install: `<app-data>/plm-werkzeug/bibliothek` (ADR 0003).
fn bibliothek_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("App-Data-Verzeichnis nicht auffindbar: {e}"))?;
    Ok(data.join("plm-werkzeug").join("bibliothek"))
}

/// The bundled default Bibliothek shipped as a Tauri resource (`resources/bibliothek`).
fn bundled_bibliothek_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let res = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Ressourcen-Verzeichnis nicht auffindbar: {e}"))?;
    Ok(res.join("resources").join("bibliothek"))
}

/// Das gebündelte portable git (MinGit) als Tauri-Resource: `resources/git/cmd/git.exe`. Nur unter
/// Windows relevant — dort wird git/git-lfs mitgeliefert, statt es als System-Voraussetzung zu
/// fordern. Auflösung analog [`bundled_bibliothek_dir`] über das Ressourcen-Verzeichnis.
#[cfg(windows)]
fn bundled_git_exe(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let res = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Ressourcen-Verzeichnis nicht auffindbar: {e}"))?;
    Ok(res
        .join("resources")
        .join("git")
        .join("cmd")
        .join("git.exe"))
}

/// Verdrahte unter Windows das gebündelte git in den [`gitrunner`], damit alle Produktions-Spawns
/// das mitgelieferte git/git-lfs nutzen statt einer System-Installation. Best-effort wie das
/// Bibliothek-Seeding: existiert die `git.exe` nicht (z. B. unvollständiges Bundle), bleibt der
/// PATH-Fallback `"git"` aktiv und die App startet trotzdem — die Bedingung nur auf stderr melden.
#[cfg(windows)]
fn wire_bundled_git(app: &tauri::AppHandle) {
    match bundled_git_exe(app) {
        Ok(exe) if exe.is_file() => gitrunner::set_git_program(exe),
        Ok(exe) => eprintln!(
            "Gebündeltes git nicht gefunden ({}); Fallback auf System-git im PATH.",
            exe.display()
        ),
        Err(e) => eprintln!("Gebündeltes git nicht auflösbar: {e}; Fallback auf System-git im PATH."),
    }
}

/// Run the idempotent, version-gated seeding of the bundled default Bausteine/Toolstacks into the
/// local Bibliothek (ADR 0003). Runs on first start and after every app update; never touches
/// user-edited or user-added Bausteine, and never touches product copies (anti-drift).
fn seed_bibliothek(app: &tauri::AppHandle) -> Result<(), String> {
    let lib = Bibliothek::new(bibliothek_root(app)?);
    let (bausteine, toolstacks) = bibliothek::load_bundled(&bundled_bibliothek_dir(app)?);
    lib.seed_from(&bausteine, &toolstacks)
        .map_err(|e| format!("Bibliothek-Seeding fehlgeschlagen: {e}"))?;
    Ok(())
}

/// List the local Bibliothek (Issue #39, ADR 0003): the seeded + user-added Bausteine and the
/// available Toolstacks. Pure read; missing/corrupt entries degrade to an empty list.
#[tauri::command]
fn list_bibliothek(app: tauri::AppHandle) -> Result<BibliothekView, String> {
    let lib = Bibliothek::new(bibliothek_root(&app)?);
    Ok(BibliothekView {
        bausteine: lib.list_bausteine(),
        toolstacks: lib.list_toolstacks(),
    })
}

/// The Bibliothek as sent to the UI: the available Bausteine and Toolstacks.
#[derive(serde::Serialize)]
struct BibliothekView {
    bausteine: Vec<Baustein>,
    toolstacks: Vec<Toolstack>,
}

/// List the available standard Toolstacks from the Bibliothek (Issue #39). Convenience read for
/// the „Standard-Toolstack wählen"-Schritt der Produkt-Anlage (#50).
#[tauri::command]
fn list_toolstacks(app: tauri::AppHandle) -> Result<Vec<Toolstack>, String> {
    let lib = Bibliothek::new(bibliothek_root(&app)?);
    Ok(lib.list_toolstacks())
}

/// Resolve a named Toolstack from the Bibliothek to its ordered Baustein-`id`s (Issue #39).
/// Returns an error if the Toolstack is unknown. Used to seed a product stack from a chosen stack.
#[tauri::command]
fn toolstack_baustein_ids(app: tauri::AppHandle, toolstack_id: String) -> Result<Vec<String>, String> {
    let lib = Bibliothek::new(bibliothek_root(&app)?);
    lib.read_toolstack(&toolstack_id)
        .map(|t| t.baustein_ids)
        .ok_or_else(|| format!("Unbekannter Toolstack: {toolstack_id}"))
}

/// Create/configure a product's Produkt-Stack as a **full self-contained copy** of the chosen
/// Bibliothek Bausteine, written to `_plm/stack.json` with a provenance stamp (Issue #39, ADR
/// 0003). No live link to the Bibliothek — a later Bibliothek edit never reaches this product
/// (anti-drift). `toolstack` is the optional display name of the chosen standard stack. The
/// product-creation UI is #50; this is the backend the ceremony calls.
#[tauri::command]
fn create_product_stack_cmd(
    app: tauri::AppHandle,
    product: String,
    baustein_ids: Vec<String>,
    toolstack: Option<String>,
) -> Result<ProduktStack, String> {
    let lib = Bibliothek::new(bibliothek_root(&app)?);
    let root = Path::new(&product);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {product}"));
    }
    let stack = create_product_stack(root, &lib, &baustein_ids, toolstack).map_err(|e| e.to_string())?;
    // Tag-1-Pflicht (Issue #48, adressiert #63): idempotente Ignore/LFS-Marker-Blöcke in die
    // Dotfiles hängen, BEVOR ein Tool seine erste Binärdatei/Müll erzeugt — kein späteres lfs migrate.
    onboardglue::onboard_stack_dotfiles(root, &stack).map_err(|e| e.to_string())?;
    // Geführte Anlage (PRD §50/§29): die Heimat-Ordner anlegen, damit der Nutzer sofort sieht, wohin
    // seine Dateien gehören.
    onboardglue::scaffold_heimat_dirs(root, &stack).map_err(|e| e.to_string())?;
    // Baustein-Default-Kanten (Issue #56, E20): die Kanten INNERHALB der Bausteine beim Onboarding
    // automatisch anlegen (idempotent; greift, sobald die Quell-/Ableitungs-Dateien erfasst sind).
    onboard_default_edges(root).map_err(|e| e.to_string())?;
    Ok(stack)
}

/// Extend an existing Produkt-Stack **additively** with further Bibliothek Bausteine (Issue #50,
/// „Tool erweitern" / „später ergänzen"). Already-copied Bausteine are kept verbatim — never re-pulled
/// from the Bibliothek (anti-drift: no silent version bump); only genuinely new `id`s are appended as
/// full copies. The newly onboarded Bausteine get their Tag-1 marker blocks written too (idempotent,
/// Issue #48). Returns the extended stack.
#[tauri::command]
fn extend_product_stack_cmd(
    app: tauri::AppHandle,
    product: String,
    baustein_ids: Vec<String>,
) -> Result<ProduktStack, String> {
    let lib = Bibliothek::new(bibliothek_root(&app)?);
    let root = Path::new(&product);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {product}"));
    }
    let stack = extend_product_stack(root, &lib, &baustein_ids).map_err(|e| e.to_string())?;
    onboardglue::onboard_stack_dotfiles(root, &stack).map_err(|e| e.to_string())?;
    onboardglue::scaffold_heimat_dirs(root, &stack).map_err(|e| e.to_string())?;
    // Default-Kanten der (ggf. neu hinzugekommenen) Bausteine anlegen (Issue #56, E20). Idempotent.
    onboard_default_edges(root).map_err(|e| e.to_string())?;
    Ok(stack)
}

/// Die Antwort des Stilllegens (Issue #51): die label-only-**Wirkung** (welche Globs erlöschen,
/// welche Dateien zu Waisen werden, welches Sediment liegen bleibt) **plus** die frisch berechnete
/// Werkbank, sodass die UI die neuen Waisen im Unzugeordnet-Fach sofort sieht.
#[derive(serde::Serialize)]
struct StilllegenResult {
    wirkung: stilllegen::StilllegenWirkung,
    stack: ProduktStack,
    werkbank: WerkbankView,
}

/// Einen Baustein eines Produkts **stilllegen** bzw. reaktivieren (Issue #51, E17). Label-only und
/// (fast) umkehrbar: setzt nur den `stillgelegt`-Schalter in `_plm/stack.json`. Die alten Globs
/// hören dadurch auf zu greifen → ihre Dateien werden zu **Waisen** im Unzugeordnet-Fach; die
/// Ignore-/LFS-Marker-Blöcke bleiben als **Sediment** unangetastet in den Dotfiles liegen; **nichts
/// wird verschoben oder gelöscht**. Gibt die Wirkung + den neuen Stack + die frisch gefaltete
/// Werkbank zurück. Eine unbekannte `id` ist eine no-op (kein Fehler).
#[tauri::command]
async fn stilllegen_baustein_cmd(
    product: String,
    baustein_id: String,
    stillgelegt: bool,
) -> Result<StilllegenResult, String> {
    on_blocking(move || {
        let root = Path::new(&product);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {product}"));
        }
        // Die Wirkung VOR dem Schreiben aus dem aktuellen Stand berechnen (reiner Kern), damit die
        // erloschenen Globs/neuen Waisen unabhängig von der Persistenz prüfbar sind.
        let stack_vorher = read_stack(root);
        let tracked = werkbank::list_tracked_files(root).map_err(|e| e.to_string())?;
        let wirkung = if stillgelegt {
            stilllegen::berechne_wirkung(&stack_vorher, &baustein_id, &tracked)
        } else {
            // Reaktivieren hat keine Stilllege-Wirkung; eine leere, neutrale Wirkung genügt der UI.
            stilllegen::StilllegenWirkung { nichts_bewegt: true, ..Default::default() }
        };
        let stack =
            stackstore::stilllegen_baustein(root, &baustein_id, stillgelegt).map_err(|e| e.to_string())?;
        let werkbank = read_werkbank(root).map_err(|e| e.to_string())?;
        Ok(StilllegenResult { wirkung, stack, werkbank })
    })
    .await
}

/// Read a product's copied Produkt-Stack from `_plm/stack.json` (Issue #39). Pure read; a product
/// with no stack file reads as an empty stack (never an error). This is the anti-drift copy — it
/// reflects only what was copied in, never the live Bibliothek.
#[tauri::command]
fn read_product_stack(product: String) -> Result<ProduktStack, String> {
    let root = Path::new(&product);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {product}"));
    }
    Ok(read_stack(root))
}

/// Read the product's **Werkbank** (Issue #47): turn tracked files into Artefakt-Karten by
/// convention via the pure Pattern-Zuordnung core, and gather the unlabeled tracked files into an
/// **Unzugeordnet-Fach pro Arbeitsbereich**. Each card carries its Hauptdatei (highest glob
/// priority) and a derived primary action (open the dominant file, else the folder) with an absolute
/// target so the UI can open it via the OS default program. Pure read — `git ls-files` is collected
/// once, then the pure core folds it; no mutation. A product without a Produkt-Stack has no
/// Glob-Satz, so every tracked file becomes a Waise (nothing is ever lost) rather than an error.
#[tauri::command]
async fn read_werkbank_cmd(product: String) -> Result<WerkbankView, String> {
    // `git ls-files` is local but walks the index; off the main thread for the same reason the other
    // git reads are — a large product can never freeze the WebView.
    on_blocking(move || {
        let root = Path::new(&product);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {product}"));
        }
        read_werkbank(root).map_err(|e| e.to_string())
    })
    .await
}

/// **Manuelle Zuordnung** einer Waise zu einem Baustein (Folge von Issue #47/#50): der Nutzer
/// ordnet eine erfasste Datei aus der Software heraus einem Baustein zu — ohne sie im Dateimanager
/// zu verschieben. Die Zuordnung ist ein zerstörungsfreies Etikett in `_plm/zuordnung.json`; sie
/// gewinnt über die Glob/Heimat-Konvention und ignoriert die Heimat-Grenze. Gibt die frisch
/// berechnete Werkbank zurück, sodass die Karte sofort erscheint.
#[tauri::command]
async fn assign_artefakt_cmd(
    product: String,
    file: String,
    baustein_id: String,
) -> Result<WerkbankView, String> {
    on_blocking(move || {
        let root = Path::new(&product);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {product}"));
        }
        zuordnungstore::assign(root, &file, &baustein_id).map_err(|e| e.to_string())?;
        read_werkbank(root).map_err(|e| e.to_string())
    })
    .await
}

/// Die **manuelle Zuordnung** einer Datei wieder lösen (Folge von Issue #47/#50): die Datei fällt
/// zurück auf die Konvention bzw. wird wieder zur Waise. Gibt die frisch berechnete Werkbank zurück.
#[tauri::command]
async fn clear_artefakt_cmd(product: String, file: String) -> Result<WerkbankView, String> {
    on_blocking(move || {
        let root = Path::new(&product);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {product}"));
        }
        zuordnungstore::clear(root, &file).map_err(|e| e.to_string())?;
        read_werkbank(root).map_err(|e| e.to_string())
    })
    .await
}

/// **Resolve the laute Ausnahme** (Issue #43, E41): the single moment the user answers the loud
/// question by choosing whose stand applies for the contested artifact. The backend then finishes
/// the sync with the chosen side and a raw git conflict marker is NEVER written to the worktree —
/// the dangerous hand-resolution stays hidden behind "mein Stand" / "Bens Stand". On success the
/// daily rhythm resumes quietly (`gesichert`); raw git (push/pull/merge) never surfaces.
#[tauri::command]
async fn resolve_sync_cmd(
    path: String,
    artifact: String,
    choice: StandChoice,
) -> Result<SyncOutcome, String> {
    // The resolve fetches + merges + commits (the fetch is networked, bounded); off the main thread
    // so finishing the sync can never freeze the WebView.
    on_blocking(move || {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("Kein Ordner: {path}"));
        }
        resolve_sync(root, &artifact, choice).map_err(|e| e.to_string())
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

/// Validate that `path` is a **plausible product folder** before re-linking to it (Issue #89,
/// PRD-US5). The check is deliberately gentle but real: the folder must exist and be reachable
/// (we probe with `read_dir`, like the search fan-out's offline probe), and it should look like a
/// product — a `_plm` metadata store or a `.git` repo present. A path that exists but carries
/// neither marker is rejected so a re-link can never point at an arbitrary, non-product folder.
/// Returns a German error string describing the first failed condition. Pure-ish (reads disk
/// only); no mutation.
fn check_plausible_product(path: &Path) -> Result<(), String> {
    if std::fs::read_dir(path).is_err() {
        return Err("Ordner nicht erreichbar".to_string());
    }
    let has_plm = path.join("_plm").is_dir();
    let has_git = path.join(".git").exists();
    if !has_plm && !has_git {
        return Err("Kein Produkt erkennbar (weder _plm noch Git im Ordner)".to_string());
    }
    Ok(())
}

/// Re-link a moved product (Issue #89, PRD-US5): a product whose folder was moved/renamed outside
/// the app points its registry entry at nothing and surfaces as offline. Rather than orphaning it,
/// the user re-points the entry to the new folder. The new path is validated as a plausible
/// product first ([`check_plausible_product`]) so there is no dead re-link; then the registry entry
/// is **replaced** (not duplicated) via the pure [`relink_path`](registry::relink_path) core — same
/// normalize/de-dup as registering, so re-linking onto an already-registered product merges instead
/// of duplicating. The display name is re-derived from the new path. Returns the refreshed list.
#[tauri::command]
fn relink_product(
    app: tauri::AppHandle,
    old_path: String,
    new_path: String,
) -> Result<Vec<RegisteredProduct>, String> {
    let file = resolve_registry_file(&app)?;
    check_plausible_product(Path::new(new_path.trim()))?;
    relink_registered(&file, &old_path, &new_path).map_err(|e| e.to_string())
}

/// Resolve the app-level Konto-Server-Adresse file under Tauri's app config dir (ADR 0004, Issue
/// #90). Lives at app level — NOT inside any product — because the Konto is ONE app-wide server
/// identity, reused for all products (next to the Produkt-Registry, #45). A failure to resolve the
/// config dir surfaces as a German error string.
fn resolve_konto_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("App-Konfigurationsordner nicht ermittelbar: {e}"))?;
    Ok(konto_path(&dir))
}

/// Read the app-wide Konto (ADR 0004, Issue #90): returns the normalized Base-URL + the angemeldete
/// account, or `None` when no Konto is set yet. **Never** returns the token — it stays in the OS
/// keystore and never leaves the backend. A missing/corrupt config reads as "kein Konto" (`None`),
/// never an error.
#[tauri::command]
fn read_konto(app: tauri::AppHandle) -> Result<Option<KontoView>, String> {
    let file = resolve_konto_file(&app)?;
    Ok(read_konto_file(&file).map(|c| KontoView {
        base_url: c.base_url,
        account: c.account,
    }))
}

/// Set up + verify the app-wide Konto (ADR 0004, Issue #90): normalize the typed Server-Adresse,
/// verify the credentials against `GET /api/v1/user` (200 = connection ok + token valid + returns
/// the account name; 401 = token wrong; network error = check server address), then persist the
/// Base-URL app-level and write the credentials to the OS keystore (host-keyed, like the existing
/// ceremony). Returns the Konto view (Base-URL + account) — **never** the token, which is never
/// echoed back or logged. Errors are typed like the existing ceremony commands: `auth` reopens the
/// token field, `keystore` reports the OS keystore is unreachable, `error` is everything else.
///
/// The check deliberately covers ONLY connection + token validity — the repo-create permission is
/// NOT pre-checked (it surfaces honestly on first publish via `forgejo::ensure_repo`'s 403).
#[tauri::command]
async fn save_konto(
    app: tauri::AppHandle,
    server: String,
    username: String,
    token: String,
) -> Result<KontoView, AppError> {
    let file = resolve_konto_file(&app).map_err(|message| AppError {
        code: "error".to_string(),
        message,
    })?;
    // Touches the network (verify) and the OS keystore; off the WebView main thread so the panel
    // never freezes. (Inline `spawn_blocking` rather than `on_blocking` because the error is typed.)
    tauri::async_runtime::spawn_blocking(move || {
        // 1) Normalize the Server-Adresse — a typo fails here, before any network call.
        let base_url = konto::normalize_base_url(&server).map_err(|message| AppError {
            code: "error".to_string(),
            message,
        })?;
        let user = username.trim().to_string();
        if user.is_empty() {
            return Err(AppError {
                code: "auth".to_string(),
                message: "Benutzername fehlt.".to_string(),
            });
        }
        // 2) Verify connection + token validity; on 200 the account name comes back.
        let account =
            forgejo::verify_account(&base_url, &user, &token).map_err(AppError::from_io)?;
        // 3) Write the credentials to the OS keystore, host-keyed under the Konto host origin (the
        //    same place askpass and `forgejo::ensure_repo` look). The Konto is the single writer of
        //    credentials (ADR 0004).
        credentials::store(&base_url, &user, &token).map_err(|e| match e {
            credentials::CredentialError::Unavailable(d) => AppError {
                code: "keystore".to_string(),
                message: format!("OS-Schlüsselbund nicht erreichbar: {d}"),
            },
            other => AppError {
                code: "keystore".to_string(),
                message: other.to_string(),
            },
        })?;
        // 4) Persist the Base-URL + confirmed account app-level as JSON (never the token).
        write_konto(
            &file,
            &KontoConfig {
                base_url: base_url.clone(),
                account: account.clone(),
            },
        )
        .map_err(|e| AppError {
            code: "error".to_string(),
            message: format!("Konto-Konfiguration konnte nicht gespeichert werden: {e}"),
        })?;
        Ok(KontoView { base_url, account })
    })
    .await
    .map_err(|e| AppError {
        code: "error".to_string(),
        message: format!("Hintergrund-Task abgebrochen: {e}"),
    })?
}

/// **Konto entfernen** (ADR 0004, Issue #91): read the persisted Base-URL, delete the host-keyed
/// keystore entries for that Konto host, and remove the persisted Base-URL JSON. **Idempotent**: a
/// missing keystore entry and a missing JSON file are both success, so „entfernen" without a Konto
/// is a no-op, never an error (Kriterium 1 + 5).
///
/// CRITICAL INVARIANT (ADR 0004): this NEVER touches existing product remotes — no `.git/config`
/// rewriting, no mass-repoint. Removing the Konto only pauses *sharing*; local work on products
/// continues unchanged, and a product re-shares once a Konto is set again.
#[tauri::command]
fn clear_konto(app: tauri::AppHandle) -> Result<(), String> {
    let file = resolve_konto_file(&app)?;
    // Read the Base-URL first so we know which host's keystore entries to forget. A missing/corrupt
    // config means there is nothing keyed to delete — still a clean no-op.
    if let Some(config) = read_konto_file(&file) {
        // `credentials::delete` is idempotent — a missing entry is treated as already-removed; only
        // a genuinely unreachable keystore surfaces as an error.
        credentials::delete(&config.base_url).map_err(|e| e.to_string())?;
    }
    // Remove the persisted Base-URL JSON (idempotent: a missing file is success).
    clear_konto_file(&file).map_err(|e| {
        format!("Konto-Konfiguration konnte nicht entfernt werden: {e}")
    })
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

/// Read the product's Aufgaben & Hinweise (Issue #40). Tasks are opt-in: a product with no task
/// file has zero tasks — never an error. Pure read; the list is returned as-is, the UI splits it
/// into Aufgaben (block-capable) and Hinweise (never block) by `kind`.
#[tauri::command]
fn list_tasks(path: String) -> Result<Vec<Task>, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    Ok(read_tasks(root))
}

/// Create an Aufgabe or Hinweis and persist it (Issue #40). The store mints the id + creation
/// timestamp; the minimal model (Titel/Typ/Verknüpfung/Fälligkeit + „blockiert überall") comes
/// from the caller. Returns the refreshed list so the UI updates in one round-trip.
#[tauri::command]
fn create_task_cmd(
    path: String,
    title: String,
    kind: TaskKind,
    link: Option<TaskLink>,
    due: Option<String>,
    blocks_everywhere: bool,
) -> Result<Vec<Task>, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    create_task(
        root,
        NewTask {
            title,
            kind,
            link,
            due,
            blocks_everywhere,
        },
    )
    .map_err(|e| e.to_string())
}

/// Edit one task (Issue #40). The UI edit form carries a task's full state, so this command
/// **replaces** title/kind/link/due/flag with the given values (a `null` `link`/`due` clears the
/// Verknüpfung/Fälligkeit). `status` is left untouched here — it has its own command. (The pure
/// [`TaskEdit`] keeps the finer clear-vs-untouched distinction for internal use; the wire stays
/// JSON-honest by always setting these fields.) Returns the refreshed list.
#[tauri::command]
fn edit_task_cmd(
    path: String,
    id: String,
    title: String,
    kind: TaskKind,
    link: Option<TaskLink>,
    due: Option<String>,
    blocks_everywhere: bool,
) -> Result<Vec<Task>, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    update_task(
        root,
        &id,
        TaskEdit {
            title: Some(title),
            kind: Some(kind),
            status: None,
            link: Some(link), // outer Some = "set the link", inner is the new value (None clears)
            due: Some(due),
            blocks_everywhere: Some(blocks_everywhere),
        },
    )
    .map_err(|e| e.to_string())
}

/// Set just the status of one task — erledigen / verwerfen / wieder öffnen (Issue #40). The
/// common gesture, kept separate from the full edit. Returns the refreshed list.
#[tauri::command]
fn set_task_status_cmd(path: String, id: String, status: TaskStatus) -> Result<Vec<Task>, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    set_task_status(root, &id, status).map_err(|e| e.to_string())
}

/// Delete one task (Issue #40). Absent id ⇒ no-op. Returns the refreshed list.
#[tauri::command]
fn delete_task_cmd(path: String, id: String) -> Result<Vec<Task>, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    delete_task(root, &id).map_err(|e| e.to_string())
}

/// Decide whether a checkpoint at the intended Revision-Art is blocked by open Aufgaben
/// (Issue #49, E42). The Strenge lives on the Art: a Freigabe is blocked by any open Aufgabe, a
/// Prototyp only by an open „blockiert überall" Aufgabe, and a Hinweis never blocks. Pure read of
/// the product's task store; the judgement is the pure [`aufgabenblock::decide_block`] core.
/// Returns whether it is blocked and the ids of the blocking tasks (so Issue #52's Freigabe-Gate
/// can name them). A product with no task store is never blocked.
#[tauri::command]
fn evaluate_task_block(path: String, art: RevisionArt) -> Result<BlockDecision, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    Ok(block_for_art(root, art))
}

/// Die **Freigabe-Gate**-Verdict für einen Checkpoint bei der angestrebten Revision-Art
/// berechnen (Issue #52, E19/E19.3). Sammelt die offenen Punkte — offene Aufgaben (#49), Waisen
/// (#47) und Stale-Kanten (#10) — und staffelt sie **nach Härte** hinter **einem** kontextabhängigen
/// Knopf: alles sauber → „Taggen"; weicher Block (Waise/Pflicht) → „Trotzdem freigeben" + ein
/// protokollierter Satz; harter Block (offene blockierende Aufgabe) → Knopf aus, daneben die
/// Aufgabe mit ihren Auswegen. Reine Lesepfade der `_plm`-Stores; das Urteil ist der reine
/// [`freigabegate::decide_gate`]-Kern. Ein leeres Produkt ist sauber (sperrt nie aus — E22).
#[tauri::command]
fn evaluate_freigabe_gate(path: String, art: RevisionArt) -> Result<GateVerdict, String> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(format!("Kein Ordner: {path}"));
    }
    Ok(gate_for_art(root, art))
}

/// Den **protokollierten Begründungs-Satz** eines weichen Blocks festhalten (Issue #52, E19/§22.1).
/// Ein weicher Block (Waise / fehlendes Pflicht-Artefakt) ist bewusst überwindbar — aber **nur per
/// protokollierter Begründung**. Diese landet als dauerhafte Zeile im Diagnose-Log, damit das
/// „Trotzdem freigeben" nachvollziehbar bleibt.
#[tauri::command]
fn log_freigabe_begruendung(version: String, begruendung: String) {
    gitlog::record(
        "freigabe-begruendung",
        format!("Freigabe „{version}“ trotz weichem Block: {begruendung}"),
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Das Git-/Sync-Diagnose-Log lesen (Issue #54-Folge) — das In-App-Diagnose-Panel pollt das, um
/// zu zeigen, **ob und warum** ein Push lief oder nicht (Warden-Entscheidung + reale git-Exits).
/// Älteste Zeile zuerst, jüngste zuletzt.
#[tauri::command]
fn read_git_log() -> Vec<String> {
    gitlog::snapshot()
}

/// Den In-Memory-Ring des Diagnose-Logs leeren (die Logdatei bleibt als dauerhaftes Protokoll).
#[tauri::command]
fn clear_git_log() {
    gitlog::clear();
}

/// Der absolute Pfad der Diagnose-Logdatei, damit das Panel „`tail -f <pfad>`" anbieten kann.
#[tauri::command]
fn git_log_path() -> Option<String> {
    gitlog::file_path().map(|p| p.display().to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(WatcherState::default())
        .setup(|app| {
            // Diagnose-Log (Issue #54-Folge) auf den App-Log-Ordner zeigen lassen, damit ein
            // still scheiternder Push nachvollziehbar wird — im In-App-Panel ODER per `tail -f`.
            // Best-effort: kein Log-Ordner ⇒ nur In-Memory-Ring, die App startet trotzdem.
            if let Ok(dir) = app.path().app_log_dir() {
                let _ = std::fs::create_dir_all(&dir);
                let file = dir.join("git-diagnose.log");
                eprintln!("Git-Diagnose-Log: {}", file.display());
                gitlog::set_file(file);
            }
            // Windows: das gebündelte portable git in den gitrunner verdrahten, damit alle
            // git-Spawns das mitgelieferte git/git-lfs nutzen statt einer System-Installation.
            // Best-effort — fehlt das Bundle, greift der PATH-Fallback und die App startet trotzdem.
            #[cfg(windows)]
            wire_bundled_git(&app.handle().clone());
            // Idempotent, version-gated seeding of the bundled default Bausteine on startup
            // (ADR 0003). A seeding failure must not stop the app from launching — it only means
            // the Bibliothek starts empty/stale; surface it on stderr and carry on.
            if let Err(e) = seed_bibliothek(&app.handle().clone()) {
                eprintln!("Bibliothek-Seeding übersprungen: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_product,
            start_watching,
            stop_watching,
            read_version_graph,
            promote_revision,
            toggle_revision_art,
            knoten_als_ordner,
            knoten_abzweigen,
            knoten_zurueckwerfen,
            read_edges,
            add_edge,
            remove_edge,
            confirm_pair_edge_cmd,
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
            freigeben,
            sichern,
            evaluate_freigabe_gate,
            log_freigabe_begruendung,
            read_git_log,
            clear_git_log,
            git_log_path,
            sweep_clean_locks,
            sync_product,
            list_bibliothek,
            list_toolstacks,
            toolstack_baustein_ids,
            create_product_stack_cmd,
            extend_product_stack_cmd,
            stilllegen_baustein_cmd,
            read_product_stack,
            read_werkbank_cmd,
            assign_artefakt_cmd,
            clear_artefakt_cmd,
            resolve_sync_cmd,
            list_products,
            register_product,
            unregister_product,
            relink_product,
            read_konto,
            save_konto,
            clear_konto,
            search_products,
            list_tasks,
            create_task_cmd,
            edit_task_cmd,
            set_task_status_cmd,
            delete_task_cmd,
            evaluate_task_block
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
