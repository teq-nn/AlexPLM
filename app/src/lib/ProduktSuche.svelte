<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import type {
    RegisteredProduct,
    SearchResult,
    SearchHit,
    HitField,
  } from "./types";

  // The produktübergreifende Live-Suche (Issue #45, E45). A full-screen instrument surface — the
  // registry is APP-LEVEL (it spans products), so the search lives in its own screen, not in any
  // product's chassis. The dark "screen" world (same LCD language as the VersionBar / fremde
  // Sperren) carries it: cross-product reach is a query against the instrument, not the warm
  // work chassis. Orange stays rationed — used ONLY for the honest offline notice (an
  // "Achtung: nicht alles durchsucht" attention state).
  let { onClose }: { onClose: () => void } = $props();

  let products = $state<RegisteredProduct[]>([]);
  let query = $state("");
  let result = $state<SearchResult | null>(null);
  let searching = $state(false);
  let error = $state<string | null>(null);
  // Debounce so each keystroke does not fan out over N product trees.
  let debounce: ReturnType<typeof setTimeout> | null = null;

  async function loadRegistry() {
    try {
      products = await invoke<RegisteredProduct[]>("list_products");
    } catch (e) {
      error = String(e);
    }
  }
  // Load the registry as soon as the surface mounts.
  void loadRegistry();

  async function runSearch() {
    const q = query.trim();
    if (!q) {
      result = null;
      return;
    }
    searching = true;
    error = null;
    try {
      result = await invoke<SearchResult>("search_products", { query: q });
    } catch (e) {
      error = String(e);
      result = null;
    } finally {
      searching = false;
    }
  }

  function onInput() {
    if (debounce !== null) clearTimeout(debounce);
    debounce = setTimeout(() => void runSearch(), 220);
  }

  async function addProduct() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Produkt zur Suche hinzufügen",
    });
    if (typeof selected !== "string") return;
    try {
      products = await invoke<RegisteredProduct[]>("register_product", {
        path: selected,
      });
      // A changed registry can change results — re-run any active query.
      if (query.trim()) void runSearch();
    } catch (e) {
      error = String(e);
    }
  }

  async function removeProduct(path: string) {
    try {
      products = await invoke<RegisteredProduct[]>("unregister_product", {
        path,
      });
      if (query.trim()) void runSearch();
    } catch (e) {
      error = String(e);
    }
  }

  // Group ranked hits by product, preserving best-first order (the first time a product appears
  // is its best hit, which fixes the group's position). Keeps the flat ranking honest while
  // letting the UI read product-by-product.
  const grouped = $derived.by(() => {
    if (!result) return [];
    const order: string[] = [];
    const byProduct = new Map<string, { name: string; hits: SearchHit[] }>();
    for (const h of result.hits) {
      if (!byProduct.has(h.product_path)) {
        byProduct.set(h.product_path, { name: h.product_name, hits: [] });
        order.push(h.product_path);
      }
      byProduct.get(h.product_path)!.hits.push(h);
    }
    return order.map((path) => ({ path, ...byProduct.get(path)! }));
  });

  const fieldLabel = (f: HitField): string =>
    f === "dateiname"
      ? "Datei"
      : f === "plm"
        ? "_plm"
        : "Notizen";

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div
  class="scrim"
  role="dialog"
  aria-modal="true"
  aria-label="Produktübergreifende Suche"
