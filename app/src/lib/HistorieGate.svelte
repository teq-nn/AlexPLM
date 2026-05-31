<script lang="ts">
  import type { GateReport } from "./types";
  import GateTaste from "./GateTaste.svelte";

  // The "Historie anfassen" gate (E38/E27). The destructive history rewrite sits behind a
  // bewusste confirmation: the user reads the stakes, then deliberately arms the shared black,
  // separated „Gate-Taste" (GateTaste) before it can be pressed — never accidentally clickable.
  let {
    report,
    busy = false,
    onConfirm,
    onCancel,
  }: {
    report: GateReport;
    busy?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
</script>

<div
  class="scrim"
  role="presentation"
  onclick={() => !busy && onCancel()}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="gate"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="gate-title"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="strip" aria-hidden="true"></div>

    <header class="head">
      <span class="label kicker">Bewusste Schwere</span>
      <h2 id="gate-title" class="title">Historie anfassen</h2>
    </header>

    <div class="body">
      <p class="lede">
        Dieser Ordner trägt bereits schwere Binärdateien in seiner Historie. Sie
        dauerhaft auszulagern bedeutet, die <strong>gesamte Historie umzuschreiben</strong>
        — jeder bisherige Stand bekommt eine neue Identität.
      </p>

      <ul class="stakes label">
        <li>
          <span class="dot warn" aria-hidden="true"></span>
          Alle bisherigen Stände werden neu geschrieben — alte Verweise gelten nicht mehr.
        </li>
        <li>
          <span class="dot ok" aria-hidden="true"></span>
          Erlaubt nur, weil dieser Ordner <strong>nicht geteilt</strong> ist: kein fremder
          Klon, der vergiftet werden könnte.
        </li>
        <li>
          <span class="dot ok" aria-hidden="true"></span>
          Danach liegen die schweren Inhalte schlank als Verweise; der Ordner bleibt nutzbar.
        </li>
      </ul>

      <div class="facts mono" role="group" aria-label="Befund">
        <span class="fact"
          ><span class="fk">Historie</span><span class="fv"
            >{report.has_history ? "vorhanden" : "keine"}</span
          ></span
        >
        <span class="fact"
          ><span class="fk">Geteilte Klone</span><span class="fv"
            >{report.shared_clones_exist ? "ja" : "keine"}</span
          ></span
        >
        <span class="fact"
          ><span class="fk">Schwere Binaries</span><span class="fv"
            >{report.giant_binaries_in_history ? "in Historie" : "keine"}</span
          ></span
        >
      </div>
    </div>

    <footer class="foot">
      <button class="key ghost" onclick={onCancel} disabled={busy}>
        <span class="label">Abbrechen</span>
      </button>

      <!-- The shared black „Gate-Taste" (Issue #46): arm, then the heavy key becomes pressable. -->
      <GateTaste
        consent="Ich schreibe die Historie bewusst um"
        label="Historie umschreiben"
        busyLabel="schreibe um …"
        {busy}
        onPress={onConfirm}
      />
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

  .gate {
    width: min(540px, 100%);
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 24px 60px -16px rgba(8, 7, 6, 0.6),
      0 2px 0 rgba(255, 255, 255, 0.5) inset;
    overflow: hidden;
    animation: gate-in 200ms var(--ease) backwards;
  }
  @keyframes gate-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.99);
    }
  }

  /* A single dark band at the very top: this dialog carries weight. */
  .strip {
    height: 5px;
    background: linear-gradient(90deg, #1c1a19, #000 60%, #1c1a19);
  }

  .head {
    padding: 18px 22px 4px;
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

  .body {
    padding: 10px 22px 18px;
  }
  .lede {
    margin: 0 0 16px;
    color: var(--ink-default);
    font-size: 14px;
    line-height: 1.5;
  }
  .lede strong {
    color: var(--ink-strong);
  }

  .stakes {
    list-style: none;
    margin: 0 0 18px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 9px;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 13px;
    line-height: 1.45;
  }
  .stakes li {
    display: grid;
    grid-template-columns: 9px 1fr;
    gap: 10px;
    align-items: start;
    color: var(--ink-default);
  }
  .stakes strong {
    color: var(--ink-strong);
    font-weight: 600;
  }
  .stakes .dot {
    margin-top: 5px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .dot.warn {
    background: var(--led-attention);
    box-shadow: 0 0 6px rgba(240, 66, 28, 0.4);
  }
  .dot.ok {
    background: var(--led-free);
  }

  /* Recessed instrument readout of the three gate facts. */
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 4px;
    border-radius: var(--radius-sm);
    background: linear-gradient(180deg, #131110, #0b0a09);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.9);
    font-size: 11px;
  }
  .fact {
    flex: 1 1 140px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 7px 10px;
  }
  .fk {
    color: #6b6660;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 9.5px;
  }
  .fv {
    color: var(--screen-fg);
    font-weight: 500;
  }

  .foot {
    display: flex;
    align-items: stretch;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 22px 20px;
    border-top: 1px solid var(--hairline);
  }

  /* Neutral cancel key, matching the shell's physical keys. */
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
  .key.ghost {
    background: transparent;
    box-shadow: none;
    color: var(--ink-default);
  }
  .key.ghost:hover:not(:disabled) {
    background: var(--surface-sunken);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.5;
    box-shadow: none;
  }
</style>
