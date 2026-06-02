<script lang="ts">
  // Bibliothek-Editor (Issue #108) — 3-way segmented "Öffnen-Aktion". Auto / Datei / Ordner (baustein.rs::Oeffnen).
  import type { Oeffnen } from "$lib/types";

  let { value = $bindable("auto") }: { value: Oeffnen } = $props();

  const choices: { v: Oeffnen; label: string; hint: string }[] = [
    { v: "auto", label: "Auto", hint: "Dominante Datei → diese, sonst Ordner" },
    { v: "datei", label: "Datei", hint: "Immer die Hauptdatei öffnen" },
    { v: "ordner", label: "Ordner", hint: "Immer den Heimat-Ordner öffnen" },
  ];
</script>

<div class="seg" role="radiogroup" aria-label="Öffnen-Aktion">
  {#each choices as c (c.v)}
    <button
      type="button"
      class="opt"
      class:on={value === c.v}
      role="radio"
      aria-checked={value === c.v}
      title={c.hint}
      onclick={() => (value = c.v)}
    >
      <span class="label">{c.label}</span>
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    background: var(--surface-sunken);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
  }
  .opt {
    appearance: none;
    cursor: pointer;
    border: 0;
    background: transparent;
    color: var(--ink-muted);
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    transition:
      background var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .opt:hover:not(.on) {
    color: var(--ink-default);
  }
  .opt.on {
    background: var(--key-light);
    color: var(--ink-strong);
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.1);
  }
  .opt .label {
    color: inherit;
  }
</style>
