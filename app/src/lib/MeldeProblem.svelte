<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { IssueRef, RepoLabel } from "./types";

  // Der Name des Laufzeit-Etiketts (Spiegel von `feedback::RUNTIME_LABEL_NAME`): das Backend hängt es
  // IMMER automatisch an, daher wird es nicht zum Auswählen angeboten, sondern als fester Chip gezeigt.
  const RUNTIME_LABEL_NAME = "aus-der-laufzeit";

  // „Problem melden" (Issue #85) — ein Issue aus der laufenden Werkbank direkt ins Repository des
  // offenen Produkts legen. Bewusst KEIN orange-gerahmter Moment (das Akzent-Orange ist für die
  // laute Ausnahme reserviert): das hier ist eine ruhige, gewollte Nutzer-Handlung. Titel +
  // Beschreibung, optional das Diagnose-Log angehängt; das Backend trägt das Laufzeit-Etikett an.
  // Adresse + Zugangsdaten zieht das Backend aus der Produkt-Remote + dem Konto — der Nutzer tippt
  // hier nie Server oder Token.
  let {
    open = false,
    productPath,
    onClose,
  }: {
    open?: boolean;
    productPath: string | null;
    onClose: () => void;
  } = $props();

  // Ein typisierter Backend-Fehler (wie KontoPanel/Zeremonie): `auth` = Token abgelehnt, `keystore`
  // = Schlüsselbund nicht erreichbar, `error` = alles übrige. Die menschliche Meldung wird gezeigt —
  // nie ein „[object Object]" und nie roher git-Text.
  type AppError = { code: string; message: string };
  function asAppError(e: unknown): AppError {
    if (e && typeof e === "object" && "code" in e && "message" in e) {
      return e as AppError;
    }
    return { code: "error", message: String(e) };
  }

  let titel = $state("");
  let beschreibung = $state("");
  let logAnhaengen = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  // Die Bestätigung des angelegten Issues (Nummer + Link). Solange null, zeigt das Panel das Formular.
  let result = $state<IssueRef | null>(null);

  // Die Etiketten des Produkt-Repos (für den Picker) + die gewählten Ids. Best-effort geladen: scheitert
  // das Lesen (kein Konto, Netz), bleibt der Picker leer und das Backend hängt nur das Laufzeit-Etikett an.
  let labels = $state<RepoLabel[]>([]);
  let selectedIds = $state<number[]>([]);

  // Das Laufzeit-Etikett wird automatisch angehängt — aus der Auswahlliste herausgehalten und separat
  // als fester Chip gezeigt. Der Rest ist frei wählbar.
  let pickable = $derived(
    labels.filter((l) => l.name.toLowerCase() !== RUNTIME_LABEL_NAME),
  );

  // Bei jedem Öffnen frisch: ein voriger Bericht (oder Fehler) darf den nächsten nicht vorbelegen.
  $effect(() => {
    if (open) {
      titel = "";
      beschreibung = "";
      logAnhaengen = false;
      error = null;
      result = null;
      busy = false;
      labels = [];
      selectedIds = [];
      void loadLabels();
    }
  });

  // Die Repo-Etiketten laden. Best-effort: ein Fehler lässt nur den Picker leer — das Melden selbst
  // bleibt möglich (das Backend hängt das Laufzeit-Etikett ohnehin an). Kein lautes Error-Banner hier.
  async function loadLabels() {
    if (!productPath) return;
    try {
      labels = await invoke<RepoLabel[]>("produkt_etiketten", { path: productPath });
    } catch {
      labels = [];
    }
  }

  function toggle(id: number) {
    selectedIds = selectedIds.includes(id)
      ? selectedIds.filter((x) => x !== id)
      : [...selectedIds, id];
  }

  // Die Render-Farbe eines Etiketts: `#`-präfixiertes Hex, oder ein neutraler Ton, wenn keine da ist.
  function swatch(l: RepoLabel): string {
    const c = (l.color ?? "").trim();
    return c ? `#${c}` : "var(--ink-muted)";
  }

  let canSend = $derived(titel.trim() !== "" && productPath !== null && !busy);

  async function send() {
    if (!productPath) return;
    error = null;
    busy = true;
    try {
      result = await invoke<IssueRef>("melde_problem", {
        path: productPath,
        titel,
        beschreibung,
        logAnhaengen,
        labels: selectedIds,
      });
    } catch (e) {
      error = asAppError(e).message;
    } finally {
      busy = false;
    }
  }

  async function openInBrowser() {
    if (result) await openUrl(result.html_url);
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onclick={() => !busy && onClose()}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
    <section
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="melde-title"
      onclick={(e) => e.stopPropagation()}
    >
      <header class="head">
        <span class="label kicker">Rückmeldung</span>
        <h2 id="melde-title" class="title">Problem melden</h2>
        <p class="sub label">
          Geht als Issue direkt ins Repository dieses Produkts — mit dem Etikett „aus-der-laufzeit".
        </p>
      </header>

      <div class="body">
        {#if result}
          <!-- Bestätigung: das Issue ist angelegt. Dunkles Instrument-Readout mit Nummer + Link. -->
          <div class="readout mono" role="status">
            <span class="dot ok" aria-hidden="true"></span>
            <span class="rv">
              Gemeldet als <strong>#{result.number}</strong>
            </span>
          </div>
          <p class="lede">
            Danke — dein Bericht ist im Produkt-Repository angelegt. Du kannst ihn im Browser öffnen,
            um ihn zu verfolgen oder zu ergänzen.
          </p>
        {:else}
          <p class="lede">
            Beschreibe kurz, was nicht stimmt oder fehlt. Server und Zugangsdaten kommen aus deinem
            Konto — du musst hier nichts weiter angeben.
          </p>

          <div class="form">
            <label class="field">
              <span class="label fk">Titel</span>
              <input
                class="in"
                bind:value={titel}
                placeholder="Kurz benennen, z. B. „Sichern bleibt hängen"
                autocomplete="off"
                spellcheck="false"
                disabled={busy}
              />
            </label>
            <label class="field">
              <span class="label fk">Beschreibung</span>
              <textarea
                class="in area"
                bind:value={beschreibung}
                rows="5"
                placeholder="Was ist passiert? Was hattest du erwartet? Schritte zum Nachstellen?"
                spellcheck="false"
                disabled={busy}
              ></textarea>
            </label>

            <!-- Etiketten-Picker: das Laufzeit-Etikett ist fest (wird immer angehängt), die übrigen
                 Repo-Etiketten sind frei wählbar. Fehlt die Liste (Lesefehler), wird nur der feste
                 Chip gezeigt — Melden bleibt möglich. -->
            <div class="field">
              <span class="label fk">Etiketten</span>
              <div class="chips" role="group" aria-label="Etiketten">
                <span class="chip fixed" title="Wird automatisch angehängt">
                  <span class="swatch" style="background:var(--accent)" aria-hidden="true"></span>
                  <span class="chip-name">{RUNTIME_LABEL_NAME}</span>
                  <span class="chip-auto label">auto</span>
                </span>
                {#each pickable as label (label.id)}
                  <button
                    type="button"
                    class="chip pick"
                    class:on={selectedIds.includes(label.id)}
                    aria-pressed={selectedIds.includes(label.id)}
                    onclick={() => toggle(label.id)}
                    disabled={busy}
                  >
                    <span class="swatch" style="background:{swatch(label)}" aria-hidden="true"></span>
                    <span class="chip-name">{label.name}</span>
                  </button>
                {/each}
              </div>
            </div>

            <!-- Opt-in: das Diagnose-Log (git/sync-Ring) anhängen. Standard aus — es kann Pfade
                 enthalten; der Nutzer entscheidet bewusst, es mitzuschicken. -->
            <label class="check">
              <input type="checkbox" bind:checked={logAnhaengen} disabled={busy} />
              <span class="check-body">
                <span class="check-title label">Diagnose-Log anhängen</span>
                <span class="check-hint label">
                  Hängt das jüngste Sync- & Sicherungs-Protokoll an — hilft beim Nachstellen.
                </span>
              </span>
            </label>
          </div>
        {/if}

        {#if error}
          <div class="err" role="alert">
            <span class="dot warn" aria-hidden="true"></span>
            <span class="err-text label">{error}</span>
          </div>
        {/if}
      </div>

      <footer class="foot">
        {#if result}
          <button class="key ghost" onclick={onClose}>
            <span class="label">Schließen</span>
          </button>
          <button class="key go" onclick={openInBrowser}>
            <span class="label">Im Browser öffnen</span>
          </button>
        {:else}
          <button class="key ghost" onclick={onClose} disabled={busy}>
            <span class="label">Abbrechen</span>
          </button>
          <button class="key go" onclick={send} disabled={!canSend}>
            <span class="label">{busy ? "sende …" : "Melden"}</span>
          </button>
        {/if}
      </footer>
    </section>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(8, 7, 6, 0.62);
    backdrop-filter: blur(2px);
    animation: scrim-in 160ms var(--ease);
  }
  @keyframes scrim-in {
    from {
      opacity: 0;
    }
  }

  .panel {
    width: min(520px, 100%);
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 24px 60px -16px rgba(8, 7, 6, 0.6),
      0 2px 0 rgba(255, 255, 255, 0.5) inset;
    overflow: hidden;
    animation: panel-in 200ms var(--ease) backwards;
  }
  @keyframes panel-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.99);
    }
  }

  .head {
    padding: 20px 22px 6px;
  }
  .kicker {
    color: var(--ink-muted);
    display: block;
    margin-bottom: 6px;
  }
  .title {
    margin: 0;
    font-family: var(--font-label);
    font-weight: 700;
    font-size: 22px;
    letter-spacing: -0.01em;
    color: var(--ink-strong);
  }
  .sub {
    margin: 8px 0 0;
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12px;
  }

  .body {
    padding: 14px 22px 18px;
  }
  .lede {
    margin: 14px 0 14px;
    color: var(--ink-default);
    font-size: 14px;
    line-height: 1.5;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .fk {
    color: var(--ink-muted);
    font-size: 10px;
  }

  /* Etiketten-Chips: kleine, ruhige Marken. Ein gewählter Chip bekommt den dunklen Rahmen + Grund
     (dieselbe „gedrückt"-Sprache wie die Tasten); der feste Laufzeit-Chip trägt den Akzent-Punkt. */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 9px;
    font-size: 11.5px;
    color: var(--ink-default);
    background: var(--surface-base);
    border: 1px solid var(--hairline);
    border-radius: 99px;
  }
  .swatch {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(28, 26, 25, 0.12) inset;
  }
  .chip-name {
    overflow-wrap: anywhere;
  }
  .chip.pick {
    appearance: none;
    cursor: pointer;
    transition:
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .chip.pick:hover:not(:disabled) {
    border-color: var(--ink-strong);
  }
  .chip.pick.on {
    background: var(--key-dark);
    border-color: var(--key-dark);
    color: var(--key-light);
  }
  .chip.pick:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .chip.pick:focus-visible {
    outline: 2px solid var(--ink-strong);
    outline-offset: 2px;
  }
  /* The fixed runtime label: always-on, not toggleable — a quiet marker, not a button. */
  .chip.fixed {
    color: var(--ink-muted);
    border-style: dashed;
  }
  .chip-auto {
    color: var(--ink-muted);
    font-size: 8.5px;
    opacity: 0.8;
  }
  /* Recessed input wells — same light-on-hairline treatment as the Konto fields. */
  .in {
    appearance: none;
    width: 100%;
    padding: 9px 11px;
    font-size: 13px;
    font-family: inherit;
    color: var(--ink-strong);
    background: #faf9f5;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    box-shadow: inset 0 1px 2px rgba(28, 26, 25, 0.07);
    transition: border-color var(--dur) var(--ease);
  }
  .in:focus {
    outline: none;
    border-color: var(--ink-strong);
  }
  .in::placeholder {
    color: var(--ink-muted);
    opacity: 0.6;
  }
  .in:disabled {
    opacity: 0.6;
  }
  .area {
    resize: vertical;
    min-height: 92px;
    line-height: 1.5;
  }

  /* Opt-in checkbox: a quiet row, not a loud toggle. The hint explains what is shared. */
  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    background: var(--surface-base);
    cursor: pointer;
  }
  .check input {
    margin: 1px 0 0;
    accent-color: var(--ink-strong);
    cursor: pointer;
  }
  .check-body {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .check-title {
    color: var(--ink-default);
    font-size: 11px;
  }
  .check-hint {
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 11px;
    line-height: 1.4;
  }

  /* Dark instrument readout for the "Gemeldet als #N" confirmation — mirrors KontoPanel's. */
  .readout {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: linear-gradient(180deg, #131110, #0b0a09);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.9);
    font-size: 12px;
  }
  .readout .rv {
    color: var(--screen-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .readout .rv strong {
    color: #fff;
  }
  .dot.ok {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
  }

  .err {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-top: 14px;
    padding: 11px 13px;
    border: 1px solid var(--hairline);
    border-left: 3px solid var(--led-attention);
    border-radius: var(--radius);
    background: var(--surface-base);
  }
  .err .dot.warn {
    margin-top: 3px;
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--led-attention);
    box-shadow: 0 0 6px rgba(240, 66, 28, 0.4);
  }
  .err-text {
    color: var(--ink-default);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12.5px;
    line-height: 1.45;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 22px 20px;
    border-top: 1px solid var(--hairline);
  }
  .key {
    appearance: none;
    cursor: pointer;
    background: var(--key-light);
    color: var(--ink-strong);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 9px 15px;
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.12);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease),
      opacity var(--dur) var(--ease);
  }
  .key .label {
    color: inherit;
  }
  .key:hover:not(:disabled) {
    background: #f5f3ee;
  }
  .key:active:not(:disabled) {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.12);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.5;
    box-shadow: none;
  }
  .key.ghost {
    background: transparent;
    box-shadow: none;
    color: var(--ink-default);
  }
  .key.ghost:hover:not(:disabled) {
    background: var(--surface-sunken);
  }
  /* The forward key carries the dark, weighted cap — sending the report is the deliberate act. */
  .key.go {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
  }
  .key.go:hover:not(:disabled) {
    background: #2a2724;
  }
</style>
