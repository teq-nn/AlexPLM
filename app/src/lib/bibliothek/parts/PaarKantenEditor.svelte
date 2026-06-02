<script lang="ts">
  // Bibliothek-Editor (Issue #108) — Paar-Default-Kanten: a suggestion that fires only when a partner Baustein is also
  // in the stack (baustein.rs::PaarDefaultKante). partner_id is a dropdown over the other Bausteine
  // (self excluded). Dangling refs are tolerated (warned upstream, never blocked) — handoff §5.
  import type { PaarDefaultKante } from "$lib/types";

  let {
    items = $bindable([]),
    partners = [],
  }: {
    items: PaarDefaultKante[];
    /** {id,name} of every other Baustein — the dropdown source (self already excluded by caller). */
    partners: { id: string; name: string }[];
  } = $props();

  function add() {
    items = [...items, { partner_id: partners[0]?.id ?? "", derived_glob: "", source_glob: "" }];
  }
  function remove(i: number) {
    items = items.filter((_, j) => j !== i);
  }
  function patch(i: number, p: Partial<PaarDefaultKante>) {
    const next = items.slice();
    next[i] = { ...next[i], ...p };
    items = next;
  }
  function isDangling(id: string): boolean {
    return id !== "" && !partners.some((p) => p.id === id);
  }
</script>

<div class="paar">
  {#each items as k, i (i)}
    <div class="row">
      <div class="partnerwrap">
        <span class="plus mono" aria-hidden="true">+</span>
        <select
          class="partner mono"
          class:dangling={isDangling(k.partner_id)}
          value={k.partner_id}
          onchange={(e) => patch(i, { partner_id: e.currentTarget.value })}
        >
          {#if isDangling(k.partner_id)}
            <option value={k.partner_id}>{k.partner_id} (fehlt)</option>
          {/if}
          {#each partners as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      </div>
      <input
        class="mono in"
        value={k.derived_glob}
        placeholder="abgeleitet  z.B. *.pos"
        oninput={(e) => patch(i, { derived_glob: e.currentTarget.value })}
      />
      <span class="arrow mono" aria-hidden="true">←</span>
      <input
        class="mono in"
        value={k.source_glob}
        placeholder="Quelle  z.B. *.kicad_pcb"
        oninput={(e) => patch(i, { source_glob: e.currentTarget.value })}
      />
      <button type="button" class="x" onclick={() => remove(i)} aria-label="Entfernen">✕</button>
    </div>
  {/each}
  {#if items.length === 0}
    <p class="empty label">Keine Paar-Vorschläge — greift erst, wenn ein Partner-Baustein dazukommt.</p>
  {/if}
  <button type="button" class="add" onclick={add} disabled={partners.length === 0}>
    <span class="label">+ Paar-Vorschlag</span>
  </button>
</div>

<style>
  .paar {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .partnerwrap {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: none;
  }
  .plus {
    color: var(--ink-muted);
  }
  .partner {
    appearance: none;
    padding: 7px 10px;
    font-size: 12px;
    color: var(--ink-strong);
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .partner.dangling {
    border-color: var(--accent);
    color: var(--accent);
  }
  .in {
    flex: 1;
    min-width: 120px;
    padding: 7px 10px;
    font-size: 12px;
    color: var(--ink-strong);
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }
  .in:focus {
    outline: none;
    border-color: var(--ink-strong);
  }
  .arrow {
    flex: none;
    color: var(--ink-muted);
    font-size: 14px;
  }
  .x {
    appearance: none;
    cursor: pointer;
    border: 1px solid var(--hairline);
    background: var(--surface-sunken);
    color: var(--ink-muted);
    border-radius: var(--radius-sm);
    width: 26px;
    height: 28px;
    font-size: 10px;
    flex: none;
  }
  .x:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  .add {
    appearance: none;
    cursor: pointer;
    align-self: flex-start;
    margin-top: 2px;
    padding: 6px 12px;
    background: transparent;
    color: var(--ink-muted);
    border: 1px dashed var(--hairline);
    border-radius: var(--radius-sm);
  }
  .add:hover:not(:disabled) {
    color: var(--ink-strong);
    border-color: var(--ink-muted);
  }
  .add:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .add .label {
    color: inherit;
  }
  .empty {
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12px;
    margin: 2px 0;
  }
</style>
