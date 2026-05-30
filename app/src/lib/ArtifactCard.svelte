<script lang="ts">
  import type { Baustein, ArtifactSignal } from "./types";
  import Led from "./Led.svelte";

  let {
    baustein,
    index = 0,
    signal = null,
    onedit = undefined,
  }: {
    baustein: Baustein;
    index?: number;
    signal?: ArtifactSignal | null;
    onedit?: (() => void) | undefined;
  } = $props();

  // Split the main file into directory + filename so the filename can carry weight
  // while the real path stays visible but muted (the tool never hides the filesystem).
  const file = $derived(baustein.main_file ?? null);
  const fileName = $derived(file ? file.split("/").pop()! : null);

  // Map the derived status (Status Reader, E37) onto the physical-LED vocabulary.
  //   free            → green  (frei / sauber)
  //   in-progress     → grey   (in Arbeit / ruhend) — the quiet default
  //   locked-by-other → orange (gesperrt — the one loud exception)
  const status = $derived(signal?.status ?? "in-progress");
  const led = $derived(
    status === "free"
      ? "free"
      : status === "locked-by-other"
        ? "attention"
        : "working",
  );
  const ledTitle = $derived(
    status === "locked-by-other"
      ? (signal?.tooltip ?? "gesperrt")
      : status === "free"
        ? "frei"
        : "in Arbeit / ruhend",
  );
  const lockedByOther = $derived(status === "locked-by-other");
  // Editing is offered for any artifact with a file; lockable ones auto-acquire a lock when
  // opened (E31). A foreign-locked file can't be taken over here — that's loud coordination.
  const canEdit = $derived(!!onedit && !!baustein.main_file && !lockedByOther);
</script>

<article class="card" class:locked={lockedByOther} style:--i={index}>
  <div class="head">
    <Led status={led} title={ledTitle} />
    <h2 class="label name">{baustein.name}</h2>
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
