<script lang="ts">
  // Voll-Editor eines Bibliothek-Bausteins (Issue #108, Slice 2 — BEARBEITEN, Slice 3 — ANLEGEN).
  // Voll-Flächen-Editor mit Modus-Tabs (Grunddaten · Artefakte · Aufgaben · Kanten), gepfropft aus dem
  // Prototyp-Takeover (VariantC). Alle Felder editierbar INKL. der strukturierten — AUSSER `stillgelegt`
  // (das ist ein Produkt-Stack-Label und wird hier nie geschrieben). `version` ist in der UI unsichtbar.
  //
  // Zwei Modi, unterschieden über das `create`-Prop:
  //   • BEARBEITEN (create=false): die `id` ist unveränderlich (Dateiname + Stack-Referenz), wird nur
  //     angezeigt; keine Eindeutigkeitsprüfung.
  //   • ANLEGEN (create=true): die `id` ist editierbar und sichtbar, wird live aus dem Namen als Kebab
  //     abgeleitet (Umlaute transliteriert), bis der Nutzer sie selbst überschreibt — danach bleibt sie
  //     stehen. Anlege-Eindeutigkeit wird live geprüft (Rust bleibt die Autorität).
  //
  // Speichern läuft über `cmd.saveBausteinCmd`; die Validierung spiegelt den reinen Rust-Kern
  // (validate.ts), der Rust-Kern bleibt die Autorität.
  import type { Baustein } from "$lib/types";
  import { validate, emptyBaustein, toKebab, type BausteinVoll } from "./validate";
  import GlobListe from "./parts/GlobListe.svelte";
  import MusterListe from "./parts/MusterListe.svelte";
  import OeffnenWahl from "./parts/OeffnenWahl.svelte";
  import StartaufgabenEditor from "./parts/StartaufgabenEditor.svelte";
  import KantenEditor from "./parts/KantenEditor.svelte";
  import PaarKantenEditor from "./parts/PaarKantenEditor.svelte";

  let {
    baustein,
    bausteine,
    create = false,
    onSave,
    onCancel,
  }: {
    /** Der zu bearbeitende bzw. der leere Anlege-Entwurf (für create=true: `emptyBaustein()`). */
    baustein: Baustein;
    /** Die ganze Bibliothek — Quelle für die Partner-Dropdown und die Existenz-Warnung. */
    bausteine: Baustein[];
    /** Anlege-Modus (Slice 3): `id` editierbar + aus dem Namen abgeleitet, Eindeutigkeit geprüft. */
    create?: boolean;
    /** Speichert den Entwurf; löst beim Erfolg den Stage-Wechsel zurück zur Galerie aus. */
    onSave: (b: Baustein) => Promise<void>;
    /** Verwirft den Entwurf, zurück zur Galerie. */
    onCancel: () => void;
  } = $props();

  // Entwurf als voll-erforderliche Form; $state-Proxy via snapshot klonen, bevor structuredClone greift.
  // Der Editor wird pro bearbeitetem Baustein frisch montiert, daher genügt der Anfangswert von
  // `baustein` als Saat — spätere Prop-Änderungen sind hier bewusst nicht zu spiegeln.
  // svelte-ignore state_referenced_locally
  let draft = $state<BausteinVoll>({ ...emptyBaustein(), ...structuredClone($state.snapshot(baustein)) });
  let saving = $state(false);
  let saveError = $state<string | null>(null);

  // Anlege-Modus: die Kennung wird live aus dem Namen als Kebab abgeleitet, bis der Nutzer sie selbst
  // anfasst (typisches Slug-Verhalten). Danach bleibt die Hand-Eingabe stehen — keine Auto-Ableitung mehr.
  let idTouched = $state(false);
  function onNameInput() {
    if (create && !idTouched) draft.id = toKebab(draft.name);
  }
  function onIdInput() {
    idTouched = true;
  }

  type Mode = "grund" | "artefakte" | "aufgaben" | "kanten";
  let mode = $state<Mode>("grund");
  const modes: { k: Mode; label: string }[] = [
    { k: "grund", label: "Grunddaten" },
    { k: "artefakte", label: "Artefakte" },
    { k: "aufgaben", label: "Aufgaben" },
    { k: "kanten", label: "Kanten" },
  ];

  // Partner-Dropdown: alle anderen Bausteine (sich selbst ausgeschlossen).
  let partners = $derived(
    bausteine.filter((b) => b.id !== draft.id).map((b) => ({ id: b.id, name: b.name })),
  );
  // Anlegen (Slice 3) ⇒ isCreate = true: Live-Eindeutigkeitsprüfung der Kennung. Bearbeiten ⇒ false.
  let check = $derived(validate(draft, bausteine, create));
  let errCount = $derived(Object.keys(check.errors).length);
  let canSave = $derived(errCount === 0 && !saving);

  async function save() {
    if (!canSave) return;
    saving = true;
    saveError = null;
    try {
      // Auf `Baustein` zuweisbar (BausteinVoll = Required<Baustein>). `stillgelegt` bleibt unangetastet
      // wie geladen; `version` bleibt unangetastet — beide werden hier nie aus der UI verändert.
      await onSave(structuredClone($state.snapshot(draft)) as Baustein);
    } catch (e) {
      saveError = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="takeover">
  <header class="thead2">
    <div class="crumb">
      <button class="back" onclick={onCancel}><span class="label">‹ Bibliothek</span></button>
      <span class="sep">/</span>
      <span class="label crumbnow">{draft.name || "Ohne Namen"}</span>
    </div>
    <div class="idline">
      <span class="mono idval">{draft.id || "—"}</span>
      <span class="label idtag">{create ? "neu" : "fest"}</span>
    </div>
  </header>

  <nav class="modes">
    {#each modes as m (m.k)}
      <button class="mtab" class:on={mode === m.k} onclick={() => (mode = m.k)}>
        <span class="label">{m.label}</span>
        {#if m.k === "grund" && (check.errors.name || check.errors.id || check.errors.heimat)}<span class="dot"></span>{/if}
        {#if m.k === "artefakte" && check.errors.globs}<span class="dot"></span>{/if}
        {#if m.k === "kanten" && (check.errors.default_kanten || check.errors.paar_default_kanten)}<span class="dot"></span>{/if}
      </button>
    {/each}
  </nav>

  <div class="panel">
    {#if mode === "grund"}
      <div class="pane narrow">
        <div class="fld">
          <span class="label fl">Name</span>
          <input class="in" bind:value={draft.name} oninput={onNameInput} placeholder="z.B. KiCad" />
          {#if check.errors.name}<span class="err label">{check.errors.name}</span>{/if}
        </div>
        <div class="fld">
          <span class="label fl">Kennung</span>
          {#if create}
            <input class="in mono" bind:value={draft.id} oninput={onIdInput} placeholder="z.B. kicad" />
            <span class="hint">Dateiname und Stack-Referenz — aus dem Namen abgeleitet, anpassbar</span>
          {:else}
            <div class="frozen mono">{draft.id}</div>
            <span class="hint">fest — Dateiname und Stack-Referenz</span>
          {/if}
          {#if check.errors.id}<span class="err label">{check.errors.id}</span>{/if}
        </div>
        <div class="fld">
          <span class="label fl">Heimat</span>
          <input class="in mono" bind:value={draft.heimat} placeholder="z.B. elektronik" />
          {#if check.errors.heimat}<span class="err label">{check.errors.heimat}</span>{/if}
        </div>
        <div class="fld">
          <span class="label fl">Öffnen-Aktion</span>
          <OeffnenWahl bind:value={draft.oeffnen} />
        </div>
      </div>
    {:else if mode === "artefakte"}
      <div class="pane">
        <div class="fld">
          <span class="label fl">Artefakt-Muster <span class="sub">geordnet — das erste ist die Hauptdatei</span></span>
          <GlobListe bind:globs={draft.globs} />
          {#if check.errors.globs}<span class="err label">{check.errors.globs}</span>{/if}
        </div>
        <div class="two">
          <div class="fld">
            <span class="label fl">Ignorieren</span>
            <MusterListe bind:items={draft.ignore} />
          </div>
          <div class="fld">
            <span class="label fl">Große Dateien</span>
            <MusterListe bind:items={draft.lfs} />
          </div>
        </div>
      </div>
    {:else if mode === "aufgaben"}
      <div class="pane">
        <div class="fld">
          <span class="label fl">Startaufgaben <span class="sub">werden beim Onboarding in ein Produkt angelegt</span></span>
          <StartaufgabenEditor bind:items={draft.startaufgaben} />
        </div>
      </div>
    {:else}
      <div class="pane">
        <div class="fld">
          <span class="label fl">Default-Kanten <span class="sub">abgeleitet ← Quelle, im eigenen Arbeitsbereich</span></span>
          <KantenEditor bind:items={draft.default_kanten} />
          {#if check.errors.default_kanten}<span class="err label">{check.errors.default_kanten}</span>{/if}
        </div>
        <div class="fld">
          <span class="label fl">Paar-Default-Kanten <span class="sub">greift, sobald der Partner-Baustein mit im Stack liegt</span></span>
          <PaarKantenEditor bind:items={draft.paar_default_kanten} {partners} />
          {#if check.errors.paar_default_kanten}<span class="err label">{check.errors.paar_default_kanten}</span>{/if}
        </div>
      </div>
    {/if}
  </div>

  <footer class="tfoot">
    <span class="status label">
      {#if saveError}
        <span class="bad">{saveError}</span>
      {:else if errCount > 0}
        <span class="bad">{errCount} {errCount === 1 ? "Feld" : "Felder"} prüfen</span>
      {:else if check.warnings.length > 0}
        {check.warnings[0]}
      {:else}
        bereit
      {/if}
    </span>
    <div class="keys">
      <button class="ghost" onclick={onCancel}><span class="label">Abbrechen</span></button>
      <button class="go" onclick={save} disabled={!canSave}>
        <span class="label">{saving ? "Speichert …" : "Speichern"}</span>
      </button>
    </div>
  </footer>
</div>

<style>
  .takeover {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .thead2 {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 20px;
    border-bottom: 1px solid var(--hairline);
    flex: none;
  }
  .crumb {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .back {
    appearance: none;
    cursor: pointer;
    background: transparent;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 5px 10px;
    color: var(--ink-default);
  }
  .back:hover {
    background: var(--surface-sunken);
  }
  .back .label {
    color: inherit;
  }
  .sep {
    color: var(--ink-muted);
  }
  .crumbnow {
    color: var(--ink-strong);
    font-size: 13px;
  }
  .idline {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .idval {
    font-size: 13px;
    color: var(--ink-strong);
  }
  .idtag {
    font-size: 9px;
    color: var(--ink-muted);
    padding: 2px 7px;
    border-radius: 99px;
    background: rgba(28, 26, 25, 0.06);
  }

  .modes {
    display: flex;
    gap: 2px;
    padding: 8px 16px 0;
    border-bottom: 1px solid var(--hairline);
    flex: none;
  }
  .mtab {
    appearance: none;
    cursor: pointer;
    position: relative;
    background: transparent;
    border: 0;
    padding: 10px 16px;
    color: var(--ink-muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    transition: color var(--dur) var(--ease);
  }
  .mtab:hover {
    color: var(--ink-default);
  }
  .mtab.on {
    color: var(--ink-strong);
    border-bottom-color: var(--ink-strong);
  }
  .mtab .label {
    color: inherit;
  }
  .mtab .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }

  .panel {
    flex: 1;
    overflow-y: auto;
    padding: 22px 20px;
    min-height: 0;
  }
  .pane {
    display: flex;
    flex-direction: column;
    gap: 22px;
    max-width: 760px;
  }
  .pane.narrow {
    max-width: 440px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 22px;
  }
  .fld {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .fl {
    color: var(--ink-muted);
    font-size: 10px;
  }
  .sub {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    color: var(--ink-muted);
    margin-left: 6px;
    font-size: 10px;
  }
  .in {
    padding: 9px 11px;
    font-size: 13px;
    font-family: var(--font-label);
    color: var(--ink-strong);
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }
  .in.mono {
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .in:focus {
    outline: none;
    border-color: var(--ink-strong);
  }
  .frozen {
    font-size: 13px;
    color: var(--ink-default);
    padding: 9px 0;
  }
  .hint {
    font-size: 11px;
    color: var(--ink-muted);
  }
  .err {
    font-size: 10px;
    color: var(--accent);
    text-transform: none;
    letter-spacing: 0;
  }

  .tfoot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 13px 20px;
    border-top: 1px solid var(--hairline);
    flex: none;
  }
  .status {
    font-size: 10.5px;
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
  }
  .status .bad {
    color: var(--accent);
  }
  .keys {
    display: flex;
    gap: 10px;
  }
  .ghost,
  .go {
    appearance: none;
    cursor: pointer;
    border-radius: var(--radius);
    padding: 9px 15px;
    border: 1px solid var(--hairline);
  }
  .ghost {
    background: transparent;
    color: var(--ink-default);
  }
  .ghost:hover {
    background: var(--surface-sunken);
  }
  .go {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
  }
  .go:hover:not(:disabled) {
    background: #2a2724;
  }
  .go:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .ghost .label,
  .go .label {
    color: inherit;
  }
</style>
