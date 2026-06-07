<script lang="ts">
  import type { GraphFilter, StandNode, VersionGraph } from "./types";

  let {
    graph,
    onPromote,
    onToggleArt,
    filter = null,
    onNodeAction = null,
    title = "Verlauf · Graph",
  }: {
    graph: VersionGraph | null;
    /** Promote a Stand to a Revision with human VERSION_NOTES text. */
    onPromote: (node: StandNode, version: string, notes: string) => Promise<void>;
    /** Toggle a Revision's Art: Prototyp ↔ Freigabe ("Releasen" / "Un-Release"). E42. */
    onToggleArt: (node: StandNode) => Promise<void>;
    /** Graph-Raum display filter (Issue #55, E45): hides nodes only, never rewrites. When null
     *  (the embedded column) everything shows. */
    filter?: GraphFilter | null;
    /** Graph-Raum node-verb hook (Issue #55, E27): when provided, a node click opens the verb
     *  menu instead of the promote dialog — the room never silently moves the Werkbank. The
     *  caller is handed the clicked node and the LED's screen rect to anchor the menu. */
    onNodeAction?: ((node: StandNode, anchor: DOMRect) => void) | null;
    /** Heading shown in the display head. */
    title?: string;
  } = $props();

  // Graph-Raum mode: a node click offers the three verbs instead of promoting. The embedded
  // column keeps its promote-on-click affordance.
  const verbMode = $derived(onNodeAction !== null);

  // The promote dialog is the rare moment the user writes text (E28) — it is a quiet,
  // deliberate panel, not an alarm. No orange here; orange is for the laute Ausnahme only.
  let promoting = $state<StandNode | null>(null);
  let draftVersion = $state("");
  let draftNotes = $state("");
  let busy = $state(false);
  let promoteError = $state<string | null>(null);

  function openPromote(node: StandNode) {
    promoting = node;
    draftVersion = node.revision ?? "";
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
      promoteError = "Revision braucht einen Text";
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

  // Toggle the Art of the Revision being viewed in the dialog (E42): Prototyp → Freigabe
  // is "Releasen" (write-protects the tag), Freigabe → Prototyp the deliberate "Un-Release".
  // A considered act, so it shares the dialog's calm seated-key weight — never orange.
  async function toggleArt() {
    if (!promoting || promoting.revision === null) return;
    busy = true;
    promoteError = null;
    try {
      await onToggleArt(promoting);
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

  // ── Graph geometry ────────────────────────────────────────────────────────
  // The tree is laid out like a real git-graph (GitKraken/VS Code *structure*): each Stand
  // is a node on its Bahn (lane), rows run newest-first top-to-bottom, and a connector is
  // drawn from every Stand down to each predecessor it „folgt auf". Same lane → a straight
  // segment; a lane change (a fork, or a Zusammenführung of two Linien) → a smooth Bézier.
  const ROW = 60; // vertical rhythm between Stände
  const LANE = 26; // horizontal gap between Bahnen
  const PAD_LEFT = 20; // x of the trunk (lane 0) centre
  const RIGHT_GUT = 18; // breathing room between the lanes and the text body

  // Pure display filter (Issue #55, E45): hide variant lines and/or keep only Revisionen.
  // The active line always survives the variant filter so "where I am" stays navigable; the
  // filter mirrors the Rust `passes_filter` core exactly. Null filter = show everything.
  function passesFilter(n: StandNode): boolean {
    if (!filter) return true;
    if (!filter.varianten && !n.on_active) return false;
    if (filter.nur_revisionen && n.revision === null) return false;
    return true;
  }
  const nodes = $derived((graph?.nodes ?? []).filter(passesFilter));
  const laneCount = $derived(graph?.lane_count ?? 1);
  const rowOf = $derived(new Map(nodes.map((n, i) => [n.id, i])));
  const byId = $derived(new Map(nodes.map((n) => [n.id, n])));

  const laneAreaWidth = $derived(PAD_LEFT + (laneCount - 1) * LANE + RIGHT_GUT);
  const svgHeight = $derived(Math.max(nodes.length * ROW, ROW));

  const rowY = (i: number) => i * ROW + ROW / 2;
  const laneX = (lane: number) => PAD_LEFT + lane * LANE;

  // Colour by Bahn, within the rationed palette: the trunk (lane 0, the active line) reads
  // in warm grey/white; every diverging Zweig reads in the single foreign blue, brightened
  // a step per lane so several Zweige stay distinguishable WITHOUT inventing new hues.
  // Orange is never produced here — it is reserved for the laute Ausnahme.
  function foreignTint(lane: number): string {
    const l = Math.min((lane - 1) * 16, 48);
    return `color-mix(in srgb, var(--data-foreign) ${100 - l}%, #bcd6ff ${l}%)`;
  }
  function wireColor(lane: number): string {
    if (lane <= 0) return "rgba(232, 230, 225, 0.34)";
    return foreignTint(lane);
  }

  // One connector per (Stand → predecessor present in the tree). A predecessor always sits
  // on a lower row (it is older → larger y), so the path runs downward. Both control points
  // ride the vertical midpoint, giving the symmetric S that reads as a fork / Zusammenführung.
  const edges = $derived.by(() => {
    const out: { d: string; lane: number; key: string }[] = [];
    for (let i = 0; i < nodes.length; i++) {
      const child = nodes[i];
      for (const pid of child.parents) {
        const pi = rowOf.get(pid);
        if (pi === undefined) continue;
        const parent = byId.get(pid)!;
        const x1 = laneX(child.lane);
        const y1 = rowY(i);
        const x2 = laneX(parent.lane);
        const y2 = rowY(pi);
        const d =
          x1 === x2
            ? `M${x1} ${y1} L${x2} ${y2}`
            : `M${x1} ${y1} C${x1} ${(y1 + y2) / 2} ${x2} ${(y1 + y2) / 2} ${x2} ${y2}`;
        // Colour by the deeper Bahn so a fork/merge curve takes the diverging line's tone.
        out.push({ d, lane: Math.max(child.lane, parent.lane), key: `${child.id}>${pid}` });
      }
    }
    return out;
  });

  // The active Stand to highlight: the newest Stand on the active line (its tip). With a
  // single linear history that is simply the newest Stand.
  const activeId = $derived(nodes.find((n) => n.on_active)?.id ?? null);

  // A Zweig (off-trunk lane) is labelled once, on its newest Stand (its tip). Nodes arrive
  // newest-first, so the first node we meet on a lane is its tip.
  const tipOfLane = $derived.by(() => {
    const tips = new Set<string>();
    const seen = new Set<number>();
    for (const n of nodes) {
      if (n.lane > 0 && !seen.has(n.lane)) {
        seen.add(n.lane);
        tips.add(n.id);
      }
    }
    return tips;
  });

  // More than one line present? Then the head shows the active-line marker; a single linear
  // history keeps lane 0 throughout and reads as one quiet track.
  const branched = $derived(laneCount > 1);

  // ── Hover/focus tooltip ─────────────────────────────────────────────────────
  // A fixed card that follows the cursor. The Versionsbaum is the right-most column, so the
  // card opens to the LEFT of the pointer to stay on screen. Domain words only — no git, no
  // author (the model carries none).
  let tip = $state<{ node: StandNode; x: number; y: number } | null>(null);
  function showTip(node: StandNode, ev: MouseEvent) {
    tip = { node, x: ev.clientX, y: ev.clientY };
  }
  function moveTip(ev: MouseEvent) {
    if (tip) tip = { ...tip, x: ev.clientX, y: ev.clientY };
  }
  function focusTip(node: StandNode, ev: FocusEvent) {
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    tip = { node, x: r.left, y: r.top + r.height / 2 };
  }
  function hideTip() {
    tip = null;
  }
  // A node click: in Graph-Raum mode it opens the verb menu (the room never silently moves the
  // Werkbank — E27/§55); in the embedded column it opens the promote dialog as before.
  function activateNode(node: StandNode, ev: MouseEvent | KeyboardEvent) {
    if (verbMode && onNodeAction) {
      const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
      onNodeAction(node, r);
    } else {
      openPromote(node);
    }
  }
  function onNodeKey(node: StandNode, ev: KeyboardEvent) {
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      activateNode(node, ev);
    }
  }
</script>

<section class="display" aria-label={title}>
  <div class="display-head">
    <span class="label title">{title}</span>
    {#if graph}
      {#if branched && graph.active_branch}
        <span class="active-line label" title="Aktiver Branch">
          <span class="active-dot" aria-hidden="true"></span>
          {graph.active_branch}
        </span>
      {/if}
      <span class="node-count mono">{nodes.length.toString().padStart(2, "0")}</span>
    {/if}
  </div>

  <div class="tree-scroll">
    {#if !graph || nodes.length === 0}
      <p class="idle mono">
        {#if graph && graph.nodes.length > 0}— nichts passt zum Filter —{:else}— noch keine Commits —{/if}
      </p>
    {:else}
      <div
        class="graph"
        style="--row: {ROW}px; --lane-area: {laneAreaWidth}px; min-height: {svgHeight}px;"
      >
        <!-- Connectors between Stände: drawn behind the LEDs, never interactive. -->
        <svg
          class="wires"
          width={laneAreaWidth}
          height={svgHeight}
          viewBox="0 0 {laneAreaWidth} {svgHeight}"
          aria-hidden="true"
        >
          {#each edges as e (e.key)}
            <path class="wire" d={e.d} style="stroke: {wireColor(e.lane)};" />
          {/each}
        </svg>

        <ol class="tree">
          {#each nodes as n, i (n.id)}
            {@const isMs = n.revision !== null}
            {@const foreign = !n.on_active}
            {@const isTip = tipOfLane.has(n.id)}
            {@const isActive = n.id === activeId}
            <li class="row" style="--lane-x: {laneX(n.lane)}px;">
              <!-- The Stand itself: an LED on its Bahn. It is the affordance to promote, and
                   hovering/focusing it raises the detail card. -->
              <button
                type="button"
                class="led"
                class:ms={isMs}
                class:foreign
                class:offloaded={n.offloaded}
                class:active={isActive}
                style={foreign ? `--c: ${foreignTint(n.lane)};` : ""}
                title={verbMode ? "Verben: öffnen · abzweigen · zurückwerfen" : "Zur Revision machen"}
                aria-label={isMs
                  ? `Revision ${n.revision}, ${day(n.timestamp)}${verbMode ? " — Verben öffnen" : ""}`
                  : `Commit ${leaf(n.path)}, ${day(n.timestamp)} — ${verbMode ? "Verben öffnen" : "zur Revision machen"}`}
                onclick={(e) => activateNode(n, e)}
                onkeydown={(e) => onNodeKey(n, e)}
                onmouseenter={(e) => showTip(n, e)}
                onmousemove={moveTip}
                onmouseleave={hideTip}
                onfocus={(e) => focusTip(n, e)}
                onblur={hideTip}
              >
                {#if isActive}<span class="ping" aria-hidden="true"></span>{/if}
              </button>

              <div class="body">
                {#if foreign && isTip && n.branch}
                  <span class="zweig-tag label" title="Branch {n.branch}">{n.branch}</span>
                {/if}
                <div class="line">
                  <span class="path mono" title={n.path}>{leaf(n.path)}</span>
                  {#if isMs}
                    <span class="ms-tags">
                      {#if n.revision_art === "freigabe"}
                        <span
                          class="art-chip label freigabe"
                          title="Freigabe — schreibgeschützt"
                        >
                          <span class="lock" aria-hidden="true"></span>
                        </span>
                      {/if}
                      <span class="version mono" title="Revision {n.revision}"
                        >{n.revision}</span
                      >
                    </span>
                  {/if}
                </div>

                <div class="line sub">
                  <span class="time mono">
                    <span class="t">{clock(n.timestamp)}</span>
                    <span class="d">{day(n.timestamp)}</span>
                  </span>

                  {#if n.veroeffentlicht || n.offloaded}
                    <span class="marks">
                      {#if n.veroeffentlicht}
                        <span class="tag veroeff-tag label" title="auf der geteilten Linie">
                          <span class="uplink" aria-hidden="true"></span>veröffentlicht
                        </span>
                      {/if}
                      {#if n.offloaded}
                        <span class="tag offloaded-tag label">
                          Inhalt ausgelagert{#if graph.offloaded_archive}
                            · {graph.offloaded_archive}{/if}
                        </span>
                      {/if}
                    </span>
                  {/if}
                </div>
              </div>
            </li>
          {/each}
        </ol>
      </div>
    {/if}
  </div>
</section>

{#if tip}
  <!-- Detail card: a pure read-only readout, opening to the left of the pointer. Since E43 the
       honest git nouns are sichtbar und erlaubt — only the gefährliche „Wie"-Mechanik stays
       hidden. So the card NAMES the four git-Substantive a HW-Ingenieur may *see* without ever
       operating them (Issue #135, E55): Commit (the Stand), Branch (the line it sits on), Tag
       (the Revision's underlying git tag — a Revision ist „technisch ein Tag auf einem Commit",
       Glossar), and Push (this Stand reached the shared line — veröffentlicht). The word is
       clearly named; there is NO field here where a git/recovery formula could be typed — that
       remains the verb menu's walled-off business. Domain wording stays the headline; the git
       noun rides beside it as a quiet, honest echo. -->
  <div
    class="tip"
    class:foreign={!tip.node.on_active}
    style="left: {tip.x}px; top: {tip.y}px;"
    role="presentation"
  >
    <div class="tip-head">
      <span class="tip-id mono">{tip.node.id.slice(0, 8)}</span>
      {#if tip.node.revision !== null}
        <span class="tip-version mono">{tip.node.revision}</span>
      {/if}
    </div>
    <div class="tip-row mono">
      <span class="tip-key label">Branch</span>
      <span class="tip-val">{tip.node.branch ?? "aktiver Branch"}</span>
    </div>
    <div class="tip-row mono">
      <span class="tip-key label">Commit</span>
      <span class="tip-val">{day(tip.node.timestamp)} · {clock(tip.node.timestamp)}</span>
    </div>
    {#if tip.node.revision !== null}
      <div class="tip-row mono">
        <span class="tip-key label">Art</span>
        <span class="tip-val"
          >{tip.node.revision_art === "freigabe"
            ? "Freigabe · schreibgeschützt"
            : "Prototyp"}</span
        >
      </div>
      <div class="tip-row mono">
        <span class="tip-key label">Revision</span>
        <span class="tip-val">{tip.node.has_notes ? "mit Notiz" : "ohne Notiz"}</span>
      </div>
      <!-- Tag (E55): the honest git noun under a Revision — a Tag on this Commit (Glossar). The
           Revision is the domain word, the Tag carries its version label. Read-only readout. -->
      <div class="tip-row mono">
        <span class="tip-key label">Tag</span>
        <span class="tip-val">{tip.node.revision}</span>
      </div>
    {/if}
    {#if tip.node.offloaded}
      <div class="tip-row mono">
        <span class="tip-key label">Inhalt</span>
        <span class="tip-val">ausgelagert</span>
      </div>
    {/if}
    {#if tip.node.veroeffentlicht}
      <div class="tip-row mono">
        <span class="tip-key label">Linie</span>
        <span class="tip-val veroeff">veröffentlicht</span>
      </div>
      <!-- Push (E55): the honest git noun behind „veröffentlicht" — this Stand reached the shared
           line by being pushed there. „veröffentlicht" stays the domain headline; „Push" names the
           git act it took, read-only. No button: pushing remains its own deliberate handle. -->
      <div class="tip-row mono">
        <span class="tip-key label">Push</span>
        <span class="tip-val veroeff">auf geteilter Linie</span>
      </div>
    {/if}
  </div>
{/if}

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
      aria-label="Revision anlegen"
      tabindex="-1"
    >
      <header class="dialog-head">
        <span class="label">Revision</span>
        <span class="dialog-stand mono">{leaf(promoting.path)} · {clock(promoting.timestamp)}</span>
      </header>

      {#if promoting.revision !== null}
        <!-- Art-Toggle (E42): only for a Stand already promoted. Prototyp is the lax default;
             Freigabe write-protects the tag. Releasen / Un-Release is one deliberate handle. -->
        {@const freigabe = promoting.revision_art === "freigabe"}
        <div class="art-row">
          <div class="art-state">
            <span class="art-state-dot" class:freigabe aria-hidden="true"></span>
            <div class="art-state-text">
              <span class="art-state-name label">{freigabe ? "Freigabe" : "Prototyp"}</span>
              <span class="art-state-sub mono">
                {freigabe ? "schreibgeschützt — streng" : "lax — Warnungen, kein Block"}
              </span>
            </div>
          </div>
          <button
            class="key art-toggle label"
            class:solid={!freigabe}
            class:ghost={freigabe}
            onclick={toggleArt}
            disabled={busy}
            title={freigabe
              ? "Freigabe zurücknehmen (Un-Release)"
              : "Diese Revision freigeben"}
          >
            {busy ? "…" : freigabe ? "Zurückschalten" : "Freigeben"}
          </button>
        </div>
      {/if}

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
          placeholder="Was macht diesen Commit vorzeigbar?"
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
    padding: 4px 0 18px;
    position: relative;
    z-index: 1;
  }

  .idle {
    color: #5a564f;
    font-size: 12px;
    text-align: center;
    padding: 24px 14px;
  }

  /* The graph canvas: connectors live in one SVG layer behind the rows; each row carries
     its LED (positioned on its Bahn) and the data body to the right of the lane gutter. */
  .graph {
    position: relative;
  }
  .wires {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
    overflow: visible;
  }
  .wire {
    fill: none;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
    position: relative;
    z-index: 1;
  }

  .row {
    position: relative;
    height: var(--row);
    display: flex;
    align-items: center;
    padding-left: var(--lane-area);
    padding-right: 14px;
  }

  /* Node = LED, sitting exactly on its Bahn centre so the connectors meet it cleanly. A
     plain Stand is a small recessed grey LED; a Revision is a brighter, larger filled
     LED with a ring — the promoted node literally stands out on its line. */
  .led {
    position: absolute;
    left: var(--lane-x);
    top: 50%;
    transform: translate(-50%, -50%);
    width: 11px;
    height: 11px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    cursor: pointer;
    background: #4a4641;
    box-shadow:
      0 0 0 1px #000,
      inset 0 1px 0.5px rgba(255, 255, 255, 0.25);
    transition:
      box-shadow var(--dur) var(--ease),
      transform var(--dur) var(--ease);
  }
  .led:hover {
    transform: translate(-50%, -50%) scale(1.18);
  }
  .led:focus-visible {
    outline: none;
    box-shadow:
      0 0 0 1px #000,
      0 0 0 3px rgba(232, 230, 225, 0.45);
  }
  .led.ms {
    width: 15px;
    height: 15px;
    background: var(--screen-fg);
    box-shadow:
      0 0 0 1px #000,
      0 0 0 3px rgba(232, 230, 225, 0.12),
      0 0 6px 1px rgba(232, 230, 225, 0.35),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.7);
  }

  /* A diverging Zweig reads in the second colour (foreign blue), brightened a step per lane
     via the inline --c. The active line stays grey — clearly the "own" one. */
  .led.foreign {
    background: var(--c, var(--data-foreign));
    box-shadow:
      0 0 0 1px #000,
      0 0 5px 0.5px color-mix(in srgb, var(--c, var(--data-foreign)) 55%, transparent),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.35);
  }
  .led.foreign.ms {
    box-shadow:
      0 0 0 1px #000,
      0 0 0 3px color-mix(in srgb, var(--c, var(--data-foreign)) 16%, transparent),
      0 0 7px 1px color-mix(in srgb, var(--c, var(--data-foreign)) 60%, transparent),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.6);
  }

  /* Offloaded: honestly dimmed, content gone but the node remains (E36). */
  .led.offloaded {
    background: #322f2c;
    box-shadow: 0 0 0 1px #000;
  }
  .led.foreign.offloaded {
    background: #1c2330;
    box-shadow:
      0 0 0 1px #000,
      inset 0 1px 0.5px rgba(255, 255, 255, 0.12);
  }

  /* The active Stand (the tip of the active line) glows: a soft halo + a slow ping ring,
     so the eye lands on "where I am" without any colour beyond the line's own tone. */
  .led.active {
    box-shadow:
      0 0 0 1px #000,
      0 0 0 2px rgba(232, 230, 225, 0.5),
      0 0 10px 2px rgba(232, 230, 225, 0.4),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.7);
  }
  .led.active.foreign {
    box-shadow:
      0 0 0 1px #000,
      0 0 0 2px color-mix(in srgb, var(--c, var(--data-foreign)) 60%, transparent),
      0 0 10px 2px color-mix(in srgb, var(--c, var(--data-foreign)) 55%, transparent),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.5);
  }
  .ping {
    position: absolute;
    inset: -3px;
    border-radius: 50%;
    border: 1px solid rgba(232, 230, 225, 0.5);
    animation: ping 2.4s var(--ease) infinite;
    pointer-events: none;
  }
  .led.foreign .ping {
    border-color: color-mix(in srgb, var(--c, var(--data-foreign)) 60%, transparent);
  }
  @keyframes ping {
    0% {
      transform: scale(0.8);
      opacity: 0.7;
    }
    70%,
    100% {
      transform: scale(2.1);
      opacity: 0;
    }
  }

  .body {
    min-width: 0;
    flex: 1;
    padding: 4px 0;
  }
  .line {
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

  /* Version label + Art chip sit together at the right of the line. */
  .ms-tags {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  /* A Freigabe wears a small lock chip beside its version (E42) — the calm "schreibgeschützt"
     signal, in the screen-fg tone, never orange. A Prototyp shows nothing (the lax default). */
  .art-chip {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
    color: var(--screen-fg);
    background: rgba(232, 230, 225, 0.08);
    box-shadow: inset 0 0 0 1px rgba(232, 230, 225, 0.2);
  }
  .lock {
    position: relative;
    width: 8px;
    height: 9px;
    flex: none;
  }
  .lock::before {
    content: "";
    position: absolute;
    left: 1px;
    top: 3px;
    width: 6px;
    height: 5px;
    border-radius: 1px;
    background: currentColor;
  }
  .lock::after {
    content: "";
    position: absolute;
    left: 2px;
    top: 0;
    width: 4px;
    height: 5px;
    border: 1.2px solid currentColor;
    border-bottom: 0;
    border-radius: 3px 3px 0 0;
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

  /* Trailing state markers on the sub-line, grouped right so the time stays left and the badges
     sit together (instead of being spread by the row's space-between). */
  .marks {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  /* „veröffentlicht" — this Stand reached the shared line (E47, #30). Tied to the existing
     free/published green (--led-free), but desaturated toward warm grey so it stays a calm, settled
     readout next to the timestamp — never the warm Revision chip, never the foreign blue, and never
     orange (reserved for loud exceptions). Shown only when published; absence stays silent. */
  .veroeff-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: color-mix(in srgb, var(--led-free) 55%, #8f8c85);
  }
  /* A tiny lit uplink dot — the instrument-panel cue that the Stand has left the machine. */
  .veroeff-tag .uplink {
    flex: none;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--led-free);
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--led-free) 28%, transparent),
      0 0 4px color-mix(in srgb, var(--led-free) 50%, transparent);
  }
  /* Even on a foreign (Variante) line the publication readout keeps its green — being on the shared
     line is orthogonal to which line the Stand sits on, so the blue tint must not override it. */
  .row:has(.led.foreign) .veroeff-tag {
    color: color-mix(in srgb, var(--led-free) 55%, #8f8c85);
  }
  /* The detail-card echo: the value picks up the same calm green so the tip and the row agree. */
  .tip-val.veroeff {
    color: color-mix(in srgb, var(--led-free) 62%, #cfccc5);
  }

  /* A foreign Stand tints its path + version in the line's blue, so the body reads as part
     of the same diverging line as its LED. */
  .row:has(.led.foreign) .path {
    color: color-mix(in srgb, var(--data-foreign) 70%, #cfccc5);
  }
  .row:has(.led.foreign.ms) .path,
  .row:has(.led.foreign) .version {
    color: color-mix(in srgb, var(--data-foreign) 78%, #fff);
  }
  .row:has(.led.foreign) .version {
    background: color-mix(in srgb, var(--data-foreign) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--data-foreign) 34%, transparent);
  }
  .row:has(.led.foreign.offloaded) .path,
  .row:has(.led.foreign.offloaded) .version {
    color: #6d7585;
  }

  /* The Zweig name, shown once at the line's tip: a small foreign-blue caps tag naming the
     line in domain vocabulary (never "branch"). */
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

  @media (prefers-reduced-motion: reduce) {
    .led,
    .led:hover {
      transition: none;
      transform: translate(-50%, -50%);
    }
    .ping {
      animation: none;
      display: none;
    }
  }

  /* Hover/focus detail card — a small seated readout that floats over the chassis. Grey by
     default; a foreign Stand tints its frame in the line's blue. Domain words only. */
  .tip {
    position: fixed;
    z-index: 60;
    transform: translate(calc(-100% - 14px), -50%);
    min-width: 168px;
    max-width: 240px;
    padding: 9px 11px;
    background: linear-gradient(180deg, #17150f, #0d0c0b);
    border: 1px solid rgba(232, 230, 225, 0.16);
    border-radius: var(--radius);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    pointer-events: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .tip.foreign {
    border-color: color-mix(in srgb, var(--data-foreign) 45%, transparent);
  }
  .tip-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    padding-bottom: 5px;
    margin-bottom: 1px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }
  .tip-id {
    color: #8c8881;
    font-size: 11px;
    letter-spacing: 0.04em;
  }
  .tip-version {
    color: var(--screen-fg);
    font-size: 12px;
    font-weight: 600;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: rgba(232, 230, 225, 0.1);
  }
  .tip.foreign .tip-version {
    color: color-mix(in srgb, var(--data-foreign) 78%, #fff);
    background: color-mix(in srgb, var(--data-foreign) 16%, transparent);
  }
  .tip-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .tip-key {
    color: #6b6864;
    font-size: 9px;
  }
  .tip-val {
    color: #cfccc5;
    font-size: 11px;
  }

  /* Promote affordance lives on the node now; the dialog below is unchanged. */

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

  /* Art-Toggle row (E42): the current Prototyp/Freigabe state on the left, the deliberate
     toggle key on the right. A seated sunken strip on the chassis — calm, never orange. */
  .art-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 11px 12px;
    border-radius: var(--radius);
    background: var(--surface-sunken);
    box-shadow: inset 0 1px 2px rgba(28, 26, 25, 0.08);
  }
  .art-state {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }
  /* A small LED reading the Art: dim grey for Prototyp (lax, "an" but quiet), a brighter
     filled dot with a ring for Freigabe (released). Stays within the warm-grey palette. */
  .art-state-dot {
    width: 9px;
    height: 9px;
    flex: none;
    border-radius: 50%;
    background: var(--led-working);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
  }
  .art-state-dot.freigabe {
    background: var(--ink-strong);
    box-shadow:
      0 0 0 2px rgba(28, 26, 25, 0.12),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.25);
  }
  .art-state-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .art-state-name {
    color: var(--ink-strong);
  }
  .art-state-sub {
    color: var(--ink-muted);
    font-size: 10px;
    letter-spacing: 0;
  }
  .art-toggle {
    flex: none;
    padding: 7px 13px;
    /* Label swaps Freigeben ⇄ Zurückschalten ⇄ … (busy); pin to the widest so the key
       never resizes as its state changes. */
    min-width: 130px;
    text-align: center;
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
  /* Festschreiben collapses to … while busy; reserve its width so the footer doesn't twitch. */
  .dialog-actions .key {
    min-width: 116px;
    text-align: center;
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
     Revision is a considered act, so it reads as a seated dark key, not loud orange. */
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
