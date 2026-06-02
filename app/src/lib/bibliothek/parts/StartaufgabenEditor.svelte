<script lang="ts">
  // Bibliothek-Editor (Issue #108) — Startaufgaben editor. Each row: titel + typ (Aufgabe|Hinweis) + blockiert.
  // A Hinweis never blocks (blockiert forced false), mirroring baustein.rs::AufgabenTyp.
  import type { Startaufgabe } from "$lib/types";

  let { items = $bindable([]) }: { items: Startaufgabe[] } = $props();

  function add() {
    items = [...items, { titel: "", typ: "aufgabe", blockiert: false }];
  }
  function remove(i: number) {
    items = items.filter((_, j) => j !== i);
  }
  function patch(i: number, p: Partial<Startaufgabe>) {
    const next = items.slice();
    next[i] = { ...next[i], ...p };
    if (next[i].typ === "hinweis") next[i].blockiert = false; // Hinweis blockiert nie
    items = next;
  }
</script>

<div class="tasks">
  {#each items as t, i (i)}
    <div class="row">
      <input
        class="in titel"
        value={t.titel}
        placeholder="Titel der Aufgabe / des Hinweises"
        oninput={(e) => patch(i, { titel: e.currentTarget.value })}
      />
      <div class="typ">
        <button type="button" class="t" class:on={t.typ === "aufgabe"} onclick={() => patch(i, { typ: "aufgabe" })}>
          <span class="label">Aufgabe</span>
        </button>
        <button type="button" class="t" class:on={t.typ === "hinweis"} onclick={() => patch(i, { typ: "hinweis" })}>
          <span class="label">Hinweis</span>
        </button>
      </div>
      <label class="block" class:disabled={t.typ === "hinweis"} title="Blockiert das Freigabe-Gate">
        <input
          type="checkbox"
          checked={t.blockiert}
          disabled={t.typ === "hinweis"}
          onchange={(e) => patch(i, { blockiert: e.currentTarget.checked })}
        />
        <span class="label">blockiert</span>
      </label>
      <button type="button" class="x" onclick={() => remove(i)} aria-label="Entfernen">✕</button>
    </div>
  {/each}
  {#if items.length === 0}
    <p class="empty label">Keine Startaufgaben — beim Onboarding wird nichts angelegt.</p>
  {/if}
  <button type="button" class="add" onclick={add}>
    <span class="label">+ Startaufgabe</span>
  </button>
</div>

<style>
  .tasks {
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
  .in {
    flex: 1;
    min-width: 160px;
    padding: 7px 10px;
    font-size: 12.5px;
    font-family: var(--font-label);
    color: var(--ink-strong);
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }
  .in:focus {
    outline: none;
    border-color: var(--ink-strong);
  }
  .typ {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    background: var(--surface-sunken);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    flex: none;
  }
  .t {
    appearance: none;
    cursor: pointer;
    border: 0;
    background: transparent;
    color: var(--ink-muted);
    padding: 4px 10px;
    border-radius: 2px;
  }
  .t.on {
    background: var(--key-light);
    color: var(--ink-strong);
  }
  .t .label {
    color: inherit;
  }
  .block {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    cursor: pointer;
    color: var(--ink-default);
    flex: none;
  }
  .block.disabled {
    opacity: 0.4;
    cursor: default;
  }
  .block .label {
    color: inherit;
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
