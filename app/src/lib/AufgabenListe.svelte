<script lang="ts">
  import Led from "./Led.svelte";
  import type { Task, TaskKind, TaskStatus, TaskLink } from "./types";

  // Aufgaben & Hinweise for the open product (Issue #40, PRD US 27–30). The two are separated
  // PURELY by Blockier-Fähigkeit, not importance: an Aufgabe *can* block, a Hinweis never does.
  // The block DECISION (the loud orange moment) is a later slice (Issue #49) — so this list stays
  // entirely grey/quiet: routine workshop chassis, never the rationed orange.
  let {
    tasks = [],
    // Optional candidate Verknüpfungen the create/edit picker can offer (product-relative paths
    // of Bausteine/Arbeitsbereiche). The Produkt + a free Version link are always available.
    artefakte = [],
    onCreate = (_t: NewTaskInput) => {},
    onEdit = (_id: string, _t: EditTaskInput) => {},
    onSetStatus = (_id: string, _s: TaskStatus) => {},
    onDelete = (_id: string) => {},
  }: {
    tasks?: Task[];
    artefakte?: { name: string; path: string }[];
    onCreate?: (t: NewTaskInput) => void;
    onEdit?: (id: string, t: EditTaskInput) => void;
    onSetStatus?: (id: string, s: TaskStatus) => void;
    onDelete?: (id: string) => void;
  } = $props();

  // ── Shapes handed back to the parent (which maps them to the Tauri commands). ─────────
  type NewTaskInput = {
    title: string;
    kind: TaskKind;
    link: TaskLink | null;
    due: string | null;
    blocks_everywhere: boolean;
  };
  type EditTaskInput = {
    title: string;
    kind: TaskKind;
    link: TaskLink | null;
    due: string | null;
    blocks_everywhere: boolean;
  };

  // Split by kind — the ONLY axis that separates the two (US 27). Open items first within each.
  const order = (s: TaskStatus) => (s === "offen" ? 0 : 1);
  const aufgaben = $derived(
    tasks.filter((t) => t.kind === "aufgabe").sort((a, b) => order(a.status) - order(b.status)),
  );
  const hinweise = $derived(
    tasks.filter((t) => t.kind === "hinweis").sort((a, b) => order(a.status) - order(b.status)),
  );
  const openAufgaben = $derived(aufgaben.filter((t) => t.status === "offen").length);

  // ── The "neu" form — hidden until asked (the quiet, opt-in gesture, like ArtifactCard). ──
  let creating = $state(false);
  let editingId = $state<string | null>(null);

  // One shared draft; reused for both create and edit so the markup stays single-source.
  type Draft = {
    title: string;
    kind: TaskKind;
    linkKind: "" | "produkt" | "version" | "arbeitsbereich" | "artefakt";
    linkRef: string;
    due: string;
    blocks_everywhere: boolean;
  };
  const emptyDraft = (): Draft => ({
    title: "",
    kind: "aufgabe",
    linkKind: "",
    linkRef: "",
    due: "",
    blocks_everywhere: false,
  });
  let draft = $state<Draft>(emptyDraft());

  function draftToLink(d: Draft): TaskLink | null {
    if (d.linkKind === "") return null;
    if (d.linkKind === "produkt") return { kind: "produkt" };
    const ref = d.linkRef.trim();
    if (!ref) return null; // a referencing link with no target is just "free-floating"
    return { kind: d.linkKind, ref };
  }

  function linkToDraft(link: TaskLink | null): Pick<Draft, "linkKind" | "linkRef"> {
    if (!link) return { linkKind: "", linkRef: "" };
    if (link.kind === "produkt") return { linkKind: "produkt", linkRef: "" };
    return { linkKind: link.kind, linkRef: link.ref };
  }

  function openCreate() {
    editingId = null;
    draft = emptyDraft();
    creating = true;
  }
  function openEdit(t: Task) {
    creating = false;
    editingId = t.id;
    draft = {
      title: t.title,
      kind: t.kind,
      due: t.due ?? "",
      blocks_everywhere: t.blocks_everywhere,
      ...linkToDraft(t.link),
    };
  }
  function cancel() {
    creating = false;
    editingId = null;
    draft = emptyDraft();
  }

  function submit() {
    const title = draft.title.trim();
    if (!title) return; // a task always keeps a Titel; the core refuses blanks too
    const payload = {
      title,
      kind: draft.kind,
      link: draftToLink(draft),
      due: draft.due.trim() || null,
      blocks_everywhere: draft.kind === "aufgabe" ? draft.blocks_everywhere : false,
    };
    if (editingId) onEdit(editingId, payload);
    else onCreate(payload);
    cancel();
  }

  // Human one-line summary of a Verknüpfung for the row (data → Mono).
  function linkLabel(link: TaskLink | null): string | null {
    if (!link) return null;
    if (link.kind === "produkt") return "Produkt";
    if (link.kind === "version") return `Version ${link.ref}`;
    if (link.kind === "arbeitsbereich") return `Bereich ${link.ref}`;
    return link.ref; // artefakt: the path is the most useful identity
  }

  // The LED vocabulary, kept deliberately grey: an open Aufgabe is "working" (live, in play); a
  // Hinweis a softer off; a settled (done/dropped) item is dimmed off. Orange is NOT used here —
  // the block itself is the loud exception, decided elsewhere (Issue #49).
  function led(t: Task): "free" | "working" | "off" {
    if (t.status === "erledigt") return "free"; // erledigt reads as the calm green "done"
    if (t.status === "verworfen") return "off";
    return t.kind === "aufgabe" ? "working" : "off";
  }
  function ledTitle(t: Task): string {
    if (t.status === "erledigt") return "erledigt";
    if (t.status === "verworfen") return "verworfen";
    return t.kind === "aufgabe" ? "offen — kann blockieren" : "Hinweis — blockiert nie";
  }
