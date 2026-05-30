<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onDestroy } from "svelte";
  import type {
    ImportResult,
    ProductView,
    ArtifactSignal,
    ForeignLock,
  } from "$lib/types";
  import VersionBar from "$lib/VersionBar.svelte";
  import ArtifactCard from "$lib/ArtifactCard.svelte";
  import ForeignLocksPanel from "$lib/ForeignLocksPanel.svelte";

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
  let loading = $state<"open" | "import" | null>(null);
  // Import outcome, in the tool's own vocabulary — never "git" / "commit".
  let imported = $state<ImportResult | null>(null);

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
    signals = {};
    foreignLocks = [];
    stopStatusLoop();
  }

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
      startStatusLoop();
    } catch (e) {
      error = String(e);
      product = null;
      productPath = null;
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
    loading = "import";
    try {
      const result = await invoke<ImportResult>("import_product", {
        path: selected,
      });
      imported = result;
      product = result.product;
      productPath = selected;
      startStatusLoop();
    } catch (e) {
      error = String(e);
      product = null;
      productPath = null;
    } finally {
      loading = null;
    }
  }
</script>

<div class="app">
  <VersionBar {product} />

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
            >{loading === "import" ? "lege an …" : "Ordner anlegen"}</span
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
      {#if error}
        <p class="notice mono">{error}</p>
      {:else if product}
        {#if product.bausteine.length > 0}
          <div class="grid">
            {#each product.bausteine as b, i (b.path)}
              <ArtifactCard
                baustein={b}
                index={i}
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
                <span class="label">Ordner anlegen</span>
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
      <ForeignLocksPanel locks={foreignLocks} />
    {/if}
  </div>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
  }

  /* Work chassis + foreign-locks instrument rail share the row below the display. */
  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
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
</style>
