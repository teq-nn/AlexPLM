<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import type {
    Baustein,
    EdgeView,
    GateReport,
    ImportResult,
    ProductView,
    ArtifactSignal,
    ForeignLock,
    SetupReport,
    Stand,
    StandEvent,
    StandNode,
    VersionGraph,
    WardenAction,
    SyncOutcome,
    LoudQuestion,
    StandChoice,
  } from "$lib/types";
  import VersionBar from "$lib/VersionBar.svelte";
  import ArtifactCard from "$lib/ArtifactCard.svelte";
  import ForeignLocksPanel from "$lib/ForeignLocksPanel.svelte";
  import HistorieGate from "$lib/HistorieGate.svelte";
  import EinrichtungsZeremonie from "$lib/EinrichtungsZeremonie.svelte";
  import StandList from "$lib/StandList.svelte";
  import VersionTree from "$lib/VersionTree.svelte";
  import Sicherungsstatus from "$lib/Sicherungsstatus.svelte";
  import LauteAusnahme from "$lib/LauteAusnahme.svelte";

  // self-hosted fonts (offline WebView) + design tokens
  import "@fontsource/archivo/400.css";
  import "@fontsource/archivo/500.css";
  import "@fontsource/archivo/600.css";
  import "@fontsource/archivo/700.css";
  import "@fontsource/ibm-plex-mono/400.css";
  import "@fontsource/ibm-plex-mono/500.css";
  import "@fontsource/ibm-plex-mono/600.css";
  import "$lib/tokens.css";

  let product = $state<ProductView | null>(null);
  let productPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let loading = $state<"open" | "import" | "gate" | "migrate" | null>(null);
  // Import outcome, in the tool's own vocabulary — never "git" / "commit".
  let imported = $state<ImportResult | null>(null);
  // When the gate decides the dangerous branch, hold the chosen folder + report here so the
  // "Historie anfassen" modal can explain the stakes before any rewrite.
  let gate = $state<{ path: string; report: GateReport } | null>(null);
  // A plain refusal note when the folder is shared (E38: never poison others' clones).
  let refusal = $state<string | null>(null);

  // Auto-Lock & Status-Signale (Issue #6, E37). Both are *derived purely* by reading git back
  // (`git lfs locks` + worktree status); nothing is mirrored or cached as a second truth.
  let signals = $state<Record<string, ArtifactSignal>>({});
  let foreignLocks = $state<ForeignLock[]>([]);
  let statusTimer: ReturnType<typeof setInterval> | null = null;

  // The Lock Warden's last decided action (Issue #9, E35), surfaced in the tool's own
  // vocabulary by the Sicherungsstatus readout — "gesichert" (Sicherungs-Push) / "freigegeben"
  // (Freigabe-Push) / "Sperre gelöst" (auto-unlock). The safety-critical decision (and the
  // Binär-Invariante) lives entirely in the Rust core; the UI only reflects what it returns.
  // `refuse` surfaces as nothing — the daily rhythm stays silent.
  let wardenAction = $state<WardenAction | null>(null);

  /** Run a Lock Warden checkpoint for one artifact and reflect the action it decided.
   *  Best-effort: a push failure (e.g. no server yet) must never break the silent rhythm. */
  async function runCheckpoint(path: string, milestone: boolean) {
    if (!productPath) return;
    try {
      const action = await invoke<WardenAction>("run_checkpoint", {
        product: productPath,
        path,
        milestone,
      });
      // Only a real action lights the readout; Refuse leaves the rhythm silent.
      if (action !== "refuse") wardenAction = action;
      // At every checkpoint, self-heal: auto-unlock every held lock whose path is now locally
      // clean (committed, no open edit). The Lock Warden decides per path (Issue #42, E31/E35);
      // the freed binaries rest read-only (frei) again. Best-effort — never breaks the rhythm.
      await sweepCleanLocks();
      await refreshStatus();
    } catch (e) {
      // The two push types are background safety nets; surfacing the raw error would break the
      // silent vocabulary, so we swallow it (a louder, in-tool sync error is a later slice).
    }
  }

  /** Auto-unlock every held lock whose path is locally clean (Issue #42). Best-effort: an
   *  offline/unpublished repo simply frees nothing — never breaks the silent rhythm. */
  async function sweepCleanLocks() {
    if (!productPath) return;
    try {
      await invoke<string[]>("sweep_clean_locks", { product: productPath });
    } catch (e) {
      // Self-healing is a quiet safety net; a hiccup must never surface as a loud error.
    }
  }

  // The stiller Sync + Sync Decider (Issue #11, E41). The daily net-sync runs SILENTLY in the
  // background: it just keeps the local stand "aktuell". The user never sees push/pull/merge.
  // `syncQuiet` reflects the calm state ("aktuell" / "gesichert"); a real, unmergeable
  // contradiction surfaces as `loud` — the SINGLE orange-frame moment in the whole instrument.
  let syncQuiet = $state<"aktuell" | "gesichert" | null>(null);
  let loud = $state<LoudQuestion | null>(null);
  // While the chosen side is being applied + the merge finished (Issue #43), the orange-frame keys
  // are disabled so the one deliberate press cannot be double-fired.
  let resolving = $state(false);
  let syncTimer: ReturnType<typeof setInterval> | null = null;
  // Guard so a slow networked fetch never overlaps the next 8-second sync tick (see statusInFlight).
  let syncInFlight = false;

  /** Run one silent daily sync pass (E41). Best-effort: an offline/unpublished repo simply stays
   *  quiet — a raw sync error must never break the silent vocabulary. The pure Sync Decider (Rust)
   *  decides silent-merge vs. the loud exception; the UI only reflects the result. */
  async function runSync() {
    if (!productPath || syncInFlight) return;
    // While a loud exception is unresolved, do not keep re-running into it — wait for the choice.
    if (loud) return;
    syncInFlight = true;
    try {
      const outcome = await invoke<SyncOutcome>("sync_product", {
        path: productPath,
        other: foreignLocks[0]?.owner ?? null,
      });
      const s = outcome.status;
      if (s === "aktuell") {
        syncQuiet = "aktuell";
      } else if (s === "gesichert") {
        syncQuiet = "gesichert";
        // a silent merge may have changed artifacts/timestamps — refresh the quiet views
        await refreshGraph();
        await refreshEdges();
        await refreshStatus();
      } else if (typeof s === "object" && "laute-ausnahme" in s) {
        // The one moment the tool raises its voice: stop and ask whose stand applies.
        loud = s["laute-ausnahme"];
      }
    } catch (e) {
      // Silent by design (E41): no server / offline keeps the daily rhythm quiet, never loud.
    } finally {
      syncInFlight = false;
    }
  }

  function startSyncLoop() {
    stopSyncLoop();
    void runSync(); // pull on open (E41), then on idle ticks
    syncTimer = setInterval(() => void runSync(), 8000);
  }
  function stopSyncLoop() {
    if (syncTimer !== null) {
      clearInterval(syncTimer);
      syncTimer = null;
    }
  }
  onDestroy(stopSyncLoop);

  /** Resolve the loud exception by choosing whose stand applies (Issue #43, E41). The backend
   *  applies the chosen side for the contested artifact and FINISHES the sync — a raw git conflict
   *  marker is never written to the worktree (the dangerous hand-resolution stays hidden behind
   *  "mein Stand" / "Bens Stand"). On success the orange frame closes and the silent rhythm resumes
   *  "gesichert"; NO git vocabulary surfaces. */
  async function resolveLoud(choice: StandChoice) {
    if (!productPath || !loud || resolving) return;
    // The first contested artifact is the one the question names; resolving it (and any other
    // contested touch, defensively, in the backend) lets the merge finish cleanly.
    const artifact = loud.artefakte[0];
    if (!artifact) return;
    resolving = true;
    try {
      const outcome = await invoke<SyncOutcome>("resolve_sync_cmd", {
        path: productPath,
        artifact,
        choice,
      });
      loud = null;
      // The resolve completes the merge; reflect the calm state and refresh the quiet views.
      syncQuiet =
        outcome.status === "aktuell" || outcome.status === "gesichert"
          ? outcome.status
          : "gesichert";
      await refreshGraph();
      await refreshEdges();
      await refreshStatus();
    } catch (e) {
      // A resolve failure is real — surface it plainly (still no raw git markers, by construction
      // of the backend). The orange frame stays open so the user can try again.
      error = String(e);
    } finally {
      resolving = false;
    }
  }

  // The one-time Einrichtungs-Zeremonie (Issue #5, E41). `setup` is the server-decided state;
  // the ceremony modal opens on demand and auto-opens once when a product without a connected
  // server is opened/imported, then stays out of the silent daily rhythm.
  let setup = $state<SetupReport | null>(null);
  let ceremonyOpen = $state(false);

  /** Read the ceremony state from git (server connected? published?). Best-effort. */
  async function refreshSetup() {
    if (!productPath) return;
    try {
      setup = await invoke<SetupReport>("read_setup_state", { path: productPath });
    } catch (e) {
      // The ceremony state is auxiliary; a read failure must not break the shell.
      setup = null;
    }
  }

  // Guard so a slow status read (a networked `git lfs locks` can take up to the backend bound)
  // never overlaps the next 4-second tick. Without it, ticks pile up faster than they drain.
  let statusInFlight = false;

  /** Re-read the world from git: per-artifact LED status + the foreign-locks panel. */
  async function refreshStatus() {
    if (!productPath || !product || statusInFlight) return;
    statusInFlight = true;
    const paths = product.bausteine
      .map((b) => b.main_file)
      .filter((f): f is string => f !== null);
    try {
      const [sigs, foreign] = await Promise.all([
        invoke<ArtifactSignal[]>("read_status", {
          product: productPath,
          paths,
        }),
        invoke<ForeignLock[]>("read_foreign_locks", { product: productPath }),
      ]);
      signals = Object.fromEntries(sigs.map((s) => [s.path, s]));
      foreignLocks = foreign;
    } catch (e) {
      // Read-only status is best-effort; never blocks the shell (e.g. no LFS remote).
      error = String(e);
    } finally {
      statusInFlight = false;
    }
  }

  /** Start polling git for live status; replaces any previous loop. */
  function startStatusLoop() {
    stopStatusLoop();
    void refreshStatus();
    statusTimer = setInterval(() => void refreshStatus(), 4000);
  }
  function stopStatusLoop() {
    if (statusTimer !== null) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  }
  onDestroy(stopStatusLoop);

  /** Editing/opening a lockable artifact auto-acquires a `git lfs lock` (E31), then re-reads. */
  async function editBaustein(mainFile: string | null) {
    if (!productPath || !mainFile) return;
    try {
      await invoke<boolean>("lock_artifact", {
        product: productPath,
        path: mainFile,
      });
    } catch (e) {
      error = String(e); // a foreign-held lock is real, loud coordination — surface it
    }
    await refreshStatus();
  }

  function reset() {
    error = null;
    imported = null;
    refusal = null;
    signals = {};
    foreignLocks = [];
    wardenAction = null;
    setup = null;
    ceremonyOpen = false;
    syncQuiet = null;
    loud = null;
    resolving = false;
    stands = [];
    graph = null;
    edgeView = { edges: [], warnings: [] };
    stopStatusLoop();
    stopSyncLoop();
  }

  // The running ledger of Stände, newest first. Grows silently as saves settle.
  let stands = $state<Stand[]>([]);
  let standSeq = 0;

  // The version tree (Issue #8): Stände as nodes, Meilensteine marked, active version
  // driving the bar. Read read-only and refreshed whenever a new Stand settles.
  let graph = $state<VersionGraph | null>(null);

  // Manual „abgeleitet von" edges + their Stale-Warnungen (Issue #10). Opt-in: a product
  // with no drawn edges keeps this empty and shows no warnings (E40).
  let edgeView = $state<EdgeView>({ edges: [], warnings: [] });

  // Per-artifact lookups derived from the edge view: which source a card is derived from,
  // and whether it is currently stale (source newer than derivation — E26).
  const sourceOf = $derived(
    new Map(edgeView.edges.map((e) => [e.derived, e.source])),
  );
  const staleSet = $derived(new Set(edgeView.warnings.map((w) => w.derived)));

  async function refreshGraph() {
    if (!productPath) return;
    try {
      graph = await invoke<VersionGraph>("read_version_graph", {
        path: productPath,
      });
    } catch (e) {
      // The tree is a read-only view; a transient read failure must not break the shell.
      error = String(e);
    }
  }

  async function refreshEdges() {
    if (!productPath) return;
    try {
      edgeView = await invoke<EdgeView>("read_edges", { path: productPath });
    } catch (e) {
      // Edges are opt-in extra; a read failure must not break the read-only shell.
      error = String(e);
    }
  }

  // Other Bausteine this card can be derived from (itself excluded; no self-edge).
  function candidatesFor(self: Baustein): Baustein[] {
    return product ? product.bausteine.filter((b) => b.path !== self.path) : [];
  }

  async function deriveFrom(derived: string, source: string) {
    if (!productPath) return;
    edgeView = await invoke<EdgeView>("add_edge", {
      path: productPath,
      derived,
      source,
    });
  }

  async function clearEdge(derived: string) {
    if (!productPath) return;
    const source = sourceOf.get(derived);
    if (!source) return;
    edgeView = await invoke<EdgeView>("remove_edge", {
      path: productPath,
      derived,
      source,
    });
  }

  // Single long-lived listener for settled saves. The watcher (Rust) does the
  // debouncing and the silent local commit; we only render the resulting Stand and
  // refresh the tree so the new node appears.
  let unlisten: UnlistenFn | null = null;
  listen<StandEvent>("stand-created", (e) => {
    stands = [{ ...e.payload, id: standSeq++ }, ...stands];
    void refreshGraph();
    // A new save can change an artifact's timestamp, so Stale-Warnungen may flip (E26).
    void refreshEdges();
    // A settled save is a laufender Checkpoint: the Lock Warden runs and, for open work,
    // mirrors it to the private backup (Sicherungs-Push) — never the shared stand (E35).
    void runCheckpoint(e.payload.path, false);
  }).then((u) => (unlisten = u));

  // The watcher auto-locked the first dirty lockable path (Issue #42): the lock now exists before
  // any checkpoint, closing the Binär-Invarianten-Fenster. Re-read so the card's LED reflects it
  // (mine → grey/in Arbeit; a colleague would now see „gesperrt von X seit …"). No git vocabulary.
  let unlistenLock: UnlistenFn | null = null;
  listen<string>("lock-acquired", () => {
    void refreshStatus();
  }).then((u) => (unlistenLock = u));

  onDestroy(() => {
    unlisten?.();
    unlistenLock?.();
    void invoke("stop_watching").catch(() => {});
  });

  async function openProduct() {
    reset();
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Produkt öffnen",
    });
    if (typeof selected !== "string") return;
    loading = "open";
    try {
      product = await invoke<ProductView>("open_product", { path: selected });
      productPath = selected;
      loadWidths(selected); // restore this product's saved column layout
      // A fresh product starts with a fresh ledger, then watching begins silently.
      stands = [];
      await invoke("start_watching", { path: selected });
      await refreshGraph();
      await refreshEdges();
      await refreshSetup();
      startStatusLoop();
      // The daily net-sync begins silently (E41): pull on open, then on idle ticks.
      startSyncLoop();
    } catch (e) {
      error = String(e);
      product = null;
      productPath = null;
      graph = null;
      edgeView = { edges: [], warnings: [] };
    } finally {
      loading = null;
    }
  }

  async function importProduct() {
    reset();
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Ordner als Produkt anlegen",
    });
    if (typeof selected !== "string") return;

    // First run the Import Gate (read-only): it tells us whether this folder is safe to
    // clean-import, must go behind the "Historie anfassen" gate, or has to be refused.
    loading = "gate";
    let report: GateReport;
    try {
      report = await invoke<GateReport>("evaluate_gate", { path: selected });
    } catch (e) {
      error = String(e);
      loading = null;
      return;
    }

    if (report.decision === "refuse") {
      // Shared clones exist — rewriting history would poison them. Refuse, clearly.
      refusal =
        "Dieser Ordner ist bereits geteilt. Ein Umschreiben der Historie würde fremde " +
        "Kopien vergiften — das Werkzeug verweigert es. Bitte zuerst lokal/ungeteilt anlegen.";
      loading = null;
      return;
    }

    if (report.decision === "migrate-behind-gate") {
      // Hand off to the bewusste "Historie anfassen" confirmation; do nothing destructive yet.
      gate = { path: selected, report };
      loading = null;
      return;
    }

    // clean-init: the safe, non-destructive import path (#3).
    await runCleanImport(selected);
  }

  async function runCleanImport(path: string) {
    loading = "import";
    try {
      const result = await invoke<ImportResult>("import_product", { path });
      imported = result;
      product = result.product;
      productPath = path;
      loadWidths(path); // restore this product's saved column layout
      stands = [];
      await invoke("start_watching", { path });
      await refreshGraph();
      await refreshEdges();
      await refreshSetup();
      // A freshly created product has no server yet — open the one-time ceremony once so the
      // user is guided to share it. Reopening/daily use never re-triggers this.
      if (setup && setup.stage === "not-configured") ceremonyOpen = true;
      startStatusLoop();
      startSyncLoop();
    } catch (e) {
      error = String(e);
      product = null;
      productPath = null;
      graph = null;
      edgeView = { edges: [], warnings: [] };
    } finally {
      loading = null;
    }
  }

  async function confirmMigrate() {
    if (!gate) return;
    const path = gate.path;
    loading = "migrate";
    try {
      const result = await invoke<ImportResult>("migrate_history", { path });
      imported = result;
      product = result.product;
      gate = null;
    } catch (e) {
      error = String(e);
      gate = null;
    } finally {
      loading = null;
    }
  }

  // ── Spaltenbreiten (Issue #26) ──────────────────────────────────────────────
  // The three columns (Versionsbaum + Fremde-Sperren-Schiene) carry explicit widths the
  // user can drag; the Bausteine work area simply flexes into whatever space is left. Each
  // width has a sensible Mindestbreite so no column can be dragged away to nothing, and the
  // work area is protected by its own minimum so resizing the window never collapses it.
  const TREE_MIN = 220;
  const TREE_MAX = 640;
  const RAIL_MIN = 200;
  const RAIL_MAX = 520;
  const TREE_DEFAULT = 300;
  const RAIL_DEFAULT = 264;
  // Keep the Bausteine work area usable even when columns grow / the window shrinks.
  const WORK_MIN = 320;

  let treeWidth = $state(TREE_DEFAULT);
  let railWidth = $state(RAIL_DEFAULT);

  const clamp = (v: number, lo: number, hi: number) =>
    Math.min(hi, Math.max(lo, v));

  // Widths persist per product (the WebView origin is already per-window), so reopening the
  // same product restores its layout. A plain localStorage key — the app keeps no other
  // frontend persistence, and these are pure view preferences, never domain truth.
  function layoutKey(path: string): string {
    return `plm.spaltenbreiten:${path}`;
  }

  function loadWidths(path: string) {
    try {
      const raw = localStorage.getItem(layoutKey(path));
      if (!raw) {
        treeWidth = TREE_DEFAULT;
        railWidth = RAIL_DEFAULT;
        return;
      }
      const saved = JSON.parse(raw) as { tree?: number; rail?: number };
      treeWidth = clamp(saved.tree ?? TREE_DEFAULT, TREE_MIN, TREE_MAX);
      railWidth = clamp(saved.rail ?? RAIL_DEFAULT, RAIL_MIN, RAIL_MAX);
    } catch {
      treeWidth = TREE_DEFAULT;
      railWidth = RAIL_DEFAULT;
    }
  }

  function saveWidths() {
    if (!productPath) return;
    try {
      localStorage.setItem(
        layoutKey(productPath),
        JSON.stringify({ tree: treeWidth, rail: railWidth }),
      );
    } catch {
      // View preferences are best-effort; a full/blocked storage must never break the shell.
    }
  }

  // Drag a splitter. `which` says which seam was grabbed; we move the adjacent column's edge
  // and clamp against both the column's own min/max and the work area's minimum so the work
  // never collapses. Pointer capture keeps the drag alive even past the thin handle.
  function startResize(which: "tree" | "rail", ev: PointerEvent) {
    ev.preventDefault();
    const handle = ev.currentTarget as HTMLElement;
    handle.setPointerCapture(ev.pointerId);
    const stage = handle.closest(".stage") as HTMLElement | null;
    const startX = ev.clientX;
    const startTree = treeWidth;
    const startRail = railWidth;

    const onMove = (e: PointerEvent) => {
      const dx = e.clientX - startX;
      const stageW = stage?.clientWidth ?? window.innerWidth;
      if (which === "tree") {
        // The tree sits left of the rail; dragging right grows it (handle is on its left edge).
        const room = stageW - WORK_MIN - railWidth;
        const hi = Math.min(TREE_MAX, Math.max(TREE_MIN, room));
        treeWidth = clamp(startTree - dx, TREE_MIN, hi);
      } else {
        // The rail is the rightmost column; dragging left grows it (handle is on its left edge).
        const room = stageW - WORK_MIN - treeWidth;
        const hi = Math.min(RAIL_MAX, Math.max(RAIL_MIN, room));
        railWidth = clamp(startRail - dx, RAIL_MIN, hi);
      }
    };
    const onUp = (e: PointerEvent) => {
      handle.releasePointerCapture(e.pointerId);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      saveWidths();
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
  }

  // Keyboard nudge for accessibility: arrow keys move the grabbed seam in small steps.
  function nudge(which: "tree" | "rail", e: KeyboardEvent) {
    const step = e.shiftKey ? 32 : 8;
    let delta = 0;
    if (e.key === "ArrowLeft") delta = -step;
    else if (e.key === "ArrowRight") delta = step;
    else return;
    e.preventDefault();
    // Both handles grow their column when moved left, shrink when moved right.
    if (which === "tree") treeWidth = clamp(treeWidth - delta, TREE_MIN, TREE_MAX);
    else railWidth = clamp(railWidth - delta, RAIL_MIN, RAIL_MAX);
    saveWidths();
  }

  // Promote a Stand to a Meilenstein: the user writes the human VERSION_NOTES text (E28),
  // Rust persists it and labels the version durably, then returns the refreshed tree.
  async function promote(node: StandNode, version: string, notes: string) {
    if (!productPath) return;
    graph = await invoke<VersionGraph>("promote_milestone", {
      path: productPath,
      standId: node.id,
      version,
      notes,
    });
    // A Meilenstein is the Freigabe checkpoint ("ich bin fertig damit"): the Lock Warden
    // publishes the finished artifact to the shared stand AND releases its lock atomically
    // (E35). The Binär-Invariante is upheld in the Rust core, never here.
    void runCheckpoint(node.path, true);
  }

  // Toggle a Meilenstein's Art (E42): Prototyp → Freigabe ("Releasen", write-protects the
  // tag) or back ("Un-Release"). Rust persists the Art per tag and flips the write-protect,
  // then returns the refreshed tree. The dreistufige Freigabe-Gate block-check is a separate
  // slice (Issue #52) and plugs into the Rust seam; nothing about it lives here.
  async function toggleArt(node: StandNode) {
    if (!productPath || node.milestone === null) return;
    graph = await invoke<VersionGraph>("toggle_milestone_art", {
      path: productPath,
      version: node.milestone,
    });
  }
</script>

<div class="app">
  <VersionBar
    {product}
    activeMilestone={graph?.active_milestone ?? null}
    activeMilestoneArt={graph?.active_milestone_art ?? null}
  />

  <!-- Einstiegs-Buttons: the product entry points live in their own app-level bar, not in the
       Bausteine pane — they aren't part of browsing Bausteine. The write-vs-read distinction
       stays legible: "Neues Produkt" is the solid primary key (schreibt), "Produkt öffnen" the
       quieter ghost key (liest nur). -->
  <div class="entrybar">
    <div class="entry-actions">
      <button
        class="key"
        onclick={importProduct}
        disabled={loading !== null}
      >
        <span class="label"
          >{loading === "gate"
            ? "prüfe …"
            : loading === "import" || loading === "migrate"
              ? "lege an …"
              : "Neues Produkt"}</span
        >
      </button>
      <button class="key ghost" onclick={openProduct} disabled={loading !== null}>
        <span class="label"
          >{loading === "open" ? "öffne …" : "Produkt öffnen"}</span
        >
      </button>
      <span class="entry-hint label">anlegen schreibt — öffnen liest nur</span>
    </div>
  </div>

  <div class="stage">
    <main class="work">
    <div class="toolbar">
      <span class="label section">Bausteine</span>

      <div class="actions">
        <!-- The stiller Sync's quiet status (Issue #11, E41): "aktuell" / "gesichert" only —
             never push/pull/merge. The loud exception is NOT shown here; it takes the screen. -->
        {#if syncQuiet}
          <span class="readout mono sync" role="status" aria-live="polite">
            <span class="dot" class:fresh={syncQuiet === "aktuell"}></span>
            <span class="readout-text"
              >{syncQuiet === "aktuell" ? "aktuell" : "gesichert"}</span
            >
          </span>
        {/if}

        <!-- The Lock Warden's two push types in the tool's own vocabulary (Issue #9). -->
        <Sicherungsstatus action={wardenAction} />

        {#if setup}
          <!-- One-time ceremony trigger / settled readout. Git-near wording lives ONLY here. -->
          {#if setup.stage === "eingerichtet"}
            <button
              class="readout mono"
              title="Geteilt — Einrichtung abgeschlossen"
              onclick={() => (ceremonyOpen = true)}
            >
              <span class="dot fresh"></span>
              <span class="readout-text">geteilt</span>
            </button>
          {:else}
            <button class="key share" onclick={() => (ceremonyOpen = true)}>
              <span class="label"
                >{setup.stage === "remote-set-not-published"
                  ? "Veröffentlichen"
                  : "Teilen einrichten"}</span
              >
            </button>
          {/if}
        {/if}

        {#if imported}
          <!-- Import outcome chip: recessed instrument readout, tool vocabulary only. -->
          <span class="readout mono" role="status">
            <span class="dot" class:fresh={imported.git_initialized}></span>
            <span class="readout-text">
              {imported.git_initialized
                ? "Produkt angelegt"
                : "Bestehendes übernommen"}
            </span>
            {#if imported.locked_count > 0}
              <span class="readout-sep">·</span>
              <span class="readout-locks"
                >{imported.locked_count.toString().padStart(2, "0")} gesperrt</span
              >
            {/if}
          </span>
        {/if}
      </div>
    </div>

    <div class="content">
      {#if refusal}
        <div class="refusal" role="alert">
          <span class="dot warn" aria-hidden="true"></span>
          <span class="refusal-text label">{refusal}</span>
        </div>
      {/if}
      {#if error}
        <p class="notice mono">{error}</p>
      {:else if product}
        {#if product.bausteine.length > 0}
          <div class="grid">
            {#each product.bausteine as b, i (b.path)}
              <ArtifactCard
                baustein={b}
                index={i}
                candidates={candidatesFor(b)}
                source={sourceOf.get(b.path) ?? null}
                stale={staleSet.has(b.path)}
                onDeriveFrom={(s) => deriveFrom(b.path, s)}
                onClearEdge={() => clearEdge(b.path)}
                signal={b.main_file ? (signals[b.main_file] ?? null) : null}
                onedit={() => editBaustein(b.main_file)}
              />
            {/each}
          </div>
        {:else}
          <p class="notice label">Keine Bausteine in diesem Ordner gefunden</p>
        {/if}
      {:else}
        <div class="empty">
          <div class="empty-panel">
            <span class="label empty-hint">Ordner wählen</span>
            <div class="empty-keys">
              <button
                class="key big"
                onclick={importProduct}
                disabled={loading !== null}
              >
                <span class="label"
                  >{loading === "gate"
                    ? "prüfe …"
                    : loading === "import" || loading === "migrate"
                      ? "lege an …"
                      : "Neues Produkt"}</span
                >
              </button>
              <button
                class="key big ghost"
                onclick={openProduct}
                disabled={loading !== null}
              >
                <span class="label">Produkt öffnen</span>
              </button>
            </div>
            <span class="label empty-sub"
              >anlegen schreibt — öffnen liest nur</span
            >
          </div>
        </div>
      {/if}
    </div>
    </main>

    {#if product}
      <!-- Splitter between the Bausteine work area and the Versionsbaum. A hairline seam
           that widens its grab zone on hover; no orange — routine sizing stays grey.
           role="separator" + focusable IS the resize-splitter ARIA pattern; the generic
           a11y lint for <div> handlers/tabindex doesn't apply, so we silence it here. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="splitter"
        role="separator"
        aria-orientation="vertical"
        aria-label="Breite des Versionsbaums"
        aria-valuenow={treeWidth}
        aria-valuemin={TREE_MIN}
        aria-valuemax={TREE_MAX}
        tabindex="0"
        onpointerdown={(e) => startResize("tree", e)}
        onkeydown={(e) => nudge("tree", e)}
      ></div>

      <div class="tree-col" style="width: {treeWidth}px;">
        <VersionTree {graph} onPromote={promote} onToggleArt={toggleArt} />
      </div>

      <!-- Splitter between the Versionsbaum and the Fremde-Sperren-Schiene. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="splitter"
        role="separator"
        aria-orientation="vertical"
        aria-label="Breite der Fremde-Sperren-Schiene"
        aria-valuenow={railWidth}
        aria-valuemin={RAIL_MIN}
        aria-valuemax={RAIL_MAX}
        tabindex="0"
        onpointerdown={(e) => startResize("rail", e)}
        onkeydown={(e) => nudge("rail", e)}
      ></div>

      <aside class="rail" style="width: {railWidth}px;">
        <ForeignLocksPanel locks={foreignLocks} />
        <StandList {stands} />
      </aside>
    {/if}
  </div>
</div>

{#if gate}
  <HistorieGate
    report={gate.report}
    busy={loading === "migrate"}
    onConfirm={confirmMigrate}
    onCancel={() => (gate = null)}
  />
{/if}

{#if ceremonyOpen && productPath && setup}
  <EinrichtungsZeremonie
    {productPath}
    report={setup}
    onUpdated={(r) => (setup = r)}
    onClose={() => (ceremonyOpen = false)}
  />
{/if}

<!-- The single orange-frame moment (Issue #11, E41): the stiller Sync hit a real, unmergeable
     contradiction and raised its voice. Domain language only; no git markers, ever. -->
{#if loud}
  <LauteAusnahme question={loud} busy={resolving} onChoose={resolveLoud} />
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
  }

  /* The app-level entry bar: product entry points sit here, above the work chassis, so the
     Bausteine pane stays about Bausteine. Reads as a shelf seated under the LCD display. */
  .entrybar {
    display: flex;
    align-items: center;
    padding: 10px 16px;
    background: var(--surface-raised);
    border-bottom: 1px solid var(--hairline);
  }
  .entry-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  /* The read-only distinction, kept legible after the move (mirrors the empty-state sub-line). */
  .entry-hint {
    margin-left: 4px;
    color: var(--ink-muted);
    font-size: 11px;
    opacity: 0.8;
  }

  /* Work chassis + instrument rail (foreign locks + Stände) share the row below the display. */
  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  /* The right-hand instrument rail stacks the foreign-locks panel over the Stände ledger.
     A single hairline seam separates the rail from the work chassis; the children carry
     their own widths, so we pin the rail to the wider of the two for a clean edge. */
  .rail {
    display: flex;
    flex-direction: column;
    flex: none;
    /* width comes from an inline style (drag-set, persisted); these bound it */
    width: 264px;
    min-width: 200px;
    max-width: 520px;
    min-height: 0;
    border-left: 1px solid var(--hairline);
  }

  /* Wrapper that owns the Versionsbaum's drag-set width; the VersionTree's own
     instrument display fills it edge-to-edge. */
  .tree-col {
    flex: none;
    min-width: 220px;
    max-width: 640px;
    min-height: 0;
    display: flex;
  }
  .tree-col > :global(.display) {
    width: 100%;
    flex: 1;
  }

  /* A splitter is a hairline seam with an invisible widened grab zone. It carries no fill of
     its own (the columns it sits between already draw their seams); on hover/active the seam
     brightens to the raised-surface tone. Strictly grey — orange stays reserved for the loud
     exception, never routine layout. */
  .splitter {
    flex: none;
    width: 7px;
    margin: 0 -3px; /* overlap the neighbours' hairlines so no double seam shows */
    position: relative;
    z-index: 1;
    cursor: col-resize;
    touch-action: none;
  }
  .splitter::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 1px;
    transform: translateX(-50%);
    background: transparent;
    transition: background var(--dur) var(--ease);
  }
  .splitter:hover::before {
    background: var(--key-mid);
  }
  .splitter:active::before,
  .splitter:focus-visible::before {
    width: 2px;
    background: var(--ink-muted);
  }
  .splitter:focus-visible {
    outline: none;
  }
  /* Children already style their own surfaces; drop their seams so only the rail's shows. */
  .rail > :global(.panel),
  .rail > :global(.rail) {
    width: 100%;
    border-left: none;
  }
  /* The foreign-locks panel sits at the top at its natural height; Stände fills the rest. */
  .rail > :global(.rail) {
    flex: 1;
    min-height: 0;
    border-top: 1px solid var(--hairline);
  }

  .work {
    flex: 1;
    /* Stay usable when columns grow or the window shrinks — the work area never collapses
       below a legible width; any further squeeze is absorbed by the stage, not this column. */
    min-width: 320px;
    min-height: 0;
    display: flex;
    flex-direction: column;
    /* warm grain so the work area never reads as flat fill */
    background-color: var(--surface-base);
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.025'/%3E%3C/svg%3E");
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 11px 16px;
    border-bottom: 1px solid var(--hairline);
  }
  .section {
    color: var(--ink-muted);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* Import outcome: a small recessed instrument readout, same LCD language as
     the VersionBar screen — never git/commit wording. */
  .readout {
    display: inline-flex;
    align-items: baseline;
    gap: 7px;
    padding: 5px 11px;
    border: none;
    border-radius: var(--radius);
    background: linear-gradient(180deg, #131110, #0b0a09);
    box-shadow:
      inset 0 1px 2px rgba(0, 0, 0, 0.9),
      inset 0 0 0 1px rgba(255, 255, 255, 0.03);
    color: var(--screen-fg);
    font-size: 12px;
    letter-spacing: 0.01em;
    animation: readout-in 260ms var(--ease) backwards;
  }
  /* The settled "geteilt" readout doubles as a button to reopen the ceremony (invite). */
  button.readout {
    cursor: pointer;
    font-family: var(--font-mono);
  }
  button.readout:hover {
    box-shadow:
      inset 0 1px 2px rgba(0, 0, 0, 0.9),
      inset 0 0 0 1px rgba(255, 255, 255, 0.07);
  }
  /* The "Teilen einrichten" / "Veröffentlichen" key: dark, deliberate — a one-time act. */
  .key.share {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
  }
  .key.share:hover {
    background: #2a2724;
  }
  .readout .dot {
    align-self: center;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--led-working);
    box-shadow: 0 0 5px rgba(201, 198, 191, 0.3);
  }
  /* freshly created product gets the "free / done" green; taken-over stays neutral */
  .readout .dot.fresh {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }
  .readout-text {
    color: var(--screen-fg);
    font-weight: 600;
  }
  .readout-sep {
    color: #4a4641;
  }
  .readout-locks {
    color: #b8b4ad;
  }
  @keyframes readout-in {
    from {
      opacity: 0;
      transform: translateY(-2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 18px 16px 28px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(248px, 1fr));
    gap: 12px;
  }

  /* Physical "key": light cap, hairline, seated bottom edge, crisp press. */
  .key {
    appearance: none;
    cursor: pointer;
    background: var(--key-light);
    color: var(--ink-strong);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 8px 14px;
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.12);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .key:hover {
    background: #f5f3ee;
  }
  .key:active {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.12);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.55;
    box-shadow: none;
  }
  .key.big {
    padding: 12px 22px;
  }
  .key .label {
    color: inherit;
  }

  .empty {
    height: 100%;
    display: grid;
    place-items: center;
  }
  .empty-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    padding: 38px 46px;
    border: 1px dashed var(--hairline);
    border-radius: var(--radius);
  }
  .empty-hint {
    color: var(--ink-muted);
  }
  .empty-keys {
    display: flex;
    gap: 12px;
  }
  /* secondary, read-only action reads quieter than the primary "anlegen" key */
  .key.ghost {
    background: transparent;
    box-shadow: none;
    color: var(--ink-default);
  }
  .key.ghost:hover {
    background: var(--surface-raised);
  }
  .empty-sub {
    color: var(--ink-muted);
    font-size: 11px;
    opacity: 0.8;
  }

  .notice {
    color: var(--ink-muted);
    font-size: 13px;
  }

  /* Refusal banner (E38): the tool will not poison shared clones. Calm, not alarmist —
     orange dot for attention, but no full orange fill. */
  .refusal {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-bottom: 16px;
    padding: 12px 14px;
    border: 1px solid var(--hairline);
    border-left: 3px solid var(--led-attention);
    border-radius: var(--radius);
    background: var(--surface-raised);
  }
  .refusal .dot.warn {
    margin-top: 3px;
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--led-attention);
    box-shadow: 0 0 6px rgba(240, 66, 28, 0.4);
  }
  .refusal-text {
    color: var(--ink-default);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 13px;
    line-height: 1.45;
  }
</style>
