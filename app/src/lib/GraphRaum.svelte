<script lang="ts">
  import type {
    GraphFilter,
    StandNode,
    VersionGraph,
  } from "./types";
  import VersionTree from "./VersionTree.svelte";
  import GateTaste from "./GateTaste.svelte";

  // The Graph-Raum (Issue #55, E45): a SEPARATE room from the Werkbank — the user *sucht ihn auf*,
  // it is not the start screen. It renders the version tree (the existing VersionTree, reused, not
  // duplicated) and adds the two pure display filters (E45) plus the three Knoten-Verben (E27). A
  // click on an old node never silently moves the Werkbank; it offers the verbs in a calm popover.
  let {
    graph,
    onPromote,
    onToggleArt,
    onOpenAsFolder,
    onBranchFrom,
    onThrowBack,
  }: {
    graph: VersionGraph | null;
    onPromote: (node: StandNode, version: string, notes: string) => Promise<void>;
    onToggleArt: (node: StandNode) => Promise<void>;
    /** „Als Ordner öffnen" (Default): materialise a read-only worktree, return its path to open. */
    onOpenAsFolder: (node: StandNode) => Promise<void>;
    /** „Von hier abzweigen": save current work, then create the named branch. */
    onBranchFrom: (node: StandNode, branch: string) => Promise<void>;
    /** „Zurückwerfen" (destructive, behind the black gate): the SAFE restore. */
    onThrowBack: (node: StandNode) => Promise<void>;
  } = $props();

  // The two pure display filters (E45). Default = everything visible; they only hide, never write.
  let filter = $state<GraphFilter>({ varianten: true, nur_revisionen: false });

  // The open node-verb menu, anchored to the clicked LED's screen rect. Domain words only.
  let menu = $state<{ node: StandNode; x: number; y: number } | null>(null);
  let branchName = $state("");
  let busy = $state<null | "ordner" | "abzweigen" | "zurueck">(null);
  let verbError = $state<string | null>(null);

  function openMenu(node: StandNode, anchor: DOMRect) {
    // The graph is the right-most column; the menu opens to the LEFT of the LED to stay on screen,
    // vertically centred on the node. Same placement language as the hover detail card.
    menu = { node, x: anchor.left, y: anchor.top + anchor.height / 2 };
    branchName = "";
    verbError = null;
    busy = null;
  }
  function closeMenu() {
    if (busy) return; // don't yank the menu out from under a running verb
    menu = null;
    verbError = null;
  }

  // Which verbs this node allows — mirrors the Rust `allowed_verbs` core (knotenverben.rs): the
  // active tip can't be thrown back (you're already there); an offloaded node has no content to
  // materialise/branch/restore, so no verb applies (E36).
  const activeTipId = $derived(graph?.nodes.find((n) => n.on_active)?.id ?? null);
  const canOpenOrBranch = $derived(menu ? !menu.node.offloaded : false);
  const canThrowBack = $derived(
    menu ? !menu.node.offloaded && menu.node.id !== activeTipId : false,
  );

  function leaf(path: string): string {
    if (path === "." || path === "") return "Produkt";
    const parts = path.split("/");
    return parts[parts.length - 1];
  }
  function clock(ts: string): string {
    return ts.slice(11, 19) || ts;
  }
  function day(ts: string): string {
    return ts.slice(0, 10);
  }

  async function doOpenFolder() {
    if (!menu || busy) return;
    busy = "ordner";
    verbError = null;
    try {
      await onOpenAsFolder(menu.node);
      menu = null;
    } catch (e) {
      verbError = String(e);
    } finally {
      busy = null;
    }
  }

  async function doBranch() {
    if (!menu || busy) return;
    const name = branchName.trim();
    if (!name) {
      verbError = "Der Zweig braucht einen Namen";
      return;
    }
    busy = "abzweigen";
    verbError = null;
    try {
      await onBranchFrom(menu.node, name);
      menu = null;
    } catch (e) {
      verbError = String(e);
    } finally {
      busy = null;
    }
  }

  async function doThrowBack() {
    if (!menu || busy) return;
    busy = "zurueck";
    verbError = null;
    try {
      await onThrowBack(menu.node);
      menu = null;
    } catch (e) {
      verbError = String(e);
    } finally {
      busy = null;
    }
  }

  function onMenuKey(ev: KeyboardEvent) {
    if (ev.key === "Escape") closeMenu();
  }
</script>

