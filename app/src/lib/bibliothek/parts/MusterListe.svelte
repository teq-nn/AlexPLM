<script lang="ts">
  // Bibliothek-Editor (Issue #108) — a flat (unordered) list of patterns: used for Ignore + LFS muster.
  let {
    items = $bindable([]),
    placeholder = "*.endung",
  }: { items: string[]; placeholder?: string } = $props();

  function add() {
    items = [...items, ""];
  }
  function remove(i: number) {
    items = items.filter((_, j) => j !== i);
  }
  function set(i: number, v: string) {
    const next = items.slice();
    next[i] = v;
    items = next;
  }
</script>

<div class="list">
  {#each items as it, i (i)}
    <div class="chip">
      <input
        class="mono in"
        value={it}
        {placeholder}
        oninput={(e) => set(i, e.currentTarget.value)}
      />
      <button type="button" class="x" onclick={() => remove(i)} aria-label="Entfernen">✕</button>
    </div>
  {/each}
  <button type="button" class="add" onclick={add}>
    <span class="label">+</span>
  </button>
</div>

<style>
  .list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }
  .in {
    width: 120px;
    padding: 5px 8px;
    font-size: 12px;
    color: var(--ink-strong);
    background: transparent;
    border: 0;
  }
  .in:focus {
    outline: none;
  }
  .x {
    appearance: none;
    cursor: pointer;
    border: 0;
    border-left: 1px solid var(--hairline);
    background: transparent;
    color: var(--ink-muted);
    padding: 5px 8px;
    font-size: 10px;
  }
  .x:hover {
    color: var(--accent);
  }
  .add {
    appearance: none;
    cursor: pointer;
    width: 30px;
    height: 28px;
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
    font-size: 14px;
  }
</style>
