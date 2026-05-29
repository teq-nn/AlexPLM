<script lang="ts">
  import type { Baustein } from "./types";
  import Led from "./Led.svelte";

  let {
    baustein,
    index = 0,
    // Other artifacts this card can be derived from (its own path excluded by the parent).
    candidates = [],
    // The source this artifact is currently „abgeleitet von", if an edge was drawn.
    source = null,
    // True when a manual edge exists AND the source is newer than this derivation (E26).
    stale = false,
    // Gestures: draw / clear a manual „abgeleitet von" edge for this card.
    onDeriveFrom = (_source: string) => {},
    onClearEdge = () => {},
  }: {
    baustein: Baustein;
    index?: number;
    candidates?: Baustein[];
    source?: string | null;
    stale?: boolean;
    onDeriveFrom?: (source: string) => void;
    onClearEdge?: () => void;
  } = $props();

  // Split the main file into directory + filename so the filename can carry weight
  // while the real path stays visible but muted (the tool never hides the filesystem).
  const file = $derived(baustein.main_file ?? null);
  const fileName = $derived(file ? file.split("/").pop()! : null);

  // The picker is a cheap, hidden-until-asked gesture (E40): routine stays quiet.
  let picking = $state(false);
  let pick = $state("");

  function commitPick() {
    const chosen = pick.trim();
    if (chosen) onDeriveFrom(chosen);
    picking = false;
    pick = "";
  }
</script>

<article class="card" class:stale style:--i={index}>
  <div class="head">
    <Led
      status={stale ? "attention" : "working"}
      title={stale ? "Prüfung erforderlich — Quelle ist neuer" : "in Arbeit / ruhend"}
    />
    <h2 class="label name">{baustein.name}</h2>
    {#if stale}
      <span class="label flag">Prüfen</span>
    {/if}
  </div>

  <div class="body">
    {#if fileName}
      <div class="mono filename" title={file ?? ""}>{fileName}</div>
      <div class="mono path">{file}</div>
    {:else}
      <div class="mono path empty">{baustein.path}</div>
    {/if}
  </div>

  <!-- „Abgeleitet von" — opt-in lineage. Absent edge = quiet card, no claim (E40). -->
  <div class="edge">
    {#if source}
      <div class="lineage" class:stale title={`abgeleitet von ${source}`}>
        <span class="label from">abgeleitet von</span>
        <span class="mono src">{source}</span>
        <button class="clear" onclick={onClearEdge} title="Kante entfernen" aria-label="Kante entfernen">×</button>
      </div>
    {:else if picking}
      <div class="picker">
        <select class="mono pick" bind:value={pick} onchange={commitPick} aria-label="Quelle wählen">
          <option value="" disabled selected>Quelle wählen …</option>
          {#each candidates as c (c.path)}
            <option value={c.path}>{c.name} — {c.path}</option>
          {/each}
        </select>
        <button class="link" onclick={() => (picking = false)}>abbrechen</button>
      </div>
    {:else if candidates.length > 0}
      <button class="derive label" onclick={() => (picking = true)}>+ abgeleitet von …</button>
    {/if}
  </div>
</article>

<style>
  .card {
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 14px 15px 15px;
    display: flex;
    flex-direction: column;
    gap: 11px;
    transition:
      border-color var(--dur) var(--ease),
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
    /* staggered reveal on open */
    animation: rise 360ms var(--ease) backwards;
    animation-delay: calc(var(--i) * 35ms);
  }
  .card:hover {
    border-color: var(--key-mid);
    transform: translateY(-1px);
    box-shadow: 0 2px 0 rgba(28, 26, 25, 0.04);
  }
  /* The loud exception (E41): a stale derivation lifts its voice — a single orange edge. */
  .card.stale {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent), 0 2px 0 rgba(28, 26, 25, 0.04);
  }
  .card.stale:hover {
    border-color: var(--accent);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding-bottom: 11px;
    border-bottom: 1px solid var(--hairline);
  }
  .name {
    margin: 0;
    color: var(--ink-strong);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }
  .flag {
    color: var(--accent);
    font-size: 10px;
    flex: none;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .filename {
    color: var(--ink-default);
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    color: var(--ink-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path.empty {
    font-size: 12px;
  }

  /* Lineage row — quiet by default; the source path is data (Mono), the label is caps. */
  .edge {
    min-height: 20px;
    display: flex;
    align-items: center;
  }
  .lineage {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    width: 100%;
  }
  .from {
    color: var(--ink-muted);
    font-size: 9.5px;
    flex: none;
  }
  .lineage.stale .from {
    color: var(--accent);
  }
  .src {
    color: var(--ink-default);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }
  .clear {
    appearance: none;
    border: none;
    background: none;
    cursor: pointer;
    color: var(--ink-muted);
    font-family: var(--font-mono);
    font-size: 14px;
    line-height: 1;
    padding: 0 2px;
    flex: none;
    transition: color var(--dur) var(--ease);
  }
  .clear:hover {
    color: var(--ink-strong);
  }

  /* The cheap gesture: a quiet, dotted "key" that only appears when there's a candidate. */
  .derive {
    appearance: none;
    cursor: pointer;
    background: none;
    border: 1px dashed var(--hairline);
    border-radius: var(--radius-sm);
    color: var(--ink-muted);
    font-size: 9.5px;
    padding: 4px 8px;
    transition:
      border-color var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .derive:hover {
    border-color: var(--key-mid);
    color: var(--ink-default);
  }

  .picker {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-width: 0;
  }
  .pick {
    flex: 1;
    min-width: 0;
    background: var(--surface-sunken);
    color: var(--ink-default);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 4px 6px;
    font-size: 11px;
  }
  .pick:focus {
    outline: none;
    border-color: var(--key-mid);
  }
  .link {
    appearance: none;
    border: none;
    background: none;
    cursor: pointer;
    color: var(--ink-muted);
    font-family: var(--font-label);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 9.5px;
    font-weight: 600;
    padding: 0;
    flex: none;
    transition: color var(--dur) var(--ease);
  }
  .link:hover {
    color: var(--ink-strong);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