<section class="raum" aria-label="Graph-Raum · Verlauf">
  <!-- Filter bar: a thin instrument strip on the dark screen. Two LED-toggle switches, pure
       display (E45) — grey when off, lit screen-fg when on. No orange: routine view, not alarm. -->
  <div class="filterbar" role="group" aria-label="Graph-Filter">
    <span class="fb-label label">Filter</span>
    <button
      type="button"
      class="toggle"
      class:on={filter.varianten}
      role="switch"
      aria-checked={filter.varianten}
      title="Varianten (Zweige neben der aktiven Linie) ein-/ausblenden"
      onclick={() => (filter = { ...filter, varianten: !filter.varianten })}
    >
      <span class="t-led" aria-hidden="true"></span>
      <span class="t-text label">Varianten</span>
    </button>
    <button
      type="button"
      class="toggle"
      class:on={filter.nur_revisionen}
      role="switch"
      aria-checked={filter.nur_revisionen}
      title="Nur Revisionen zeigen"
      onclick={() => (filter = { ...filter, nur_revisionen: !filter.nur_revisionen })}
    >
      <span class="t-led" aria-hidden="true"></span>
      <span class="t-text label">nur Revisionen</span>
    </button>
  </div>

  <div class="tree-wrap">
    <VersionTree
      {graph}
      {filter}
      {onPromote}
      {onToggleArt}
      onNodeAction={openMenu}
      title="Verlauf · Graph"
    />
  </div>
</section>

