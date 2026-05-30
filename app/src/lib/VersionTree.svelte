<script lang="ts">
  import type { StandNode, VersionGraph } from "./types";

  let {
    graph,
    onPromote,
  }: {
    graph: VersionGraph | null;
    /** Promote a Stand to a Meilenstein with human VERSION_NOTES text. */
    onPromote: (node: StandNode, version: string, notes: string) => Promise<void>;
  } = $props();

  // The promote dialog is the rare moment the user writes text (E28) — it is a quiet,
  // deliberate panel, not an alarm. No orange here; orange is for the laute Ausnahme only.
  let promoting = $state<StandNode | null>(null);
  let draftVersion = $state("");
  let draftNotes = $state("");
  let busy = $state(false);
  let promoteError = $state<string | null>(null);

  function openPromote(node: StandNode) {
    promoting = node;
    draftVersion = node.milestone ?? "";
    draftNotes = "";
    promoteError = null;
  }
  function cancelPromote() {
    promoting = null;
    busy = false;
  }
  async function confirmPromote() {
    if (!promoting) return;
    const v = draftVersion.trim();
    const n = draftNotes.trim();
    if (!v) {
      promoteError = "Version fehlt";
      return;
    }
    if (!n) {
      promoteError = "Meilenstein braucht einen Text";
      return;
    }
    busy = true;
    promoteError = null;
    try {
      await onPromote(promoting, v, n);
      promoting = null;
    } catch (e) {
      promoteError = String(e);
    } finally {
      busy = false;
    }
  }

  function clock(ts: string): string {
    return ts.slice(11, 19) || ts;
  }
  function day(ts: string): string {
    return ts.slice(0, 10);
  }
  // The node label is the recorded path's tail (filename / Baustein), kept short.
  function leaf(path: string): string {
    if (path === "." || path === "") return "Produkt";
    const parts = path.split("/");
    return parts[parts.length - 1];
  }

  // A Zweig (off-trunk lane) is labelled once, on the newest Stand of that lane — its tip.
  // The nodes arrive newest-first, so the first node we meet on a lane is its tip.
  const tipOfLane = $derived.by(() => {
    const tips = new Set<string>();
    const seen = new Set<number>();
    for (const n of graph?.nodes ?? []) {
      if (n.lane > 0 && !seen.has(n.lane)) {
        seen.add(n.lane);
        tips.add(n.id);
      }
    }
    return tips;
  });

  // More than one line present? Then we draw the lane gutter; a single linear history
  // keeps lane 0 throughout and looks exactly as before.
  const branched = $derived((graph?.lane_count ?? 1) > 1);
</script>