>
  <div class="screen">
    <!-- Title bar: the instrument's name + a registry tally + close. -->
    <header class="topbar">
      <div class="title-group">
        <span class="label title">Produktübergreifende Suche</span>
        <span class="sub mono"
          >{products.length.toString().padStart(2, "0")} Produkte registriert</span
        >
      </div>
      <button class="iconbtn" onclick={onClose} aria-label="Schließen">✕</button>
    </header>

    <!-- Search field: a recessed LCD input. Live fan-out on input (debounced). -->
    <div class="searchrow">
      <span class="prompt mono" aria-hidden="true">&gt;</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="query mono"
        type="text"
        autofocus
        spellcheck="false"
        placeholder="über alle erreichbaren Produkte suchen …"
        bind:value={query}
        oninput={onInput}
        aria-label="Suchbegriff"
      />
      {#if searching}
        <span class="working mono" aria-live="polite">suche …</span>
      {/if}
    </div>

    <div class="body">
      <!-- Results column -->
      <section class="results" aria-label="Suchergebnisse">
        {#if error}
          <p class="notice mono">{error}</p>
        {:else if !query.trim()}
          <p class="idle mono">
            Live-Fan-out über Dateinamen, <span class="src">_plm</span> und
            <span class="src">VERSION_NOTES.md</span> jedes erreichbaren Produkts.
            Kein zentraler Index.
          </p>
        {:else if result}
          <!-- Honest offline notice (E45): the one place orange is allowed here. Never silently
               drop unreachable products — name the count, name the products. -->
          {#if result.offline.length > 0}
            <div class="offline" role="status">
              <span class="dot" aria-hidden="true"></span>
              <div class="offline-body">
                <div class="offline-head mono">
                  {result.offline.length} von {result.total} offline, nicht durchsucht
                </div>
                <div class="offline-list mono">
                  {#each result.offline as off (off.product_path)}
                    <div class="offline-item" title={off.product_path}>
                      {off.product_name} — {off.reason}
                    </div>
                  {/each}
                </div>
              </div>
            </div>
          {/if}

          <div class="count mono">
            {result.hits.length} Treffer in {result.searched} durchsuchten Produkten
          </div>

          {#if grouped.length === 0}
            <p class="idle mono">keine Treffer</p>
          {:else}
            {#each grouped as g (g.path)}
              <div class="group">
                <div class="group-head" title={g.path}>
                  <span class="dot fresh" aria-hidden="true"></span>
                  <span class="group-name mono">{g.name}</span>
                  <span class="group-path mono">{g.path}</span>
                </div>
                {#each g.hits as h (h.field + h.file + h.text)}
                  <div class="hit">
                    <span class="field label" data-field={h.field}
                      >{fieldLabel(h.field)}</span
                    >
                    <div class="hit-body">
                      <div class="hit-text mono">{h.text}</div>
                      {#if h.field !== "dateiname"}
                        <div class="hit-file mono">{h.file}</div>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {/each}
          {/if}
        {/if}
      </section>

      <!-- Registry rail: register/unregister the path-only product list. -->
      <aside class="registry" aria-label="Produkt-Registry">
        <div class="reg-head">
          <span class="label title">Registry</span>
          <button class="key add" onclick={addProduct}>
            <span class="label">+ Produkt</span>
          </button>
        </div>
        <div class="reg-list">
          {#if products.length === 0}
            <p class="idle mono">
              noch keine Produkte registriert — nur Pfade, kein Inhalt
            </p>
          {:else}
            {#each products as p (p.path)}
              <div class="reg-item" title={p.path}>
                <div class="reg-body">
                  <div class="reg-name mono">{p.name}</div>
                  <div class="reg-path mono">{p.path}</div>
                </div>
                <button
                  class="iconbtn small"
                  onclick={() => removeProduct(p.path)}
                  aria-label={`${p.name} aus der Registry entfernen`}
                  title="Aus der Registry entfernen (Ordner bleibt unberührt)">✕</button
                >
              </div>
            {/each}
          {/if}
        </div>
      </aside>
    </div>
  </div>
</div>

<style>
  /* A dimmed backdrop; the surface itself is the dark instrument "screen" world. */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 28px;
    background: rgba(8, 7, 6, 0.62);
    animation: scrim-in 180ms var(--ease);
  }
  .screen {
    width: min(1040px, 100%);
    height: min(740px, 100%);
    display: flex;
    flex-direction: column;
    background: var(--screen-bg);
    color: var(--screen-fg);
    border: 1px solid #000;
    border-radius: var(--radius);
    box-shadow:
      0 30px 80px rgba(0, 0, 0, 0.55),
      inset 0 0 0 1px rgba(255, 255, 255, 0.03);
    overflow: hidden;
    animation: screen-in 220ms var(--ease) backwards;
  }

  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid #1c1a18;
  }
  .title-group {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .title {
    color: #8a857d;
  }
  .sub {
    font-size: 11px;
    color: #5f5b55;
  }

  /* Recessed LCD input row. */
  .searchrow {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 14px 16px 0;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: linear-gradient(180deg, #0b0a09, #131110);
    box-shadow:
      inset 0 2px 4px rgba(0, 0, 0, 0.9),
      inset 0 0 0 1px rgba(255, 255, 255, 0.03);
  }
  .prompt {
    color: var(--led-free);
    font-weight: 600;
    font-size: 15px;
  }
  .query {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--screen-fg);
    font-size: 15px;
    letter-spacing: 0.01em;
    caret-color: var(--led-free);
  }
  .query::placeholder {
    color: #57534d;
  }
  .working {
    font-size: 11px;
    color: #8a857d;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .results {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 14px 16px 22px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .idle {
    color: #6b6660;
    font-size: 12px;
    line-height: 1.55;
    padding: 6px 2px;
  }
  .idle .src {
    color: #8a857d;
  }
  .notice {
    color: var(--led-attention);
    font-size: 12px;
  }
  .count {
    color: #6b6660;
    font-size: 11px;
    padding: 2px 2px 4px;
    border-bottom: 1px solid #1c1a18;
  }

  /* Honest offline notice — the rationed orange "attention" state. */
  .offline {
    display: flex;
    gap: 10px;
    padding: 11px 12px;
    border-radius: var(--radius);
    background: linear-gradient(180deg, #1a1311, #0e0b0a);
    box-shadow: inset 0 0 0 1px rgba(240, 66, 28, 0.18);
  }
  .offline .dot {
    flex: none;
    margin-top: 3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 7px 1px color-mix(in srgb, var(--accent) 70%, transparent);
  }
  .offline-body {
    min-width: 0;
  }
  .offline-head {
    font-size: 12px;
    font-weight: 600;
    color: #f0a48f;
  }
  .offline-list {
    margin-top: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .offline-item {
    font-size: 10px;
    color: #8a857d;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* A product group of hits. */
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    animation: row-in 220ms var(--ease) backwards;
  }
  .group-head {
    display: flex;
    align-items: baseline;
    gap: 9px;
    padding: 8px 2px 4px;
  }
  .group-head .dot {
    align-self: center;
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--led-working);
  }
  .group-head .dot.fresh {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }
  .group-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--screen-fg);
  }
  .group-path {
    font-size: 10px;
    color: #57534d;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hit {
    display: flex;
    gap: 10px;
    margin-left: 16px;
    padding: 8px 11px;
    border-radius: var(--radius-sm);
    background: linear-gradient(180deg, #151312, #0c0b0a);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.025);
  }
  /* Source tag — small recessed chip; colour-codes the three searched sources. */
  .field {
    flex: none;
    align-self: flex-start;
    margin-top: 1px;
    padding: 3px 7px;
    border-radius: var(--radius-sm);
    font-size: 9px;
    background: #211f1d;
    color: #8a857d;
  }
  .field[data-field="dateiname"] {
    color: #cdc9c1;
  }
  .field[data-field="plm"] {
    color: #7fb0e0;
  }
  .hit-body {
    min-width: 0;
    flex: 1;
  }
  .hit-text {
    font-size: 12px;
    color: var(--screen-fg);
    word-break: break-word;
  }
  .hit-file {
    margin-top: 2px;
    font-size: 10px;
    color: #57534d;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Registry rail. */
  .registry {
    flex: none;
    width: 300px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-left: 1px solid #1c1a18;
  }
  .reg-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 13px 14px;
    border-bottom: 1px solid #1c1a18;
  }
  .reg-head .title {
    color: #8a857d;
  }
  .reg-list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .reg-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    background: linear-gradient(180deg, #151312, #0c0b0a);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.025);
  }
  .reg-body {
    min-width: 0;
    flex: 1;
  }
  .reg-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--screen-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reg-path {
    margin-top: 1px;
    font-size: 9px;
    color: #57534d;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The "+ Produkt" key, sized down for the dark rail. */
  .key.add {
    appearance: none;
    cursor: pointer;
    background: #211f1d;
    color: #cdc9c1;
    border: 1px solid #2d2a27;
    border-radius: var(--radius-sm);
    padding: 6px 10px;
    transition: background var(--dur) var(--ease);
  }
  .key.add:hover {
    background: #2a2724;
  }

  .iconbtn {
    appearance: none;
    cursor: pointer;
    background: transparent;
    border: none;
    color: #6b6660;
    font-size: 14px;
    line-height: 1;
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    transition: color var(--dur) var(--ease), background var(--dur) var(--ease);
  }
  .iconbtn:hover {
    color: var(--screen-fg);
    background: rgba(255, 255, 255, 0.05);
  }
  .iconbtn.small {
    flex: none;
    font-size: 11px;
    padding: 3px 5px;
  }

  @keyframes scrim-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes screen-in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.992);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
