<script lang="ts">
  import { cmd } from "$lib/commands";
  import type { KontoView } from "./types";

  // Das globale Konto-Panel (ADR 0004, Issue #90). Genau EINE app-weite Server-Identität für das
  // selbst-gehostete Forgejo/Gitea: Server-Adresse + Username + Passwort/Token eintippen, „Prüfen &
  // Speichern" prüft Verbindung + Token-Gültigkeit gegen `GET /api/v1/user` und speichert. Erreichbar
  // über das Zahnrad im Header — auch ohne offenes Produkt. Lokales Arbeiten braucht kein Konto; es
  // wird erst im Teilen-Moment nötig. Das Passwort-Feld ist write-only (wird nie zurückgezeigt).
  let { onClose }: { onClose: () => void } = $props();

  // Ein typisierter Backend-Fehler (wie die Zeremonie, Issue #22): `auth` = Token fehlt/falsch,
  // `keystore` = OS-Schlüsselbund nicht erreichbar. Beide bringen den Nutzer klar zurück ins
  // Eingabefeld — nie ein „[object Object]".
  type AppError = { code: string; message: string };
  function asAppError(e: unknown): AppError {
    if (e && typeof e === "object" && "code" in e && "message" in e) {
      return e as AppError;
    }
    return { code: "error", message: String(e) };
  }

  // Eingabefelder. Die Zugangsdaten bleiben in dieser Komponente und gehen direkt an Rust; die
  // zurückgegebene Sicht trägt NIE den Token (nur Base-URL + Account).
  let server = $state("");
  let username = $state("");
  let token = $state("");

  // Die bestätigte Konto-Sicht (Base-URL + Account). Null, solange kein Konto eingerichtet ist.
  let konto = $state<KontoView | null>(null);
  let loaded = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  // Beim Öffnen den aktuellen Konto-Stand laden (best-effort): füllt Server + Username vor (nie das
  // Passwort — das bleibt write-only und muss neu eingetippt werden) und zeigt die Anmelde-Zeile.
  async function load() {
    try {
      konto = await cmd.readKonto();
      if (konto) {
        server = konto.base_url;
        username = konto.account;
      }
    } catch (e) {
      // Der Konto-Stand ist Hilfsinfo; ein Lese-Fehler darf das Panel nicht blockieren.
      error = asAppError(e).message;
    } finally {
      loaded = true;
    }
  }
  load();

  async function save() {
    error = null;
    busy = true;
    try {
      const view = await cmd.saveKonto(server, username, token);
      // Das Geheimnis nicht behalten, sobald es an den Backend-Keystore übergeben ist.
      token = "";
      konto = view;
      server = view.base_url;
      username = view.account;
    } catch (e) {
      // Das Backend liefert einen typisierten { code, message }: die menschliche Meldung zeigen.
      // Bei auth/keystore bleibt der Nutzer im Eingabefeld (das Panel zeigt ohnehin die Eingabe) —
      // der Fehler weist klar auf Token bzw. Schlüsselbund hin.
      error = asAppError(e).message;
    } finally {
      busy = false;
    }
  }

  // „Konto entfernen" (ADR 0004, Issue #91): löscht den Keystore-Eintrag des Konto-Hosts UND die
  // persistierte Base-URL. Rührt die `.git/config`-Remotes vorhandener Produkte NIE an — lokales
  // Arbeiten läuft weiter, nur das Teilen pausiert, bis wieder ein Konto gesetzt ist. Danach kehrt
  // das Panel in den Einrichtungs-Zustand zurück (kein Konto, leere Felder). Idempotent.
  async function clear() {
    error = null;
    busy = true;
    try {
      await cmd.clearKonto();
      konto = null;
      server = "";
      username = "";
      token = "";
    } catch (e) {
      error = asAppError(e).message;
    } finally {
      busy = false;
    }
  }

  // „Prüfen & Speichern" ist inaktiv, solange Server, Username und Passwort/Token nicht alle drei
  // gesetzt sind. Der Token wird auch beim Re-Speichern eines bestehenden Kontos neu verlangt
  // (write-only: nie zurückgezeigt, also auch nie vorbefüllt). Beim erneuten Speichern eines
  // bestehenden Kontos („ändern") überschreibt derselbe Prüf-Pfad das Konto.
  let canSave = $derived(
    server.trim() !== "" && username.trim() !== "" && token.trim() !== "",
  );
