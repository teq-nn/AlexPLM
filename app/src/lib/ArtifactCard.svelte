<script lang="ts">
  import type { ProduktBaustein, ArtifactSignal } from "./types";
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
    // Auto-Lock & Status-Signale (Issue #6): the derived per-artifact LED signal, and the
    // edit gesture that auto-acquires a lock (E31).
    signal = null,
    onedit = undefined,
  }: {
    baustein: ProduktBaustein;
    index?: number;
    candidates?: ProduktBaustein[];
    source?: string | null;
    stale?: boolean;
    onDeriveFrom?: (source: string) => void;
    onClearEdge?: () => void;
    signal?: ArtifactSignal | null;
    onedit?: (() => void) | undefined;
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

  // Map the derived status (Status Reader, E37) onto the physical-LED vocabulary.
  //   free            → green  (frei / sauber)
  //   in-progress     → grey   (in Arbeit / ruhend) — the quiet default
  //   locked-by-other → orange (gesperrt — the one loud exception)
  // A stale derivation (E26) is the other loud exception, so it also raises the LED.
  const status = $derived(signal?.status ?? "in-progress");
  const led = $derived(
    status === "locked-by-other" || stale
      ? "attention"
      : status === "free"
        ? "free"
        : "working",
  );
  const ledTitle = $derived(
    status === "locked-by-other"
      ? (signal?.tooltip ?? "gesperrt")
      : stale
        ? "Prüfung erforderlich — Quelle ist neuer"
        : status === "free"
          ? "frei"
          : "in Arbeit / ruhend",
  );
  const lockedByOther = $derived(status === "locked-by-other");
  // Editing is offered for any artifact with a file; lockable ones auto-acquire a lock when
  // opened (E31). A foreign-locked file can't be taken over here — that's loud coordination.
  const canEdit = $derived(!!onedit && !!baustein.main_file && !lockedByOther);
</script>

<article class="card" class:stale class:locked={lockedByOther} style:--i={index}>
  <div class="head">
    <Led status={led} title={ledTitle} />
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

    {#if lockedByOther && signal}
      <!-- The loud exception, stated honestly: who holds it and since when. -->
      <div class="lockline mono" title={signal.tooltip}>
        gesperrt von <span class="who">{signal.locked_by}</span>
        {#if signal.locked_at}
          <span class="since">seit {signal.locked_at}</span>
        {/if}
      </div>
    {/if}
  </div>

  {#if canEdit}
    <!-- Primary card action: opening/editing a lockable artifact auto-acquires its lock (E31). -->
    <button class="key edit" onclick={() => onedit?.()}>
      <span class="label">bearbeiten</span>
    </button>
  {/if}

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
  /* The loud exception: a foreign-locked card gets a thin orange left edge — enough to
     find at a glance on a grey grid, never a full orange fill (orange is rationed). */
  .card.locked {
    border-left: 2px solid var(--accent);
    padding-left: 14px;
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

  /* "gesperrt von X seit …" — the honest coordination line on a foreign-locked card. */
  .lockline {
    margin-top: 5px;
    font-size: 11px;
    color: var(--accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lockline .who {
    font-weight: 600;
  }
  .lockline .since {
    color: var(--ink-muted);
  }

  /* Neutral card action key — creme cap, hairline, seated edge (Stilbeschreibung §Tasten).
     Quiet by default; locking is a routine, grey act, not the loud orange exception. */
  .edit {
    appearance: none;
    cursor: pointer;
    align-self: flex-start;
    background: var(--key-light);
    color: var(--ink-strong);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 6px 11px;
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.1);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .edit .label {
    color: inherit;
    font-size: 10px;
  }
  .edit:hover {
    background: #f5f3ee;
  }
  .edit:active {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.1);
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
