<script lang="ts">
  // Bibliothek-Editor (Issue #137, E50b) — die dritte Pfad-Klasse: Rekonstruierbar.
  // Jede Regel verfolgt NUR Quelle + gepinntes Manifest und ignoriert die rekonstruierbaren
  // Framework-Dateien (RekonstruierbarRegel in baustein.rs). Aus einer Regel wird ein Ignore-Muster
  // (das Framework) PLUS je Manifest eine Negation (`!west.yml`), die das Manifest verfolgt hält —
  // ehrliche „Quelle + rekonstruierendes Manifest", keine falsche Vollständigkeit.
  import type { RekonstruierbarRegel } from "$lib/types";
  import MusterListe from "./MusterListe.svelte";

  let { items = $bindable([]) }: { items: RekonstruierbarRegel[] } = $props();

  function add() {
    items = [...items, { framework: "", manifest: [] }];
  }
  function remove(i: number) {
    items = items.filter((_, j) => j !== i);
  }
  function setFramework(i: number, v: string) {
    const next = items.slice();
    next[i] = { ...next[i], framework: v };
    items = next;
  }
  // Die Manifest-Liste je Regel bindet über MusterListe; wir reichen den Setter durch.
  function setManifest(i: number, manifest: string[]) {
    const next = items.slice();
    next[i] = { ...next[i], manifest };
    items = next;
  }
</script>

<div class="rules">
  {#each items as r, i (i)}
    <div class="rule">
      <div class="head">
        <input
          class="mono in"
          value={r.framework}
          placeholder="rekonstruierbar  z.B. modules/  ·  .west/  ·  .pio/"
          oninput={(e) => setFramework(i, e.currentTarget.value)}
        />
        <button type="button" class="x" onclick={() => remove(i)} aria-label="Entfernen">✕</button>
      </div>
      <div class="manifest">
        <span class="mlabel label">gepinntes Manifest — bleibt verfolgt</span>
        <MusterListe
          items={r.manifest}
          placeholder="west.yml"
          onItems={(m: string[]) => setManifest(i, m)}
        />
      </div>
    </div>
  {/each}
  {#if items.length === 0}
    <p class="empty label">
      Keine Rekonstruierbar-Regel — der ganze Heimat-Ordner wird verfolgt.
    </p>
  {/if}
  <button type="button" class="add" onclick={add}>
    <span class="label">+ Rekonstruierbar</span>
  </button>
</div>

<style>
  .rules {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .rule {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--surface-sunken);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }
  .head {
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
  .x {
    appearance: none;
    cursor: pointer;
    border: 1px solid var(--hairline);
    background: var(--surface-base);
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
  .manifest {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-left: 2px;
  }
  .mlabel {
    color: var(--ink-muted);
    font-size: 9px;
  }
  .add {
    appearance: none;
    cursor: pointer;
    align-self: flex-start;
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
