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
    Stand,
    StandEvent,
    StandNode,
    VersionGraph,
  } from "$lib/types";
  import VersionBar from "$lib/VersionBar.svelte";
  import ArtifactCard from "$lib/ArtifactCard.svelte";
  import ForeignLocksPanel from "$lib/ForeignLocksPanel.svelte";
  import HistorieGate from "$lib/HistorieGate.svelte";
  import StandList from "$lib/StandList.svelte";
  import VersionTree from "$lib/VersionTree.svelte";

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

  /** Re-read the world from git: per-artifact LED status + the foreign-locks panel. */
  async function refreshStatus() {
    if (!productPath || !product) return;
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
    stands = [];
    graph = null;
    edgeView = { edges: [], warnings: [] };
    stopStatusLoop();
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
  }).then((u) => (unlisten = u));

  onDestroy(() => {
    unlisten?.();
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
      // A fresh product starts with a fresh ledger, then watching begins silently.
      stands = [];
      await invoke("start_watching", { path: selected });
      await refreshGraph();
      await refreshEdges();
      startStatusLoop();
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
      stands = [];
      await invoke("start_watching", { path });
      await refreshGraph();
      await refreshEdges();
      startStatusLoop();
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
  }
</script>

<div class="app">
  <VersionBar {product} activeMilestone={graph?.active_milestone ?? null} />

  <div class="stage">
    <main class="work">
    <div class="toolbar">
      <span class="label section">Bausteine</span>

      <div class="actions">
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
                : "Ordner anlegen"}</span
          >
        </button>
        <button class="key" onclick={openProduct} disabled={loading !== null}>
          <span class="label"
            >{loading === "open" ? "öffne …" : "Produkt öffnen"}</span
          >
        </button>
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
                      : "Ordner anlegen"}</span
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
      <VersionTree {graph} onPromote={promote} />
      <aside class="rail">
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

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
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
    width: 264px;
    min-height: 0;
    border-left: 1px solid var(--hairline);
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
    min-width: 0;
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