{#if menu}
  <!-- Node-verb menu: a calm seated popover on the dark screen, opening to the LEFT of the LED.
       „Als Ordner öffnen" is the quiet default at the top; „abzweigen" a deliberate dark key;
       „Zurückwerfen" is walled off in its own recessed danger zone, armed via the black GateTaste
       (E38/E43). A click on the node NEVER moves the Werkbank — only a verb does. -->
  <div class="scrim" role="presentation" onclick={closeMenu} onkeydown={onMenuKey}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="menu"
      role="menu"
      tabindex="-1"
      aria-label="Verben für diesen Stand"
      style="left: {menu.x}px; top: {menu.y}px;"
      onclick={(e) => e.stopPropagation()}
    >
      <header class="menu-head">
        <span class="mh-id mono">{menu.node.id.slice(0, 8)}</span>
        {#if menu.node.revision !== null}
          <span class="mh-version mono">{menu.node.revision}</span>
        {/if}
        <span class="mh-meta mono">{leaf(menu.node.path)} · {day(menu.node.timestamp)} {clock(menu.node.timestamp)}</span>
      </header>

      {#if !canOpenOrBranch}
        <p class="menu-note mono">Inhalt ausgelagert — kein Verb möglich</p>
      {:else}
        <!-- Default verb, top, calm. -->
        <button
          type="button"
          class="verb default"
          role="menuitem"
          disabled={busy !== null}
          onclick={doOpenFolder}
        >
          <span class="v-main label">{busy === "ordner" ? "öffne …" : "Als Ordner öffnen"}</span>
          <span class="v-sub mono">schreibgeschützte Kopie daneben — Werkbank ruht</span>
        </button>

        <!-- Branch from here: a deliberate dark seated key with its own name field. -->
        <div class="verb branch">
          <div class="v-row">
            <span class="v-main label">Von hier abzweigen</span>
            <span class="v-sub mono">neuer Branch — laufende Arbeit wird vorher gesichert</span>
          </div>
          <div class="branch-row">
            <input
              class="input mono"
              bind:value={branchName}
              placeholder="zweig-name"
              spellcheck="false"
              autocomplete="off"
              disabled={busy !== null}
              onkeydown={(e) => e.key === "Enter" && doBranch()}
            />
            <button
              type="button"
              class="key dark"
              disabled={busy !== null || branchName.trim() === ""}
              onclick={doBranch}
            >
              {busy === "abzweigen" ? "…" : "abzweigen"}
            </button>
          </div>
        </div>

        {#if canThrowBack}
          <!-- Zurückwerfen: destructive, NEVER the default — walled off behind the black gate.
               The verb is safe by construction (the backend lays the old Stand on top as a new
               forward Stand, no reset/rebase), but the act is heavy, so it carries the gate. -->
          <div class="verb danger-zone">
            <div class="v-row">
              <span class="v-main danger-title label">Zurückwerfen</span>
              <span class="v-sub mono">springt auf diesen Stand — als neuer Stand obendrauf, reversibel</span>
            </div>
            <GateTaste
              consent="Ich werfe bewusst auf diesen Stand zurück"
              label="Zurückwerfen"
              busyLabel="werfe zurück …"
              busy={busy === "zurueck"}
              disabled={busy !== null && busy !== "zurueck"}
              onPress={doThrowBack}
            />
          </div>
        {/if}
      {/if}

      {#if verbError}
        <p class="menu-error mono">{verbError}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* The Graph-Raum is a full room: the filter strip seated atop the dark instrument display,
     the version tree filling the rest. Same recessed-screen language as VersionTree itself. */
  .raum {
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    min-height: 0;
  }

  /* Filter bar — a thin dark instrument strip. Reads as part of the screen, not the chassis. */
  .filterbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 14px;
    background: linear-gradient(180deg, #161412, #0d0c0b);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }
  .fb-label {
    color: #6b6660;
    margin-right: 2px;
  }

  /* An LED-toggle switch: a recessed pill with a status LED + caps label. Off = dim grey;
     on = the LED lights screen-fg and the pill brightens. Strictly grey — never orange. */
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 5px 10px 5px 8px;
    border-radius: 99px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
    cursor: pointer;
    transition:
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .toggle:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .toggle:focus-visible {
    outline: none;
    border-color: rgba(232, 230, 225, 0.4);
  }
  .t-led {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: #3a3733;
    box-shadow: inset 0 0 0 1px #000;
    transition:
      background var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
  }
  .toggle.on .t-led {
    background: var(--screen-fg);
    box-shadow:
      0 0 0 1px #000,
      0 0 6px 1px rgba(232, 230, 225, 0.45);
  }
  .t-text {
    color: #8c8881;
    font-size: 10px;
    text-transform: none;
    letter-spacing: 0.02em;
    transition: color var(--dur) var(--ease);
  }
  .toggle.on .t-text {
    color: var(--screen-fg);
  }

  .tree-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  /* The embedded VersionTree fills the room and drops its own left seam (the room owns edges). */
  .tree-wrap > :global(.display) {
    width: 100%;
    flex: 1;
    border-left: none;
  }

  /* Node-verb menu — a seated popover on the dark screen, like the hover card but interactive. */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 55;
  }
  .menu {
    position: fixed;
    transform: translate(calc(-100% - 14px), -50%);
    width: 290px;
    max-width: calc(100vw - 32px);
    padding: 12px;
    background: linear-gradient(180deg, #17150f, #0c0b0a);
    border: 1px solid rgba(232, 230, 225, 0.16);
    border-radius: var(--radius);
    box-shadow: 0 16px 44px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .menu-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
    padding-bottom: 9px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  .mh-id {
    color: #8c8881;
    font-size: 11px;
    letter-spacing: 0.04em;
  }
  .mh-version {
    color: var(--screen-fg);
    font-size: 12px;
    font-weight: 600;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: rgba(232, 230, 225, 0.1);
  }
  .mh-meta {
    flex-basis: 100%;
    color: #6b6660;
    font-size: 10px;
  }

  .menu-note {
    color: #8c8881;
    font-size: 11px;
    margin: 2px 0;
  }

  /* A verb row: the default „Als Ordner öffnen" is a calm full-width seated key; the others
     carry their own controls (a name field / the black gate). */
  .verb {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }
  .verb.default {
    cursor: pointer;
    text-align: left;
    transition: background var(--dur) var(--ease);
  }
  .verb.default:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
  }
  .verb.default:focus-visible {
    outline: none;
    border-color: rgba(232, 230, 225, 0.4);
  }
  .verb:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .v-row {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .v-main {
    color: var(--screen-fg);
    font-size: 12px;
    text-transform: none;
    letter-spacing: 0.01em;
  }
  .v-sub {
    color: #6b6660;
    font-size: 9.5px;
    line-height: 1.35;
  }

  .branch-row {
    display: flex;
    gap: 6px;
  }
  .input {
    flex: 1;
    min-width: 0;
    appearance: none;
    background: #0b0a09;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: var(--radius-sm);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.8);
    color: var(--screen-fg);
    padding: 6px 8px;
    font-size: 12px;
  }
  .input:focus {
    outline: none;
    border-color: rgba(232, 230, 225, 0.4);
  }
  .input:disabled {
    opacity: 0.5;
  }
  /* The deliberate dark seated key for „abzweigen" — considered act, never orange (E43). */
  .key.dark {
    flex: none;
    appearance: none;
    cursor: pointer;
    background: var(--screen-fg);
    color: var(--key-dark);
    border: 1px solid var(--screen-fg);
    border-radius: var(--radius-sm);
    padding: 6px 12px;
    /* „abzweigen" collapses to … while busy; hold the width so the key doesn't shrink mid-action. */
    min-width: 84px;
    text-align: center;
    font-family: var(--font-label);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    transition:
      transform var(--dur) var(--ease),
      opacity var(--dur) var(--ease);
  }
  .key.dark:active {
    transform: translateY(1px);
  }
  .key.dark:disabled {
    cursor: default;
    opacity: 0.4;
  }

  /* The danger zone for „Zurückwerfen": a recessed dark plate that hosts the black GateTaste.
     Walled off from the calmer verbs above it. */
  .danger-zone {
    background: linear-gradient(180deg, #131110, #0a0908);
    border-color: #2a2724;
  }
  .danger-title {
    color: #b8b4ad;
  }

  .menu-error {
    color: var(--accent);
    font-size: 11px;
    margin: 0;
  }

  /* The black GateTaste inside the menu spans the row width for a clean walled-off zone. */
  .danger-zone :global(.danger) {
    width: 100%;
  }
</style>