</script>

<section class="aufgaben" aria-label="Aufgaben & Hinweise">
  <div class="head">
    <span class="label title">Aufgaben &amp; Hinweise</span>
    <div class="head-right">
      {#if openAufgaben > 0}
        <span class="count mono" title="offene Aufgaben">
          {openAufgaben.toString().padStart(2, "0")} offen
        </span>
      {/if}
      {#if !creating && !editingId}
        <button class="key neu" onclick={openCreate}>
          <span class="label">+ neu</span>
        </button>
      {/if}
    </div>
  </div>

  {#if creating || editingId}
    <!-- The form: hidden until asked. Aufgabe vs. Hinweis is the first, primary choice — it is
         the whole distinction. „blockiert überall" only shows for an Aufgabe (US 30). -->
    <form class="form" onsubmit={(e) => { e.preventDefault(); submit(); }}>
      <div class="kindtoggle" role="radiogroup" aria-label="Typ">
        <button
          type="button"
          class="seg"
          class:on={draft.kind === "aufgabe"}
          aria-pressed={draft.kind === "aufgabe"}
          onclick={() => (draft.kind = "aufgabe")}
        >
          <span class="label">Aufgabe</span>
          <span class="seg-sub">kann blockieren</span>
        </button>
        <button
          type="button"
          class="seg"
          class:on={draft.kind === "hinweis"}
          aria-pressed={draft.kind === "hinweis"}
          onclick={() => (draft.kind = "hinweis")}
        >
          <span class="label">Hinweis</span>
          <span class="seg-sub">blockiert nie</span>
        </button>
      </div>

      <input
        class="field title-field mono"
        type="text"
        placeholder="Titel …"
        bind:value={draft.title}
        aria-label="Titel"
      />

      <div class="row-fields">
        <label class="fl">
          <span class="label fl-cap">Verknüpfung</span>
          <select class="field mono" bind:value={draft.linkKind} aria-label="Verknüpfung">
            <option value="">— keine —</option>
            <option value="produkt">Produkt</option>
            <option value="version">Version</option>
            <option value="arbeitsbereich">Arbeitsbereich</option>
            <option value="artefakt">Artefakt</option>
          </select>
        </label>

        {#if draft.linkKind === "version"}
          <label class="fl">
            <span class="label fl-cap">Version</span>
            <input class="field mono" type="text" placeholder="z. B. Rev B" bind:value={draft.linkRef} />
          </label>
        {:else if draft.linkKind === "arbeitsbereich" || draft.linkKind === "artefakt"}
          <label class="fl">
            <span class="label fl-cap">{draft.linkKind === "artefakt" ? "Artefakt" : "Bereich"}</span>
            {#if artefakte.length > 0}
              <select class="field mono" bind:value={draft.linkRef}>
                <option value="" disabled selected>wählen …</option>
                {#each artefakte as a (a.path)}
                  <option value={a.path}>{a.name} — {a.path}</option>
                {/each}
              </select>
            {:else}
              <input class="field mono" type="text" placeholder="Pfad …" bind:value={draft.linkRef} />
            {/if}
          </label>
        {/if}

        <label class="fl due">
          <span class="label fl-cap">Fälligkeit</span>
          <input class="field mono" type="date" bind:value={draft.due} aria-label="Fälligkeit" />
        </label>
      </div>

      {#if draft.kind === "aufgabe"}
        <!-- US 30: the rare context-independent opt-out. A quiet checkbox, no orange. -->
        <label class="check">
          <input type="checkbox" bind:checked={draft.blocks_everywhere} />
          <span class="check-text label">blockiert überall</span>
          <span class="check-sub">kontextunabhängig</span>
        </label>
      {/if}

      <div class="form-actions">
        <button type="submit" class="key primary" disabled={!draft.title.trim()}>
          <span class="label">{editingId ? "übernehmen" : "anlegen"}</span>
        </button>
        <button type="button" class="link" onclick={cancel}>abbrechen</button>
      </div>
    </form>
  {/if}

  <div class="lists">
    {#if tasks.length === 0 && !creating}
      <p class="idle label">Keine Aufgaben oder Hinweise — „+ neu" legt eine an</p>
    {/if}

    {#each [{ key: "aufgabe", heading: "Aufgaben", items: aufgaben }, { key: "hinweis", heading: "Hinweise", items: hinweise }] as group (group.key)}
      {#if group.items.length > 0}
        <div class="group">
          <div class="group-head">
            <span class="label group-title">{group.heading}</span>
            <span class="group-sub">{group.key === "aufgabe" ? "kann blockieren" : "blockiert nie"}</span>
          </div>

          {#each group.items as t (t.id)}
            <article class="task" class:done={t.status !== "offen"}>
              <div class="task-main">
                <Led status={led(t)} title={ledTitle(t)} />
                <div class="task-body">
                  <div class="task-top">
                    <span class="task-title mono" class:struck={t.status !== "offen"}>{t.title}</span>
                    {#if t.blocks_everywhere && t.kind === "aufgabe"}
                      <span class="badge label" title="blockiert kontextunabhängig">überall</span>
                    {/if}
                    {#if t.status !== "offen"}
                      <span class="state label">{t.status}</span>
                    {/if}
                  </div>
                  {#if linkLabel(t.link) || t.due}
                    <div class="task-meta mono">
                      {#if linkLabel(t.link)}
                        <span class="ml" title="Verknüpfung">{linkLabel(t.link)}</span>
                      {/if}
                      {#if t.due}
                        <span class="md" title="Fälligkeit">fällig {t.due}</span>
                      {/if}
                    </div>
                  {/if}
                </div>
              </div>

              <div class="task-actions">
                {#if t.status === "offen"}
                  <button class="tk" onclick={() => onSetStatus(t.id, "erledigt")} title="erledigen">erledigt</button>
                  <button class="tk" onclick={() => onSetStatus(t.id, "verworfen")} title="verwerfen">verwerfen</button>
                {:else}
                  <button class="tk" onclick={() => onSetStatus(t.id, "offen")} title="wieder öffnen">öffnen</button>
                {/if}
                <button class="tk" onclick={() => openEdit(t)} title="bearbeiten">bearbeiten</button>
                <button class="tk del" onclick={() => onDelete(t.id)} title="löschen" aria-label="löschen">×</button>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    {/each}
  </div>
</section>

<style>
  /* Warm-grey workshop chassis — the same instrument language as the cards, not the dark LCD.
     Tasks are routine work material, so this stays entirely grey: orange is reserved for the
     loud block exception, which is decided elsewhere (Issue #49). */
  .aufgaben {
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    background: var(--surface-raised);
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 11px 14px;
    border-bottom: 1px solid var(--hairline);
  }
  .title {
    color: var(--ink-muted);
  }
  .head-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .count {
    color: var(--ink-default);
    font-size: 12px;
    font-weight: 600;
  }

  /* The neutral creme "key" — quiet, physical, seated (Stilbeschreibung §Tasten). */
  .key {
    appearance: none;
    cursor: pointer;
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
  .key:hover {
    background: #f5f3ee;
  }
  .key:active {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.1);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.5;
    box-shadow: none;
  }
  .key .label {
    color: inherit;
    font-size: 10px;
  }
  .key.primary {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
  }
  .key.primary:hover {
    background: #2a2724;
  }

  /* ── Form ───────────────────────────────────────────────────────────────────────────── */
  .form {
    display: flex;
    flex-direction: column;
    gap: 11px;
    padding: 14px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface-base);
    animation: drop 220ms var(--ease) backwards;
  }

  /* The Aufgabe/Hinweis choice — the whole distinction, so it leads. A two-segment toggle. */
  .kindtoggle {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .seg {
    appearance: none;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 3px;
    align-items: flex-start;
    text-align: left;
    padding: 8px 11px;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    background: var(--surface-raised);
    color: var(--ink-muted);
    transition:
      border-color var(--dur) var(--ease),
      background var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .seg .label {
    color: inherit;
    font-size: 11px;
  }
  .seg-sub {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--ink-muted);
    opacity: 0.8;
  }
  .seg:hover {
    border-color: var(--key-mid);
  }
  /* The selected segment seats inward with a dark cap — a pressed physical switch. */
  .seg.on {
    background: var(--key-dark);
    border-color: var(--key-dark);
    color: var(--key-light);
  }
  .seg.on .seg-sub {
    color: #b8b4ad;
    opacity: 1;
  }

  .field {
    background: var(--surface-sunken);
    color: var(--ink-default);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 6px 8px;
    font-size: 12px;
    min-width: 0;
  }
  .field:focus {
    outline: none;
    border-color: var(--key-mid);
  }
  .title-field {
    width: 100%;
    font-size: 13px;
  }

  .row-fields {
    display: flex;
    flex-wrap: wrap;
    gap: 9px;
  }
  .fl {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 130px;
  }
  .fl.due {
    flex: 0 0 auto;
  }
  .fl-cap {
    color: var(--ink-muted);
    font-size: 9px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }
  .check input {
    accent-color: var(--key-dark);
  }
  .check-text {
    color: var(--ink-default);
    font-size: 10px;
  }
  .check-sub {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--ink-muted);
  }

  .form-actions {
    display: flex;
    align-items: center;
    gap: 12px;
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
    transition: color var(--dur) var(--ease);
  }
  .link:hover {
    color: var(--ink-strong);
  }

  /* ── Lists ──────────────────────────────────────────────────────────────────────────── */
  .lists {
    display: flex;
    flex-direction: column;
  }
  .idle {
    color: var(--ink-muted);
    padding: 16px 14px;
    text-transform: none;
    letter-spacing: 0;
    font-size: 12px;
  }

  .group + .group {
    border-top: 1px solid var(--hairline);
  }
  .group-head {
    display: flex;
    align-items: baseline;
    gap: 9px;
    padding: 9px 14px 7px;
  }
  .group-title {
    color: var(--ink-muted);
    font-size: 10px;
  }
  .group-sub {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: color-mix(in srgb, var(--ink-muted) 75%, transparent);
  }

  /* Each task: a seated row, LED + title + quiet meta, actions revealed on hover. */
  .task {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
    padding: 9px 14px;
  }
  .task + .task {
    border-top: 1px solid color-mix(in srgb, var(--hairline) 55%, transparent);
  }
  .task:hover {
    background: color-mix(in srgb, var(--surface-base) 60%, transparent);
  }
  .task.done {
    opacity: 0.7;
  }

  .task-main {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
    padding-top: 2px;
  }
  .task-body {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .task-top {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .task-title {
    color: var(--ink-strong);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .task-title.struck {
    text-decoration: line-through;
    color: var(--ink-muted);
  }
  /* A quiet recessed badge for „blockiert überall" — grey, not orange (the flag, not the block). */
  .badge {
    flex: none;
    font-size: 8.5px;
    color: var(--ink-default);
    background: var(--surface-sunken);
    border: 1px solid var(--hairline);
    border-radius: 99px;
    padding: 1.5px 7px;
  }
  .state {
    flex: none;
    font-size: 9px;
    color: var(--ink-muted);
  }
  .task-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 10.5px;
  }
  .ml {
    color: var(--ink-default);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 220px;
  }
  .md {
    color: var(--ink-muted);
  }

  .task-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .task:hover .task-actions,
  .task:focus-within .task-actions {
    opacity: 1;
  }
  .tk {
    appearance: none;
    cursor: pointer;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--ink-muted);
    font-family: var(--font-label);
    text-transform: uppercase;
    letter-spacing: 0.03em;
    font-size: 9px;
    font-weight: 600;
    padding: 4px 7px;
    transition:
      color var(--dur) var(--ease),
      border-color var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .tk:hover {
    color: var(--ink-strong);
    border-color: var(--hairline);
    background: var(--surface-raised);
  }
  .tk.del {
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1;
    padding: 2px 7px;
  }

  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
