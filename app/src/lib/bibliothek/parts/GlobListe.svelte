<script lang="ts">
  // Bibliothek-Editor (Issue #108) — the ordered Artefakt-Glob list. globs[0] is the Hauptdatei (highest priority,
  // baustein.rs). Reorderable (up/down), add, edit, remove. The Hauptdatei row is marked.
  let { globs = $bindable([]) }: { globs: string[] } = $props();

  function add() {
    globs = [...globs, ""];
  }
  function remove(i: number) {
    globs = globs.filter((_, j) => j !== i);
  }
  function move(i: number, dir: -1 | 1) {
    const j = i + dir;
    if (j < 0 || j >= globs.length) return;
    const next = globs.slice();
    [next[i], next[j]] = [next[j], next[i]];
    globs = next;
  }
  function set(i: number, v: string) {
    const next = globs.slice();
    next[i] = v;
    globs = next;
  }
</script>

<div class="globs">
  {#each globs as g, i (i)}
    <div class="row" class:haupt={i === 0}>
      <span class="rank mono">{i === 0 ? "Haupt" : i + 1}</span>
      <input
        class="mono in"
        value={g}
        placeholder="*.endung"
        oninput={(e) => set(i, e.currentTarget.value)}
      />
      <div class="ord">
        <button type="button" class="mini" disabled={i === 0} onclick={() => move(i, -1)} title="Höher" aria-label="Höher">↑</button>
        <button type="button" class="mini" disabled={i === globs.length - 1} onclick={() => move(i, 1)} title="Tiefer" aria-label="Tiefer">↓</button>
      </div>
      <button type="button" class="mini del" onclick={() => remove(i)} title="Entfernen" aria-label="Entfernen">✕</button>
    </div>
  {/each}
  {#if globs.length === 0}
    <p class="empty label">Noch keine Muster — das erste wird die Hauptdatei.</p>
  {/if}
  <button type="button" class="addrow" onclick={add}>
    <span class="label">+ Muster</span>
  </button>
</div>

<style>
  .globs {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .rank {
    flex: none;
    width: 44px;
    font-size: 10px;
    color: var(--ink-muted);
    text-align: right;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row.haupt .rank {
    color: var(--ink-strong);
  }
  .in {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    font-size: 12.5px;
    color: var(--ink-strong);
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }
  .row.haupt .in {
    border-color: var(--ink-muted);
    background: var(--surface-raised);
  }
  .in:focus {
    outline: none;
    border-color: var(--ink-strong);
  }
  .ord {
    display: flex;
    gap: 2px;
    flex: none;
  }
  .mini {
    appearance: none;
    cursor: pointer;
    width: 26px;
    height: 28px;
    display: grid;
    place-items: center;
    background: var(--surface-sunken);
    color: var(--ink-default);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    font-size: 12px;
    transition: background var(--dur) var(--ease);
  }
  .mini:hover:not(:disabled) {
    background: var(--surface-raised);
  }
  .mini:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .mini.del:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }
  .addrow {
    appearance: none;
    cursor: pointer;
    align-self: flex-start;
    margin-top: 2px;
    padding: 6px 12px;
    background: transparent;
    color: var(--ink-muted);
    border: 1px dashed var(--hairline);
    border-radius: var(--radius-sm);
    transition:
      color var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .addrow:hover {
    color: var(--ink-strong);
    border-color: var(--ink-muted);
  }
  .addrow .label {
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
