<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { BibliothekView, Baustein, ProduktStack } from "./types";

  // Werkzeugkasten einrichten (Issue #50): pick a Standard-Werkzeugkasten from the Bibliothek and
  // tune it Baustein-by-Baustein, then materialise it as the product's self-contained Produkt-Stack
  // (anti-drift copy, ADR 0003). Two modes:
  //   • anlegen   — the product has no Werkzeugkasten yet: choose a standard, extend/trim, confirm.
  //   • erweitern — additive only: already-copied Bausteine stay verbatim (no silent version bump);
  //                 the user just adds further ones.
  // No git terminology in visible text (PRD §49) — domain language only.
  let {
    productPath,
    mode,
    stack,
    onConfirmed,
    onClose,
  }: {
    productPath: string;
    mode: "anlegen" | "erweitern";
    /** The product's current Produkt-Stack (empty in „anlegen", populated in „erweitern"). */
    stack: ProduktStack | null;
    /** Bubble the freshly written stack up so the shell can re-derive the Werkbank. */
    onConfirmed: (s: ProduktStack) => void;
    onClose: () => void;
  } = $props();

  // The ids already copied into this product — locked in „erweitern" (anti-drift: kept verbatim,
  // never re-pulled). Stilllegen of an existing Baustein is a separate concern (#51).
  let vorhanden = $derived(new Set((stack?.bausteine ?? []).map((b) => b.id)));

  let lib = $state<BibliothekView | null>(null);
  // The selection: ids the user has ticked. In „erweitern" this holds only the *new* additions;
  // the „vorhanden" ones are shown locked and are never in this set.
  let chosen = $state<Set<string>>(new Set());
  // Which standard Werkzeugkasten seeded the current selection (display name → stored on the stack).
  let basis = $state<string | null>(null);

  let busy = $state(false);
  let error = $state<string | null>(null);

  // Load the Bibliothek once. A hiccup is shown, not swallowed — the user can retry by reopening.
  $effect(() => {
    void (async () => {
      try {
        lib = await invoke<BibliothekView>("list_bibliothek");
      } catch (e) {
        error = String(e);
      }
    })();
  });

  // The selectable Bausteine: live (not stillgelegt). In „anlegen" all are offered; in „erweitern"
  // the already-present ones render separately as locked chips, so we keep them out of the toggle grid.
  let auswahlbar = $derived(
    (lib?.bausteine ?? []).filter((b) => !b.stillgelegt),
  );

  function istVorhanden(id: string): boolean {
    return mode === "erweitern" && vorhanden.has(id);
  }

  // Pick a standard Werkzeugkasten: it seeds the tick set with that stack's Bausteine (skipping any
  // already-present in „erweitern"). The user may then add/remove freely — the standard is a start,
  // not a cage.
  function waehleStandard(id: string, name: string, bausteinIds: string[]) {
    basis = name;
    const next = new Set<string>();
    for (const bid of bausteinIds) {
      if (!istVorhanden(bid)) next.add(bid);
    }
    chosen = next;
  }

  function toggle(id: string) {
    if (istVorhanden(id)) return; // locked: anti-drift copy stays
    const next = new Set(chosen);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    chosen = next;
    // A hand-edit away from the seeded standard means the selection is no longer purely that standard.
    // We keep the display name only when it still describes the basis; otherwise it reads as „eigene
    // Auswahl" on confirm (basis stays as the provenance hint of where they started).
  }

  // In „anlegen" an empty selection is allowed (a minimal but valid, openable product). In
  // „erweitern" at least one new Baustein must be ticked for the action to mean anything.
  let kannBestaetigen = $derived(mode === "anlegen" || chosen.size > 0);

  async function bestaetigen() {
    error = null;
    busy = true;
    try {
      let result: ProduktStack;
      if (mode === "anlegen") {
        result = await invoke<ProduktStack>("create_product_stack_cmd", {
          product: productPath,
          bausteinIds: [...chosen],
          toolstack: basis,
        });
      } else {
        result = await invoke<ProduktStack>("extend_product_stack_cmd", {
          product: productPath,
          bausteinIds: [...chosen],
        });
      }
      onConfirmed(result);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Heimat label for a Baustein card — the Arbeitsbereich it settles into.
  function heimatOf(b: Baustein): string {
    return b.heimat || "—";
  }
</script>

<div class="scrim" role="presentation" onclick={() => !busy && onClose()}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="stack-title"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="head">
      <span class="label kicker"
        >{mode === "anlegen" ? "Werkzeugkasten anlegen" : "Werkzeugkasten erweitern"}</span
      >
      <h2 id="stack-title" class="title">
        {mode === "anlegen" ? "Werkzeuge wählen" : "Werkzeuge ergänzen"}
      </h2>
      <p class="sub label">
        {#if mode === "anlegen"}
          Standard wählen, nach Bedarf anpassen. Die Auswahl wird als eigene Kopie ins Produkt
          übernommen — spätere Bibliotheks-Änderungen verändern dieses Produkt nicht.
        {:else}
          Vorhandene Werkzeuge bleiben unverändert. Ergänze einzelne Bausteine additiv.
        {/if}
      </p>
    </header>

    <div class="body">
      {#if mode === "anlegen" && (lib?.toolstacks?.length ?? 0) > 0}
        <!-- Standard-Werkzeugkästen: a start, not a cage. Picking one seeds the Baustein ticks. -->
        <div class="section">
          <span class="label sk">Standard</span>
          <div class="stacks">
            {#each lib?.toolstacks ?? [] as t (t.id)}
              <button
                class="stackchip"
                class:active={basis === t.name}
                onclick={() => waehleStandard(t.id, t.name, t.baustein_ids)}
                disabled={busy}
              >
                <span class="sc-name label">{t.name}</span>
                <span class="sc-count mono">{t.baustein_ids.length}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Already-present Bausteine (erweitern only): locked, kept verbatim (anti-drift). -->
      {#if mode === "erweitern" && (stack?.bausteine?.length ?? 0) > 0}
        <div class="section">
          <span class="label sk">Vorhanden</span>
          <div class="grid">
            {#each stack?.bausteine ?? [] as b (b.id)}
              <div class="bcard locked" aria-disabled="true">
                <span class="dot on" aria-hidden="true"></span>
                <span class="b-main">
                  <span class="b-name label">{b.name}</span>
                  <span class="b-heimat mono">→ {heimatOf(b)}</span>
                </span>
                <span class="b-tag label">vorhanden</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- The toggle grid: every live Baustein in the Bibliothek. -->
      <div class="section">
        <span class="label sk">{mode === "erweitern" ? "Hinzufügen" : "Bausteine"}</span>
        {#if lib === null && !error}
          <p class="empty label">lade Bibliothek …</p>
        {:else}
          <div class="grid">
            {#each auswahlbar as b (b.id)}
              {#if istVorhanden(b.id)}
                <!-- skip — rendered above as „vorhanden" -->
              {:else}
                <button
                  class="bcard"
                  class:on={chosen.has(b.id)}
                  onclick={() => toggle(b.id)}
                  disabled={busy}
                  aria-pressed={chosen.has(b.id)}
                >
                  <span class="dot" class:on={chosen.has(b.id)} aria-hidden="true"></span>
                  <span class="b-main">
                    <span class="b-name label">{b.name}</span>
                    <span class="b-heimat mono">→ {heimatOf(b)}</span>
                  </span>
                  <span class="b-id mono">{b.id}</span>
                </button>
              {/if}
            {/each}
          </div>
          {#if auswahlbar.length === 0}
            <p class="empty label">Keine Bausteine in der Bibliothek</p>
          {/if}
        {/if}
      </div>

      {#if error}
        <div class="err" role="alert">
          <span class="dot warn" aria-hidden="true"></span>
          <span class="err-text label">{error}</span>
        </div>
      {/if}
    </div>

    <footer class="foot">
      <span class="count mono" aria-live="polite">
        {#if mode === "anlegen"}
          {chosen.size} gewählt
        {:else}
          {chosen.size} neu · {vorhanden.size} vorhanden
        {/if}
      </span>
      <div class="keys">
        <button class="key ghost" onclick={onClose} disabled={busy}>
          <span class="label">Abbrechen</span>
        </button>
        <button class="key go" onclick={bestaetigen} disabled={!kannBestaetigen || busy}>
          <span class="label">
            {#if busy}
              richte ein …
            {:else if mode === "anlegen"}
              Werkzeugkasten anlegen
            {:else}
              Hinzufügen
            {/if}
          </span>
        </button>
      </div>
    </footer>
  </section>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(8, 7, 6, 0.62);
    backdrop-filter: blur(2px);
    animation: scrim-in 160ms var(--ease);
  }
  @keyframes scrim-in {
    from {
      opacity: 0;
    }
  }

  .panel {
    width: min(620px, 100%);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 24px 60px -16px rgba(8, 7, 6, 0.6),
      0 2px 0 rgba(255, 255, 255, 0.5) inset;
    overflow: hidden;
    animation: panel-in 200ms var(--ease) backwards;
  }
  @keyframes panel-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.99);
    }
  }

  .head {
    padding: 20px 22px 6px;
    flex: none;
  }
  .kicker {
    color: var(--ink-muted);
    display: block;
    margin-bottom: 6px;
  }
  .title {
    margin: 0;
    font-family: var(--font-label);
    font-weight: 700;
    font-size: 22px;
    letter-spacing: -0.01em;
    color: var(--ink-strong);
  }
  .sub {
    margin: 8px 0 0;
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12px;
    line-height: 1.45;
  }

  .body {
    padding: 8px 22px 18px;
    overflow-y: auto;
  }
  .section {
    margin-top: 14px;
  }
  .sk {
    display: block;
    color: var(--ink-muted);
    font-size: 10px;
    margin-bottom: 8px;
  }

  /* Standard-Werkzeugkasten chips: a thin instrument row. Active chip lit like the ceremony step. */
  .stacks {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .stackchip {
    appearance: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--hairline);
    background: var(--surface-sunken);
    color: var(--ink-default);
    transition:
      border-color var(--dur) var(--ease),
      background var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .stackchip:hover:not(:disabled) {
    background: var(--surface-raised);
  }
  .stackchip.active {
    border-color: var(--ink-strong);
    background: var(--key-light);
    color: var(--ink-strong);
  }
  .sc-name {
    font-size: 11px;
  }
  .sc-count {
    font-size: 11px;
    color: var(--ink-muted);
    padding: 1px 6px;
    border-radius: 99px;
    background: rgba(28, 26, 25, 0.06);
  }
  .stackchip.active .sc-count {
    color: var(--ink-strong);
  }

  /* Baustein toggle cards: a grid of instrument keys. Selected lights an LED + the hairline. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 8px;
  }
  .bcard {
    appearance: none;
    text-align: left;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--hairline);
    background: var(--surface-sunken);
    color: var(--ink-default);
    transition:
      border-color var(--dur) var(--ease),
      background var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
  }
  .bcard:hover:not(:disabled):not(.locked) {
    background: var(--surface-raised);
  }
  .bcard.on {
    border-color: var(--ink-strong);
    background: var(--surface-raised);
    box-shadow: inset 0 0 0 1px var(--ink-strong);
  }
  .bcard.locked {
    cursor: default;
    opacity: 0.78;
  }
  .b-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .b-name {
    font-size: 11.5px;
    color: var(--ink-strong);
  }
  .b-heimat {
    font-size: 10.5px;
    color: var(--ink-muted);
  }
  .b-id {
    font-size: 10px;
    color: var(--ink-muted);
    opacity: 0.7;
  }
  .b-tag {
    font-size: 9px;
    color: var(--led-free);
    flex: none;
  }

  /* LED dots — off (rim only) → lit green when selected/present, echoing the lock LEDs. */
  .dot {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--led-off);
    transition:
      background var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
  }
  .dot.on {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }
  .dot.warn {
    margin-top: 1px;
    background: var(--led-attention);
    box-shadow: 0 0 6px rgba(240, 66, 28, 0.4);
  }

  .empty {
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12px;
  }

  .err {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-top: 14px;
    padding: 11px 13px;
    border: 1px solid var(--hairline);
    border-left: 3px solid var(--led-attention);
    border-radius: var(--radius);
    background: var(--surface-base);
  }
  .err-text {
    color: var(--ink-default);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12.5px;
    line-height: 1.45;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 13px 22px 18px;
    border-top: 1px solid var(--hairline);
    flex: none;
  }
  .count {
    font-size: 11px;
    color: var(--ink-muted);
  }
  .keys {
    display: flex;
    gap: 10px;
  }

  .key {
    appearance: none;
    cursor: pointer;
    background: var(--key-light);
    color: var(--ink-strong);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 9px 15px;
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.12);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease),
      opacity var(--dur) var(--ease);
  }
  .key .label {
    color: inherit;
  }
  .key:hover:not(:disabled) {
    background: #f5f3ee;
  }
  .key:active:not(:disabled) {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.12);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.5;
    box-shadow: none;
  }
  .key.ghost {
    background: transparent;
    box-shadow: none;
    color: var(--ink-default);
  }
  .key.ghost:hover:not(:disabled) {
    background: var(--surface-sunken);
  }
  .key.go {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
  }
  .key.go:hover:not(:disabled) {
    background: #2a2724;
  }
</style>