<section class="display" aria-label="Versionsbaum">
  <div class="display-head">
    <span class="label title">Versionsbaum</span>
    {#if graph}
      {#if branched && graph.active_branch}
        <span class="active-line label" title="Aktiver Zweig">
          <span class="active-dot" aria-hidden="true"></span>
          {graph.active_branch}
        </span>
      {/if}
      <span class="node-count mono">{graph.nodes.length.toString().padStart(2, "0")}</span>
    {/if}
  </div>

  <div class="tree-scroll">
    {#if !graph || graph.nodes.length === 0}
      <p class="idle mono">— noch keine Stände —</p>
    {:else}
      <ol class="tree">
        {#each graph.nodes as n, i (n.id)}
          {@const isMs = n.milestone !== null}
          {@const foreign = !n.on_active}
          {@const isTip = tipOfLane.has(n.id)}
          <li
            class="node"
            class:milestone={isMs}
            class:offloaded={n.offloaded}
            class:foreign
            class:first={i === 0}
            class:last={i === graph.nodes.length - 1}
            style="--lane: {n.lane};"
          >
            <!-- the spine: edge above + node dot + edge below. Off-trunk Zweige sit one
                 gutter to the right and read in the foreign blue. -->
            <span class="spine" aria-hidden="true">
              <span class="edge top"></span>
              <span class="dot" class:ms={isMs}></span>
              <span class="edge bottom"></span>
            </span>

            <div class="body">
              {#if foreign && isTip && n.branch}
                <span class="zweig-tag label" title="Zweig {n.branch}">{n.branch}</span>
              {/if}
              <div class="row">
                <span class="path mono" title={n.path}>{leaf(n.path)}</span>
                {#if isMs}
                  <span class="version mono" title="Meilenstein {n.milestone}"
                    >{n.milestone}</span
                  >
                {/if}
              </div>

              <div class="row sub">
                <span class="time mono">
                  <span class="t">{clock(n.timestamp)}</span>
                  <span class="d">{day(n.timestamp)}</span>
                </span>

                {#if n.offloaded}
                  <span class="tag offloaded-tag label">
                    Inhalt ausgelagert{#if graph.offloaded_archive}
                      · {graph.offloaded_archive}{/if}
                  </span>
                {/if}
              </div>

              {#if !isMs}
                <button
                  class="promote label"
                  onclick={() => openPromote(n)}
                  title="Diesen Stand zum Meilenstein machen"
                >
                  Zum Meilenstein
                </button>
              {/if}
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </div>
</section>

{#if promoting}
  <!-- A deliberate, quiet panel for the one place human text is written (E28). -->
  <div class="overlay">
    <!-- Backdrop is a real button so dismiss is keyboard-reachable (Esc / Enter). -->
    <button
      class="backdrop"
      type="button"
      aria-label="Abbrechen"
      onclick={cancelPromote}
    ></button>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Meilenstein anlegen"
      tabindex="-1"
    >
      <header class="dialog-head">
        <span class="label">Meilenstein</span>
        <span class="dialog-stand mono">{leaf(promoting.path)} · {clock(promoting.timestamp)}</span>
      </header>

      <label class="field">
        <span class="label field-label">Version</span>
        <input
          class="input mono"
          bind:value={draftVersion}
          placeholder="v1.0"
          spellcheck="false"
          autocomplete="off"
        />
      </label>

      <label class="field">
        <span class="label field-label">VERSION_NOTES.md</span>
        <textarea
          class="input mono notes"
          bind:value={draftNotes}
          rows="5"
          placeholder="Was macht diesen Stand vorzeigbar?"
        ></textarea>
        <span class="field-hint label">Der einzige Ort für deinen Text</span>
      </label>

      {#if promoteError}
        <p class="dialog-error mono">{promoteError}</p>
      {/if}

      <footer class="dialog-actions">
        <button class="key ghost label" onclick={cancelPromote} disabled={busy}>
          Abbrechen
        </button>
        <button class="key solid label" onclick={confirmPromote} disabled={busy}>
          {busy ? "…" : "Festschreiben"}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  /* The dark "display" zone — a recessed instrument screen, distinct from the warm-grey
     chassis. Mono data, caps labels, LED-like nodes. Orange is reserved for the loud
     exception and never appears here. */
  .display {
    display: flex;
    flex-direction: column;
    width: 300px;
    flex: none;
    min-height: 0;
    background: linear-gradient(180deg, #131110, #0b0a09);
    border-left: 1px solid #000;
    box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.03);
    position: relative;
    overflow: hidden;
  }
  /* faint scanline texture, same instrument feel as the version bar */
  .display::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: repeating-linear-gradient(
      0deg,
      rgba(255, 255, 255, 0.016) 0px,
      rgba(255, 255, 255, 0.016) 1px,
      transparent 1px,
      transparent 3px
    );
    mix-blend-mode: screen;
  }

  .display-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 11px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }
  .title {
    color: #8c8881;
  }
  .node-count {
    color: var(--screen-fg);
    font-size: 13px;
    font-weight: 600;
  }

  .tree-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 0 18px;
    position: relative;
    z-index: 1;
  }

  .idle {
    color: #5a564f;
    font-size: 12px;
    text-align: center;
    padding: 24px 14px;
  }

  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  /* Each node: a left spine column (edges + dot) and the data body. A Stand on a diverging
     Zweig (lane > 0) is pushed one gutter to the right per lane, so the line reads as its
     own track. A single linear history keeps --lane: 0 and sits flush like before. */
  .node {
    --gutter: 18px;
    display: grid;
    grid-template-columns: 26px 1fr;
    gap: 10px;
    padding: 2px 14px 2px 12px;
    padding-left: calc(12px + var(--lane, 0) * var(--gutter));
    align-items: stretch;
  }

  .spine {
    position: relative;
    display: grid;
    grid-template-rows: 1fr auto 1fr;
    justify-items: center;
    align-items: center;
  }
  .edge {
    width: 1px;
    background: #2c2a27;
    justify-self: center;
    align-self: stretch;
  }
  .node.first .edge.top,
  .node.last .edge.bottom {
    background: transparent;
  }

  /* Node dot = LED. A plain Stand is a small grey ring; a Meilenstein is a bright,
     larger filled dot — the promoted node literally stands out on the spine. */
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #4a4641;
    box-shadow:
      0 0 0 1px #000,
      inset 0 1px 0.5px rgba(255, 255, 255, 0.25);
    z-index: 1;
  }
  .dot.ms {
    width: 12px;
    height: 12px;
    background: var(--screen-fg);
    box-shadow:
      0 0 0 1px #000,
      0 0 6px 1px rgba(232, 230, 225, 0.35),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.7);
  }

  .body {
    padding: 7px 0;
    min-width: 0;
  }
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }
  .path {
    color: #cfccc5;
    font-size: 12.5px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .milestone .path {
    color: var(--screen-fg);
  }

  /* The version label: bright Mono, a small recessed chip — the 7-segment feel. */
  .version {
    flex: none;
    color: var(--screen-fg);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.01em;
    padding: 1px 7px;
    border-radius: var(--radius-sm);
    background: rgba(232, 230, 225, 0.08);
    box-shadow: inset 0 0 0 1px rgba(232, 230, 225, 0.18);
  }

  .sub {
    margin-top: 3px;
  }
  .time {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }
  .time .t {
    color: #8c8881;
    font-size: 11px;
    letter-spacing: 0.02em;
  }
  .time .d {
    color: #5a564f;
    font-size: 10px;
  }

  .tag {
    font-size: 9.5px;
    letter-spacing: 0.04em;
  }
  .offloaded-tag {
    color: #6b6864;
  }
  /* Offloaded: honestly dimmed, content gone but the node remains (E36). */
  .node.offloaded .path,
  .node.offloaded .version {
    color: #7a766f;
  }
  .node.offloaded .dot {
    background: #322f2c;
    box-shadow: 0 0 0 1px #000;
  }

  /* A diverging Zweig reads in the second colour (foreign blue, dark surfaces only): its
     spine edges, its LED, and its path tint shift to --data-foreign so the line is plainly
     distinct from the active grey trunk. The active line stays grey — clearly the "own" one. */
  .node.foreign .edge {
    background: color-mix(in srgb, var(--data-foreign) 42%, #0b0a09);
  }
  .node.foreign .dot {
    background: var(--data-foreign);
    box-shadow:
      0 0 0 1px #000,
      0 0 5px 0.5px color-mix(in srgb, var(--data-foreign) 55%, transparent),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.35);
  }
  .node.foreign .dot.ms {
    box-shadow:
      0 0 0 1px #000,
      0 0 7px 1px color-mix(in srgb, var(--data-foreign) 60%, transparent),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.6);
  }
  .node.foreign .path {
    color: color-mix(in srgb, var(--data-foreign) 70%, #cfccc5);
  }
  .node.foreign.milestone .path,
  .node.foreign .version {
    color: color-mix(in srgb, var(--data-foreign) 78%, #fff);
  }
  .node.foreign .version {
    background: color-mix(in srgb, var(--data-foreign) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--data-foreign) 34%, transparent);
  }
  .node.foreign.offloaded .path,
  .node.foreign.offloaded .version,
  .node.foreign.offloaded .dot {
    color: #6d7585;
    background: #1c1f26;
  }

  /* The Zweig name, shown once at the line's tip: a small foreign-blue caps tag that sits
     just above the tip Stand, naming the line in domain vocabulary (never "branch"). */
  .zweig-tag {
    display: inline-block;
    margin-bottom: 3px;
    color: var(--data-foreign);
    font-size: 9.5px;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--data-foreign) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--data-foreign) 30%, transparent);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The active line marker in the head: a grey LED + the active Zweig name, so the user
     always sees which line is "theirs" once more than one line is in view. */
  .active-line {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-left: auto;
    margin-right: 10px;
    color: #8c8881;
    font-size: 9.5px;
    letter-spacing: 0.05em;
  }
  .active-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--screen-fg);
    box-shadow:
      0 0 0 1px #000,
      0 0 4px 0.5px rgba(232, 230, 225, 0.4);
  }

  /* Promote affordance: a quiet hairline-outline control on the dark screen. Appears on
     hover/focus so the spine stays calm at rest. */
  .promote {
    margin-top: 7px;
    appearance: none;
    cursor: pointer;
    color: #9a968f;
    background: transparent;
    border: 1px solid rgba(232, 230, 225, 0.14);
    border-radius: var(--radius-sm);
    padding: 4px 9px;
    opacity: 0;
    transform: translateY(-1px);
    transition:
      opacity var(--dur) var(--ease),
      color var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .node:hover .promote,
  .promote:focus-visible {
    opacity: 1;
    transform: none;
  }
  .promote:hover {
    color: var(--screen-fg);
    border-color: rgba(232, 230, 225, 0.4);
  }
  @media (prefers-reduced-motion: reduce) {
    .promote {
      opacity: 1;
      transform: none;
    }
  }

  /* Promote dialog — a calm seated panel on the warm chassis, not an alarm. */
  .overlay {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    z-index: 50;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    margin: 0;
    padding: 0;
    background: rgba(14, 13, 12, 0.5);
    cursor: default;
  }
  .dialog {
    position: relative;
    z-index: 1;
    width: min(420px, calc(100vw - 48px));
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.6) inset,
      0 12px 40px rgba(28, 26, 25, 0.28);
    padding: 18px 18px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .dialog-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--hairline);
  }
  .dialog-head .label {
    color: var(--ink-strong);
  }
  .dialog-stand {
    color: var(--ink-muted);
    font-size: 11px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .field-label {
    color: var(--ink-muted);
  }
  .input {
    appearance: none;
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    box-shadow: inset 0 1px 2px rgba(28, 26, 25, 0.06);
    color: var(--ink-strong);
    padding: 8px 10px;
    font-size: 13px;
    width: 100%;
  }
  .input:focus {
    outline: none;
    border-color: var(--ink-muted);
  }
  .notes {
    resize: vertical;
    line-height: 1.5;
  }
  .field-hint {
    color: var(--ink-muted);
    font-size: 9.5px;
    text-transform: none;
    letter-spacing: 0;
  }

  .dialog-error {
    color: var(--accent);
    font-size: 12px;
    margin: 0;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 2px;
  }
  .key {
    appearance: none;
    cursor: pointer;
    border-radius: var(--radius);
    padding: 8px 16px;
    border: 1px solid var(--hairline);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .key:active {
    transform: translateY(1px);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.55;
  }
  /* Neutral cancel = hairline outline only. */
  .key.ghost {
    background: transparent;
    color: var(--ink-default);
  }
  .key.ghost:hover {
    background: var(--surface-sunken);
  }
  /* Confirm = the deliberate dark "history-touching" key (E27/E38 weight): committing a
     Meilenstein is a considered act, so it reads as a seated dark key, not loud orange. */
  .key.solid {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.25);
  }
  .key.solid:hover {
    background: #2a2724;
  }
</style>