</script>

<div class="scrim" role="presentation" onclick={() => !busy && onClose()}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="konto-title"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="head">
      <span class="label kicker">Einstellungen</span>
      <h2 id="konto-title" class="title">Konto</h2>
      <p class="sub label">
        Eine app-weite Server-Identität: einmal anmelden, für alle Produkte genutzt.
      </p>
    </header>

    <div class="body">
      {#if konto}
        <!-- Anmelde-Zeile: „angemeldet als <account> an <server>". Dunkles Instrument-Readout. -->
        <div class="readout mono" role="status">
          <span class="dot ok" aria-hidden="true"></span>
          <span class="rv">
            angemeldet als <strong>{konto.account}</strong> an {konto.base_url}
          </span>
        </div>
      {/if}

      <p class="lede">
        Gib die Adresse deines selbst-gehosteten <strong>Forgejo / Gitea</strong>-Servers und deine
        Zugangsdaten an. Es wird nur geprüft, ob die Verbindung steht und der Token gültig ist — das
        Passwort wird nie angezeigt.
      </p>

      <div class="form">
        <label class="field">
          <span class="label fk">Server-Adresse</span>
          <input
            class="mono in"
            bind:value={server}
            placeholder="https://forge.example.de"
            autocomplete="off"
            spellcheck="false"
            disabled={busy}
          />
        </label>
        <div class="row">
          <label class="field">
            <span class="label fk">Username</span>
            <input
              class="mono in"
              bind:value={username}
              placeholder="anna"
              autocomplete="off"
              spellcheck="false"
              disabled={busy}
            />
          </label>
          <label class="field">
            <span class="label fk">Passwort / Token</span>
            <input
              class="mono in"
              type="password"
              bind:value={token}
              autocomplete="off"
              placeholder={konto ? "neu eingeben" : ""}
              disabled={busy}
            />
          </label>
        </div>
      </div>

      {#if error}
        <div class="err" role="alert">
          <span class="dot warn" aria-hidden="true"></span>
          <span class="err-text label">{error}</span>
        </div>
      {/if}
    </div>

    <footer class="foot">
      <div class="foot-left">
        <button class="key ghost" onclick={onClose} disabled={busy}>
          <span class="label">Schließen</span>
        </button>
        {#if konto}
          <!-- Nur sichtbar, wenn ein Konto existiert: löscht Keystore-Eintrag + Base-URL, lässt
               Produkt-Remotes unangetastet (ADR 0004). Danach kehrt das Panel in die Einrichtung. -->
          <button class="key danger" onclick={clear} disabled={busy || !loaded}>
            <span class="label">Konto entfernen</span>
          </button>
        {/if}
      </div>
      <button class="key go" onclick={save} disabled={!canSave || busy || !loaded}>
        <span class="label"
          >{busy ? "prüfe …" : konto ? "Prüfen & Ändern" : "Prüfen & Speichern"}</span
        >
      </button>
    </footer>
  </section>
</div>

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
  .lede strong {
    color: var(--ink-strong);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    flex: 1;
    min-width: 0;
  }
  .fk {
    color: var(--ink-muted);
    font-size: 10px;
  }
  /* Recessed input wells, echoing the dark instrument readouts but light enough to type into. */
  .in {
    appearance: none;
    width: 100%;
    padding: 9px 11px;
    font-size: 13px;
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

  /* Dark instrument readout for the "angemeldet als …" line. */
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
  /* The forward key carries the dark, weighted cap — saving the Konto is a deliberate act. */
  .key.go {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
    /* Label swaps Prüfen & Speichern ⇄ Prüfen & Ändern ⇄ prüfe … — pin to the widest. */
    min-width: 168px;
    text-align: center;
  }
  .key.go:hover:not(:disabled) {
    background: #2a2724;
  }

  /* The footer's left cluster: Schließen + the (conditional) destructive „Konto entfernen". */
  .foot-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* „Konto entfernen" is destructive but reversible (set a Konto again) — a quiet ghost with the
     attention hue, never the loud weighted forward cap. */
  .key.danger {
    background: transparent;
    box-shadow: none;
    color: var(--led-attention);
    border-color: var(--hairline);
  }
  .key.danger:hover:not(:disabled) {
    background: var(--surface-sunken);
    border-color: var(--led-attention);
  }
</style>
