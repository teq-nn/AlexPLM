<script lang="ts">
  // Issue #47 — Artefakt-Karte built by convention from tracked files (Pattern-Zuordnung).
  // The pure Rust core groups files into this card, picks the Hauptdatei (highest glob
  // priority) and derives the primary action (open the dominant file, else the folder). This
  // component only renders that decision and fires the one-click open via the OS default
  // program — no app-internal program mapping (PRD §14). No git vocabulary surfaces.
  import type { ArtefaktKarte, ArtifactSignal } from "./types";
  import Led from "./Led.svelte";

  let {
    karte,
    index = 0,
    // Auto-Lock LED signal for the Hauptdatei, if read back (Issue #6); quiet by default.
    signal = null,
    // One-click primary action: open the dominant file or the folder via OS default.
    onOpen = (_karte: ArtefaktKarte) => {},
    // Hand-assignment is ONLY a correction (Issue #47): a quiet, hidden-until-asked gesture.
    onCorrect = undefined,
  }: {
    karte: ArtefaktKarte;
    index?: number;
    signal?: ArtifactSignal | null;
    onOpen?: (karte: ArtefaktKarte) => void;
    onCorrect?: ((karte: ArtefaktKarte) => void) | undefined;
  } = $props();

  // The Hauptdatei carries weight (filename), the path stays visible but muted — the tool
  // never hides the filesystem. Fall back to the folder when a card has no single main file.
  const file = $derived(karte.hauptdatei ?? null);
  const fileName = $derived(file ? file.split("/").pop()! : null);

  // Map the Auto-Lock status (Status Reader, E37) onto the physical-LED vocabulary, exactly
  // as ArtifactCard does: orange only for the loud exception (foreign lock).
  const status = $derived(signal?.status ?? "in-progress");
  const led = $derived(
    status === "locked-by-other"
      ? "attention"
      : status === "free"
        ? "free"
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

  // The primary action's verb + target, stated honestly in the tool's own words.
  const opensFile = $derived(karte.primaer === "datei");
  const actionLabel = $derived(opensFile ? "öffnen" : "Ordner öffnen");
  const extra = $derived(Math.max(0, karte.dateien.length - 1));
</script>

<article class="card" class:locked={lockedByOther} style:--i={index}>
  <div class="head">
    <Led status={led} title={ledTitle} />
    <h2 class="label name">{karte.baustein}</h2>
    {#if extra > 0}
      <span class="mono count" title={`${karte.dateien.length} Dateien in diesem Artefakt`}>
        +{extra}
      </span>
    {/if}
  </div>

  <div class="body">
    {#if fileName}
      <div class="mono filename" title={file ?? ""}>{fileName}</div>
      <div class="mono path">{file}</div>
    {:else}
      <div class="mono path empty">{karte.ordner || "."}</div>
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

  <div class="foot">
    <!-- THE one-click primary action: open the dominant file or the folder via OS default. -->
    <button
      class="key open"
      class:folder={!opensFile}
      onclick={() => onOpen(karte)}
      disabled={!karte.ziel}
      title={opensFile ? `${fileName} öffnen` : `${karte.ordner || "."} öffnen`}
    >
      <span class="glyph" aria-hidden="true">{opensFile ? "▸" : "▭"}</span>
      <span class="label">{actionLabel}</span>
    </button>

    {#if onCorrect}
      <!-- Hand-assignment is ONLY a correction (Issue #47): quiet, never the loud default. -->
      <button class="correct label" onclick={() => onCorrect?.(karte)} title="Zuordnung korrigieren">
        zuordnen …
      </button>
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
    animation: rise 360ms var(--ease) backwards;
    animation-delay: calc(var(--i) * 35ms);
  }
  .card:hover {
    border-color: var(--key-mid);
    transform: translateY(-1px);
    box-shadow: 0 2px 0 rgba(28, 26, 25, 0.04);
  }
  /* The loud exception: a foreign-locked card gets a thin orange left edge (orange is rationed). */
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
  /* "+N" companion-file count — a quiet recessed Mono chip; routine, never orange. */
  .count {
    flex: none;
    font-size: 10px;
    color: var(--ink-muted);
    background: var(--surface-sunken);
    border-radius: 99px;
    padding: 1px 7px;
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

  .foot {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* Neutral card action key — creme cap, hairline, seated edge (Stilbeschreibung §Tasten).
     Opening is routine, grey work; never the rationed orange. */
  .open {
    appearance: none;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    align-self: flex-start;
    background: var(--key-light);
    color: var(--ink-strong);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 6px 12px;
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.1);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .open .label {
    color: inherit;
    font-size: 10px;
  }
  .open .glyph {
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1;
    color: var(--ink-muted);
  }
  /* The folder action reads a touch quieter than the file action — same key, lighter glyph. */
  .open.folder .glyph {
    color: var(--key-mid);
  }
  .open:hover:not(:disabled) {
    background: #f5f3ee;
  }
  .open:active:not(:disabled) {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.1);
  }
  .open:disabled {
    cursor: default;
    opacity: 0.5;
    box-shadow: none;
  }

  /* Correction affordance: a dotted, recessed link — present but never inviting (Issue #47). */
  .correct {
    appearance: none;
    cursor: pointer;
    background: none;
    border: none;
    color: var(--ink-muted);
    font-size: 9.5px;
    padding: 0;
    transition: color var(--dur) var(--ease);
  }
  .correct:hover {
    color: var(--ink-default);
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
