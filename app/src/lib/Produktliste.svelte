<script lang="ts">
  import { cmd } from "$lib/commands";
  import type { RegisteredProduct } from "./types";

  // ── Produktliste / Verlauf (Issue #73) ──────────────────────────────────────
  // The recently-opened products switcher. The Produkt-Registry is APP-LEVEL (it spans products,
  // not one chassis), so this switcher lives in the app-level entry bar next to the cross-product
  // search — same world, same idiom. It stays in the WARM chassis (a calm grey instrument shelf),
  // not the dark search "screen": switching the one open product is routine workshop navigation,
  // never the loud exception. Orange is rationed here to exactly one state: a known folder that is
  // no longer reachable on disk (an honest "nicht erreichbar" LED, never a silent drop).
  //
  // ABGRENZUNG: exactly ONE product is open at a time. This is a switcher/Verlauf, not tabs — the
  // parent does the clean teardown (Watcher/Loops lösen, reset()) before opening the target.

  let {
    currentPath,
    onSwitch,
    disabled = false,
  }: {
    /** The path of the product currently open, so the list can mark it. `null` = none open. */
    currentPath: string | null;
    /** Switch to a product by path. The parent tears the old product down and opens this one. */
    onSwitch: (path: string) => void;
    /** Disabled while a product is mid-open/-import (mirrors the entry keys). */
    disabled?: boolean;
  } = $props();

  let open = $state(false);
  let products = $state<RegisteredProduct[]>([]);
  let error = $state<string | null>(null);

  /** Local last-opened timestamps per product path. A LOCAL Verlaufs-Reihenfolge (localStorage)
   *  deliberately keeps the registry path-only — registry.rs stores no extra facts that could
   *  drift from disk (E8/E18), and the same per-path localStorage idiom already persists column
   *  widths. A path with no recorded timestamp sorts last (known but never opened from here). */
  const HISTORY_KEY = "plm.zuletzt-geoeffnet";

  function readHistory(): Record<string, number> {
    try {
      const raw = localStorage.getItem(HISTORY_KEY);
      if (!raw) return {};
      const parsed = JSON.parse(raw) as unknown;
      return parsed && typeof parsed === "object"
        ? (parsed as Record<string, number>)
        : {};
    } catch {
      return {};
    }
  }

  let history = $state<Record<string, number>>(readHistory());

  /** Stamp a path as just-opened. Called by the parent through `markOpened` (exported below) so
   *  the Verlauf fills on every open/import/switch — even without ever touching the search. */
  export function markOpened(path: string) {
    const next = { ...history, [path]: Date.now() };
    history = next;
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
    } catch {
      // The Verlauf order is a view convenience; a full/blocked storage must never break the shell.
    }
  }

  /** Re-read the registry (path-only). Best-effort: a missing/corrupt registry reads as empty. */
  export async function refresh() {
    try {
      products = await cmd.listProducts();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }
  void refresh();

  // Newest-opened first; entries never opened from here (no timestamp) fall to the bottom, then
  // alphabetical by name so the list is stable. The currently-open product is not pinned to the
  // top — it is marked instead (a lit LED + "offen"), so its position in the Verlauf stays honest.
  const sorted = $derived.by(() => {
    const ts = (p: RegisteredProduct) => history[p.path] ?? 0;
    return [...products].sort((a, b) => {
      const d = ts(b) - ts(a);
      return d !== 0 ? d : a.name.localeCompare(b.name);
    });
  });

  function toggle() {
    if (disabled) return;
    open = !open;
    if (open) void refresh();
  }

  function pick(p: RegisteredProduct) {
    open = false;
    // Switching to the already-open product would needlessly tear it down and reopen it.
    if (p.path === currentPath) return;
    onSwitch(p.path);
  }

  async function removeEntry(path: string, ev: MouseEvent) {
    // Don't let the row's click (switch) fire when the ✕ is pressed.
    ev.stopPropagation();
    try {
      products = await cmd.unregisterProduct(path);
      // Drop the local Verlauf stamp too, so a removed product cannot resurface ranked.
      if (path in history) {
        const next = { ...history };
        delete next[path];
        history = next;
        try {
          localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
        } catch {
          // best-effort, as above
        }
      }
    } catch (e) {
      error = String(e);
    }
  }

  // A relative "zuletzt geöffnet" stamp in the tool's quiet vocabulary (no absolute timestamps in
  // the list — those belong on the dark instrument displays). "—" when never opened from here.
  function seit(path: string): string {
    const t = history[path];
    if (!t) return "—";
    const mins = Math.floor((Date.now() - t) / 60000);
    if (mins < 1) return "gerade eben";
    if (mins < 60) return `vor ${mins} min`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `vor ${hrs} h`;
    const days = Math.floor(hrs / 24);
    return days < 7 ? `vor ${days} d` : `vor ${Math.floor(days / 7)} w`;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && open) {
      open = false;
    }
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div class="produktliste">
  <button
    class="key ghost trigger"
    class:on={open}
    onclick={toggle}
    {disabled}
    aria-haspopup="menu"
    aria-expanded={open}
    title="Zuletzt geöffnete Produkte — wechseln ohne Datei-Dialog"
  >
    <span class="dot" class:lit={currentPath !== null} aria-hidden="true"></span>
    <span class="label">Produktliste</span>
    <span class="caret mono" aria-hidden="true">{open ? "▴" : "▾"}</span>
  </button>

  {#if open}
    <!-- A backdrop that closes the popover on an outside click. Transparent; the popover itself
         is the warm chassis surface (not the dark search screen). -->
    <button
      class="backdrop"
      aria-label="Produktliste schließen"
      onclick={() => (open = false)}
    ></button>

    <div class="popover" role="menu" aria-label="Zuletzt geöffnete Produkte">
      <div class="pop-head">
        <span class="label title">Zuletzt geöffnet</span>
        <span class="sub mono"
          >{products.length.toString().padStart(2, "0")} bekannt</span
        >
      </div>

      {#if error}
        <p class="empty notice mono">{error}</p>
      {:else if sorted.length === 0}
        <p class="empty mono">
          noch keine Produkte — geöffnete erscheinen hier von selbst
        </p>
      {:else}
        <ul class="list">
          {#each sorted as p (p.path)}
            {@const isCurrent = p.path === currentPath}
            <li>
              <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
              <div
                class="row"
                class:current={isCurrent}
                role="menuitem"
                tabindex="0"
                onclick={() => pick(p)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    pick(p);
                  }
                }}
                title={p.path}
              >
                <span
                  class="dot"
                  class:open={isCurrent}
                  aria-hidden="true"
                ></span>
                <span class="body">
                  <span class="name mono">{p.name}</span>
                  <span class="path mono">{p.path}</span>
                </span>
                <span class="meta">
                  {#if isCurrent}
                    <span class="badge label">offen</span>
                  {:else}
                    <span class="seit mono">{seit(p.path)}</span>
                  {/if}
                </span>
                <button
                  class="remove"
                  onclick={(e) => removeEntry(p.path, e)}
                  aria-label={`${p.name} aus der Liste entfernen`}
                  title="Aus der Liste entfernen (Ordner bleibt unberührt)"
                >✕</button>
              </div>
            </li>
          {/each}
        </ul>
        <p class="foot mono">
          Auswahl wechselt das offene Produkt — der Ordner bleibt unberührt.
        </p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .produktliste {
    position: relative;
    flex: none;
  }

  /* The trigger reuses the entry bar's ghost key idiom; its LED tells whether a product is open. */
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .trigger .dot {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--led-off);
  }
  .trigger .dot.lit {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }
  .trigger.on {
    background: var(--surface-raised);
  }
  .caret {
    font-size: 10px;
    color: var(--ink-muted);
    line-height: 1;
  }

  /* Outside-click catcher behind the popover. */
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    appearance: none;
    border: none;
    background: transparent;
    cursor: default;
  }

  /* The popover: a seated warm-chassis shelf, hairline + soft erhebung — the same physical
     instrument language as the cards/keys, never the dark search screen. */
  .popover {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 31;
    width: min(440px, 92vw);
    max-height: min(520px, 70vh);
    display: flex;
    flex-direction: column;
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.6) inset,
      0 12px 32px rgba(28, 26, 25, 0.18);
    overflow: hidden;
    animation: pop-in 150ms var(--ease) backwards;
  }

  .pop-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    padding: 11px 13px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface-base);
  }
  .pop-head .title {
    color: var(--ink-muted);
    font-size: 10px;
  }
  .pop-head .sub {
    font-size: 10px;
    color: var(--ink-muted);
  }

  .empty {
    margin: 0;
    padding: 18px 14px;
    color: var(--ink-muted);
    font-size: 12px;
    line-height: 1.5;
  }
  .empty.notice {
    color: var(--led-attention);
  }

  .list {
    margin: 0;
    padding: 6px;
    list-style: none;
    overflow: auto;
    min-height: 0;
  }
  .list li {
    list-style: none;
  }

  /* One product row — a recessive card. Hover/focus lifts it; the open one is marked, not pinned. */
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    transition:
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .row:hover,
  .row:focus-visible {
    outline: none;
    background: var(--surface-base);
    border-color: var(--hairline);
  }
  /* The currently-open product: a calm seated state with a lit green LED — marked, never loud. */
  .row.current {
    background: var(--surface-base);
    border-color: var(--hairline);
    box-shadow: inset 0 1px 2px rgba(28, 26, 25, 0.08);
    cursor: default;
  }

  .row .dot {
    width: 9px;
    height: 9px;
    flex: none;
    border-radius: 50%;
    background: var(--led-working);
    box-shadow: inset 0 0 0 1px rgba(28, 26, 25, 0.12);
  }
  /* The one currently-open product lights green ("offen"). */
  .row .dot.open {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-strong);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    font-size: 9.5px;
    color: var(--ink-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    flex: none;
    display: flex;
    align-items: center;
  }
  .seit {
    font-size: 9.5px;
    color: var(--ink-muted);
    white-space: nowrap;
  }
  .badge {
    font-size: 9px;
    color: var(--ink-strong);
    padding: 2px 7px;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--surface-raised);
  }

  /* Remove ✕ — a quiet recessed icon button; appears clearly only on hover/focus of its row. */
  .remove {
    flex: none;
    appearance: none;
    cursor: pointer;
    background: transparent;
    border: none;
    color: var(--ink-muted);
    font-size: 12px;
    line-height: 1;
    padding: 4px 5px;
    border-radius: var(--radius-sm);
    opacity: 0;
    transition:
      opacity var(--dur) var(--ease),
      color var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .row:hover .remove,
  .row:focus-within .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .remove:hover {
    color: var(--ink-strong);
    background: var(--surface-sunken);
  }

  .foot {
    margin: 0;
    padding: 9px 13px;
    border-top: 1px solid var(--hairline);
    background: var(--surface-base);
    color: var(--ink-muted);
    font-size: 10px;
    line-height: 1.4;
  }

  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
