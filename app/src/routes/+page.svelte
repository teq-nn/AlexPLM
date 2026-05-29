<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import type {
    ProductView,
    Stand,
    StandEvent,
    StandNode,
    VersionGraph,
  } from "$lib/types";
  import VersionBar from "$lib/VersionBar.svelte";
  import ArtifactCard from "$lib/ArtifactCard.svelte";
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
  let loading = $state(false);

  // The running ledger of Stände, newest first. Grows silently as saves settle.
  let stands = $state<Stand[]>([]);
  let standSeq = 0;

  // The version tree (Issue #8): Stände as nodes, Meilensteine marked, active version
  // driving the bar. Read read-only and refreshed whenever a new Stand settles.
  let graph = $state<VersionGraph | null>(null);

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

  // Single long-lived listener for settled saves. The watcher (Rust) does the
  // debouncing and the silent local commit; we only render the resulting Stand and
  // refresh the tree so the new node appears.
  let unlisten: UnlistenFn | null = null;
  listen<StandEvent>("stand-created", (e) => {
    stands = [{ ...e.payload, id: standSeq++ }, ...stands];
    void refreshGraph();
  }).then((u) => (unlisten = u));

  onDestroy(() => {
    unlisten?.();
    void invoke("stop_watching").catch(() => {});
  });

  async function openProduct() {
    error = null;
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Produkt öffnen",
    });
    if (typeof selected !== "string") return;
    loading = true;
    try {
      product = await invoke<ProductView>("open_product", { path: selected });
      productPath = selected;
      // A fresh product starts with a fresh ledger, then watching begins silently.
      stands = [];
      await invoke("start_watching", { path: selected });
      await refreshGraph();
    } catch (e) {
      error = String(e);
      product = null;
      productPath = null;
      graph = null;
    } finally {
      loading = false;
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
      <button class="key" onclick={openProduct} disabled={loading}>
        <span class="label">{loading ? "öffne …" : "Produkt öffnen"}</span>
      </button>
    </div>

    <div class="content">
      {#if error}
        <p class="notice mono">{error}</p>
      {:else if product}
        {#if product.bausteine.length > 0}
          <div class="grid">
            {#each product.bausteine as b, i (b.path)}
              <ArtifactCard baustein={b} index={i} />
            {/each}
          </div>
        {:else}
          <p class="notice label">Keine Bausteine in diesem Ordner gefunden</p>
        {/if}
      {:else}
        <div class="empty">
          <div class="empty-panel">
            <span class="label empty-hint">Ordner wählen — nur lesen</span>
            <button class="key big" onclick={openProduct}>
              <span class="label">Produkt öffnen</span>
            </button>
          </div>
        </div>
      {/if}
    </div>
    </main>

    {#if product}
      <VersionTree {graph} onPromote={promote} />
      <StandList {stands} />
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

  /* Work area + Stände rail share the row below the instrument display. */
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

  .notice {
    color: var(--ink-muted);
    font-size: 13px;
  }
</style>
