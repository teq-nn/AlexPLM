<script lang="ts">
  // Bibliothek-Editor (Issue #108) — interne Default-Kanten: derived_glob "stammt aus" source_glob (baustein.rs).
  // Read as „abgeleitet ← Quelle". Within this Baustein's own Heimat.
  import type { DefaultKante } from "$lib/types";

  let { items = $bindable([]) }: { items: DefaultKante[] } = $props();

  function add() {
    items = [...items, { derived_glob: "", source_glob: "" }];
  }
  function remove(i: number) {
    items = items.filter((_, j) => j !== i);
  }
  function patch(i: number, p: Partial<DefaultKante>) {
    const next = items.slice();
    next[i] = { ...next[i], ...p };
    items = next;
  }
</script>

<div class="kanten">
  {#each items as k, i (i)}
    <div class="row">
      <input
        class="mono in"
        value={k.derived_glob}
        placeholder="abgeleitet  z.B. *.stl"
        oninput={(e) => patch(i, { derived_glob: e.currentTarget.value })}
      />
      <span class="arrow mono" aria-hidden="true">←</span>
      <input
        class="mono in"
        value={k.source_glob}
        placeholder="Quelle  z.B. *.f3d"
        oninput={(e) => patch(i, { source_glob: e.currentTarget.value })}
      />
      <button type="button" class="x" onclick={() => remove(i)} aria-label="Entfernen">✕</button>
    </div>
  {/each}
  {#if items.length === 0}
    <p class="empty label">Keine Kanten — nichts wird automatisch als „abgeleitet aus" verknüpft.</p>
  {/if}
  <button type="button" class="add" onclick={add}>
    <span class="label">+ Kante</span>
  </button>
</div>

<style>
  .kanten {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .in {
    flex: 1;
    min-width: 0;
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
  .add:hover {
    color: var(--ink-strong);
    border-color: var(--ink-muted);
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
