<script lang="ts">
  import { cmd } from "$lib/commands";
  import { open } from "@tauri-apps/plugin-dialog";
  import { readHistory, forget, seit } from "$lib/verlauf";
  import type {
    RegisteredProduct,
    SearchResult,
    SearchHit,
    HitField,
  } from "./types";

  // The produktübergreifende Live-Suche (Issue #45, E45). An app-level surface — the registry is
  // APP-LEVEL (it spans products), so the search lives in its own panel, not in any product's
  // chassis. It wears the WARM chassis idiom (same instrument language as the Produktliste popover
  // and the cards/keys) rather than the dark LCD screen, so it sits with the rest of the app
  // instead of breaking from it. Orange stays rationed — used ONLY for the honest offline notice
  // (an "Achtung: nicht alles durchsucht" attention state).
  let {
    onClose,
    onSwitch,
    currentPath = null,
  }: {
    onClose: () => void;
    /** Switch the open product by path — the registry rail doubles as the Produktliste switcher
        (Issue #108-Folge): the parent tears the old product down and opens this one, then the
        search surface closes so you land on the product. */
    onSwitch: (path: string) => void;
    /** Path of the currently open product, so the rail can mark it „offen" and skip a no-op switch. */
    currentPath?: string | null;
  } = $props();

  let products = $state<RegisteredProduct[]>([]);
  // The local „zuletzt geöffnet"-Reihenfolge (Verlauf, Issue #73): stamped from +page.svelte on
  // every open/import/switch, read here to sort the rail newest-first. Held reactively so removing
  // a product (which forgets its stamp) re-sorts at once.
  let history = $state(readHistory());
  let query = $state("");
  let result = $state<SearchResult | null>(null);
  let searching = $state(false);
  let error = $state<string | null>(null);
  // Debounce so each keystroke does not fan out over N product trees.
  let debounce: ReturnType<typeof setTimeout> | null = null;

  async function loadRegistry() {
    try {
      products = await cmd.listProducts();
    } catch (e) {
      error = String(e);
    }
  }
  // Load the registry as soon as the surface mounts.
  void loadRegistry();

  // Newest-opened first; entries never opened from here (no stamp) fall to the bottom, then
  // alphabetical by name so the order is stable. The open product is NOT pinned to the top — it is
  // marked instead (lit LED + „offen"), so its place in the Verlauf stays honest.
  const sorted = $derived.by(() => {
    const ts = (p: RegisteredProduct) => history[p.path] ?? 0;
    return [...products].sort((a, b) => {
      const d = ts(b) - ts(a);
      return d !== 0 ? d : a.name.localeCompare(b.name);
    });
  });

  async function runSearch() {
    const q = query.trim();
    if (!q) {
      result = null;
      return;
    }
    searching = true;
    error = null;
    try {
      result = await cmd.searchProducts(q);
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
      products = await cmd.registerProduct(selected);
      // A changed registry can change results — re-run any active query.
      if (query.trim()) void runSearch();
    } catch (e) {
      error = String(e);
    }
  }

  // Neu verknüpfen (Issue #89, PRD-US5): a moved product (folder renamed/moved outside the app)
  // points its registry entry at nothing and shows up offline. Rather than orphaning it, re-point
  // the entry to the chosen folder — the backend validates it is a plausible product and REPLACES
  // the old entry (never a duplicate). The display name is re-derived from the new path.
  async function relinkProduct(oldPath: string, productName: string) {
    const selected = await open({
      directory: true,
      multiple: false,
      title: `„${productName}" neu verknüpfen — neuen Ordner wählen`,
    });
    if (typeof selected !== "string") return;
    try {
      products = await cmd.relinkProduct(oldPath, selected);
      error = null;
      // A re-pointed entry can change results — re-run any active query so the freshly
      // reachable product drops out of the offline tally and into the hits.
      if (query.trim()) void runSearch();
    } catch (e) {
      // A dead re-link (folder unreachable / not a plausible product) is reported, never silent.
      error = String(e);
    }
  }

  // Pick a registry product → switch the open product to it (the rail is now the switcher, Issue
  // #108-Folge). Switching to the already-open product would needlessly tear it down and reopen it,
  // so only the close happens then. Either way the search surface steps aside so you land on the work.
  function pickProduct(p: RegisteredProduct) {
    if (p.path !== currentPath) onSwitch(p.path);
    onClose();
  }

  async function removeProduct(path: string) {
    try {
      products = await cmd.unregisterProduct(path);
      // Drop the local Verlauf stamp too, so a removed product cannot resurface ranked if re-added.
      history = forget(path);
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
                    <div class="offline-item">
                      <span class="offline-text" title={off.product_path}>
                        {off.product_name} — {off.reason}
                      </span>
                      <button
                        class="relink"
                        onclick={() =>
                          relinkProduct(off.product_path, off.product_name)}
                        title={`Registry-Eintrag auf den verschobenen Ordner neu verknüpfen (${off.product_path})`}
                        aria-label={`„${off.product_name}" neu verknüpfen`}
                        >Neu verknüpfen…</button
                      >
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

      <!-- Registry rail: the path-only product list, doubling as the Produktliste switcher — click a
           product to wechseln, register/unregister with + / ✕. The open one is marked, never switched. -->
      <aside class="registry" aria-label="Produkte — wechseln & Registry">
        <div class="reg-head">
          <span class="label title">Produkte</span>
          <button class="key add" onclick={addProduct}>
            <span class="label">+ Produkt</span>
          </button>
        </div>
        <div class="reg-list" role="menu" aria-label="Zu Produkt wechseln">
          {#if products.length === 0}
            <p class="idle mono">
              noch keine Produkte registriert — nur Pfade, kein Inhalt
            </p>
          {:else}
            {#each sorted as p (p.path)}
              {@const isCurrent = p.path === currentPath}
              {@const opened = seit(history[p.path])}
              <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
              <div
                class="reg-item"
                class:current={isCurrent}
                role="menuitem"
                tabindex="0"
                title={isCurrent
                  ? `${p.path} — offen`
                  : `Zu „${p.name}" wechseln (${p.path})`}
                onclick={() => pickProduct(p)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    pickProduct(p);
                  }
                }}
              >
                <span class="reg-dot" class:open={isCurrent} aria-hidden="true"></span>
                <div class="reg-body">
                  <div class="reg-name mono">{p.name}</div>
                  <div class="reg-path mono">{p.path}</div>
                </div>
                {#if isCurrent}
                  <span class="reg-badge label">offen</span>
                {:else if opened}
                  <!-- The local „zuletzt geöffnet"-Stempel that drives the sort, made legible —
                       a quiet relative time, never an absolute one (those belong on the dark
                       instrument displays). The open product shows „offen" instead. -->
                  <span class="reg-when mono" title="zuletzt geöffnet">{opened}</span>
                {/if}
                <button
                  class="iconbtn small"
                  onclick={(e) => {
                    e.stopPropagation();
                    removeProduct(p.path);
                  }}
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
  /* A softly dimmed warm backdrop; the surface is the warm chassis shelf (same instrument language
     as the Produktliste popover and the cards/keys), no longer the dark LCD screen. */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 28px;
    background: rgba(28, 26, 25, 0.42);
    animation: scrim-in 180ms var(--ease);
  }
  .screen {
    /* A modal, kept as-is — just smaller: ~70% of the window width (it used to open near-full).
       Capped at 1040px so it stays sane on very wide monitors, floored so it never gets cramped. */
    width: clamp(560px, 70vw, 1040px);
    height: min(740px, 100%);
    display: flex;
    flex-direction: column;
    background: var(--surface-raised);
    color: var(--ink-default);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.6) inset,
      0 24px 60px rgba(28, 26, 25, 0.28);
    overflow: hidden;
    animation: screen-in 220ms var(--ease) backwards;
  }

  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface-base);
  }
  .title-group {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .title {
    color: var(--ink-muted);
  }
  .sub {
    font-size: 11px;
    color: var(--ink-muted);
  }

  /* Recessed warm input row — a sunken chassis trough, hairline + soft inner shadow. */
  .searchrow {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 14px 16px 0;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--surface-sunken);
    border: 1px solid var(--hairline);
    box-shadow: inset 0 1px 2px rgba(28, 26, 25, 0.12);
  }
  .prompt {
    color: var(--ink-muted);
    font-weight: 600;
    font-size: 15px;
  }
  .query {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--ink-strong);
    font-size: 15px;
    letter-spacing: 0.01em;
    caret-color: var(--led-free);
  }
  .query::placeholder {
    color: var(--ink-muted);
  }
  .working {
    font-size: 11px;
    color: var(--ink-muted);
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
    color: var(--ink-muted);
    font-size: 12px;
    line-height: 1.55;
    padding: 6px 2px;
  }
  .idle .src {
    color: var(--ink-default);
  }
  .notice {
    color: var(--accent);
    font-size: 12px;
  }
  .count {
    color: var(--ink-muted);
    font-size: 11px;
    padding: 2px 2px 4px;
    border-bottom: 1px solid var(--hairline);
  }

  /* Honest offline notice — the rationed orange "attention" state. */
  .offline {
    display: flex;
    gap: 10px;
    padding: 11px 12px;
    border-radius: var(--radius);
    background: var(--surface-base);
    box-shadow: inset 0 0 0 1px rgba(240, 66, 28, 0.32);
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
    color: var(--accent);
  }
  .offline-list {
    margin-top: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .offline-item {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .offline-text {
    flex: 1;
    min-width: 0;
    font-size: 10px;
    color: var(--ink-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Neu verknüpfen — the deliberate re-point key. It lives inside the rationed-orange offline
     notice, so it wears a faint warm edge (attention context) rather than a second loud colour;
     the hover warms it without ever shouting. */
  .relink {
    appearance: none;
    cursor: pointer;
    flex: none;
    font-size: 10px;
    line-height: 1;
    padding: 5px 9px;
    border-radius: var(--radius-sm);
    color: var(--accent);
    background: rgba(240, 66, 28, 0.08);
    border: 1px solid rgba(240, 66, 28, 0.32);
    transition:
      background var(--dur) var(--ease),
      color var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .relink:hover {
    color: var(--accent-ink);
    background: rgba(240, 66, 28, 0.22);
    border-color: rgba(240, 66, 28, 0.5);
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
    color: var(--ink-strong);
  }
  .group-path {
    font-size: 10px;
    color: var(--ink-muted);
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
    background: var(--surface-base);
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  /* Source tag — small recessed chip; colour-codes the three searched sources. */
  .field {
    flex: none;
    align-self: flex-start;
    margin-top: 1px;
    padding: 3px 7px;
    border-radius: var(--radius-sm);
    font-size: 9px;
    background: var(--surface-sunken);
    color: var(--ink-muted);
  }
  .field[data-field="dateiname"] {
    color: var(--ink-strong);
  }
  .field[data-field="plm"] {
    color: var(--data-foreign);
  }
  .hit-body {
    min-width: 0;
    flex: 1;
  }
  .hit-text {
    font-size: 12px;
    color: var(--ink-strong);
    word-break: break-word;
  }
  .hit-file {
    margin-top: 2px;
    font-size: 10px;
    color: var(--ink-muted);
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
    border-left: 1px solid var(--hairline);
  }
  .reg-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 13px 14px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface-base);
  }
  .reg-head .title {
    color: var(--ink-muted);
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
  /* One product row — a recessive warm card doubling as a switch target. Hover/focus lifts it
     (background + hairline), exactly the Produktliste „.row" idiom; the open one is marked, not loud. */
  .reg-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition:
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .reg-item:hover,
  .reg-item:focus-visible {
    outline: none;
    background: var(--surface-base);
    border-color: var(--hairline);
  }
  /* The currently-open product: a calm seated state with a lit green LED + „offen", and held —
     clicking it would only tear down and reopen the same product, so it rests at the default cursor. */
  .reg-item.current {
    cursor: default;
    background: var(--surface-base);
    border-color: var(--hairline);
    box-shadow: inset 0 1px 2px rgba(28, 26, 25, 0.08);
  }
  /* Switch LED: a dim working dot at rest, lit green for the open product (same LED vocabulary as
     the hit-group dots — the green „offen" lamp the old Produktliste switcher used). */
  .reg-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--led-working);
    box-shadow: inset 0 0 0 1px rgba(28, 26, 25, 0.12);
  }
  .reg-dot.open {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }
  .reg-badge {
    flex: none;
    font-size: 9px;
    color: var(--ink-strong);
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--surface-raised);
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  /* The relative „zuletzt geöffnet"-Stempel — a quiet muted time at the row's right edge that
     explains the recency order without shouting. Sits where „offen" would for the open product. */
  .reg-when {
    flex: none;
    font-size: 9px;
    color: var(--ink-muted);
    white-space: nowrap;
  }
  .reg-body {
    min-width: 0;
    flex: 1;
  }
  .reg-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-strong);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reg-path {
    margin-top: 1px;
    font-size: 9px;
    color: var(--ink-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The "+ Produkt" key — a small warm chassis key. */
  .key.add {
    appearance: none;
    cursor: pointer;
    background: var(--surface-base);
    color: var(--ink-strong);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 6px 10px;
    transition: background var(--dur) var(--ease);
  }
  .key.add:hover {
    background: var(--surface-sunken);
  }

  .iconbtn {
    appearance: none;
    cursor: pointer;
    background: transparent;
    border: none;
    color: var(--ink-muted);
    font-size: 14px;
    line-height: 1;
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    transition: color var(--dur) var(--ease), background var(--dur) var(--ease);
  }
  .iconbtn:hover {
    color: var(--ink-strong);
    background: var(--surface-sunken);
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
