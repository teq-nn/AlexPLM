<script lang="ts">
  import type { GateVerdict, OffenerPunkt, Haerte } from "./types";
  import GateTaste from "./GateTaste.svelte";

  // The Freigabe-Gate (Issue #52, E19/E19.3): the dreistufige Block in *einem* kontextabhängigen
  // Knopf. Offene Punkte are not thrown on a heap — they are staffed nach Härte (härtestes zuerst)
  // and the ONE button changes its Beschriftung *und* Schärfe with the hardest point present:
  //   • alles sauber / nur Warnung → der ruhige „Taggen"-Knopf (proceeds freely);
  //   • weicher Block (Waise / Pflicht) → „Trotzdem freigeben" hinter der schwarzen Gate-Taste,
  //     mit einem protokollierten Satz (§22.1);
  //   • harter Block (offene blockierende Aufgabe) → Knopf AUS; daneben die Aufgabe mit ihren drei
  //     Auswegen (Erledigen / Verwerfen / Herabstufen) — kein Begründungs-Schlupfloch (E15).
  // Orange is the rationed laute Ausnahme — the gate is exactly such a moment, so the hard rows
  // and the cross-person warning carry it; nothing else does.
  let {
    verdict,
    busy = false,
    /** Raise the tag. `begruendung` is the protokollierter Satz for a weicher Block (else null). */
    onFreigeben,
    onCancel,
    /** Act on a hard-blocking Aufgabe by its id — the one Ausweg out of a harter Block. */
    onErledigen,
    onVerwerfen,
    onHerabstufen,
  }: {
    verdict: GateVerdict;
    busy?: boolean;
    onFreigeben: (begruendung: string | null) => void;
    onCancel: () => void;
    onErledigen: (taskId: string) => void;
    onVerwerfen: (taskId: string) => void;
    onHerabstufen: (taskId: string) => void;
  } = $props();

  // The protokollierter Satz for the weicher Block. The „Trotzdem freigeben" key stays disabled
  // until a non-empty sentence is typed (§22.1: a soft block is overcome by a *logged* reason).
  let begruendung = $state("");
  let satzFehlt = $derived(verdict.begruendung_noetig && begruendung.trim().length === 0);

  // One readable line per Härte, härtestes zuerst — matches the core's sort.
  const haerteRang: Record<Haerte, number> = { hart: 0, weich: 1, warnung: 2 };
  let punkte = $derived(
    [...verdict.punkte].sort((a, b) => haerteRang[a.haerte] - haerteRang[b.haerte]),
  );
  let hartePunkte = $derived(punkte.filter((p) => p.haerte === "hart"));
  let weichePunkte = $derived(punkte.filter((p) => p.haerte === "weich"));
  let warnPunkte = $derived(punkte.filter((p) => p.haerte === "warnung"));

  function haerteWort(h: Haerte): string {
    return h === "hart"
      ? "harter Block"
      : h === "weich"
        ? "weicher Block"
        : "Warnung";
  }
  function artWort(p: OffenerPunkt): string {
    switch (p.art) {
      case "aufgabe":
        return "offene Aufgabe";
      case "waise":
        return "Waise";
      case "fehlende-pflicht":
        return "fehlendes Pflicht-Artefakt";
      case "stale-kante":
        return "Stale-Kante";
    }
  }

  function press() {
    onFreigeben(verdict.begruendung_noetig ? begruendung.trim() : null);
  }
</script>

<div class="scrim" role="presentation" onclick={() => !busy && onCancel()}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="gate"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="freigabe-title"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="strip" class:hart={verdict.harter_block} aria-hidden="true"></div>

    <header class="head">
      <span class="label kicker">Freigabe-Gate</span>
      <h2 id="freigabe-title" class="title">
        {#if verdict.harter_block}
          Erst die Aufgabe, dann die Freigabe
        {:else if verdict.begruendung_noetig}
          Technisch vollständig?
        {:else if punkte.length > 0}
          Bereit zur Freigabe
        {:else}
          Bereit zur Freigabe
        {/if}
      </h2>
    </header>

    <div class="body">
      {#if punkte.length === 0}
        <p class="lede">
          Keine offenen Punkte. Dieser Stand kann als <strong>Freigabe</strong> getaggt werden.
        </p>
      {:else}
        <p class="lede">
          {#if verdict.harter_block}
            Ein offener blockierender Punkt hält die Freigabe. Der Ausweg ist
            <strong>ein Griff an die Aufgabe</strong> — nicht ein Satz.
          {:else if verdict.begruendung_noetig}
            Technisch unvollständig, aber bewusst überwindbar — mit einem
            <strong>protokollierten Satz</strong>.
          {:else}
            Nur Warnungen offen. Sie blockieren nicht — die Freigabe kann gesetzt werden.
          {/if}
        </p>

        <!-- The one härte-sortierte Liste, härtestes zuerst. -->
        <ol class="punkte">
          {#each hartePunkte as p (p.ref_id)}
            <li class="punkt hart">
              <span class="dot attention" aria-hidden="true"></span>
              <div class="punkt-text">
                <div class="punkt-kopf">
                  <span class="label haerte">{haerteWort(p.haerte)}</span>
                  <span class="punkt-art mono">{artWort(p)}</span>
                </div>
                <span class="punkt-label">{p.label}</span>
                <!-- The three Auswege — the only way past a harter Block (E15/E19). -->
                <div class="auswege">
                  <button class="tk" onclick={() => onErledigen(p.ref_id)} disabled={busy}
                    >erledigen</button
                  >
                  <button class="tk" onclick={() => onVerwerfen(p.ref_id)} disabled={busy}
                    >verwerfen</button
                  >
                  <button class="tk" onclick={() => onHerabstufen(p.ref_id)} disabled={busy}
                    >zum Hinweis</button
                  >
                </div>
              </div>
            </li>
          {/each}

          {#each weichePunkte as p (p.ref_id)}
            <li class="punkt weich">
              <span class="dot working" aria-hidden="true"></span>
              <div class="punkt-text">
                <div class="punkt-kopf">
                  <span class="label haerte">{haerteWort(p.haerte)}</span>
                  <span class="punkt-art mono">{artWort(p)}</span>
                </div>
                <span class="punkt-label mono">{p.label}</span>
              </div>
            </li>
          {/each}

          {#each warnPunkte as p (p.ref_id)}
            <li class="punkt warnung">
              <span class="dot off" aria-hidden="true"></span>
              <div class="punkt-text">
                <div class="punkt-kopf">
                  <span class="label haerte">{haerteWort(p.haerte)}</span>
                  <span class="punkt-art mono">{artWort(p)}</span>
                </div>
                <span class="punkt-label mono">{p.label}</span>
              </div>
            </li>
          {/each}
        </ol>
      {/if}

      {#if verdict.fremd_warnung}
        <!-- Personenübergreifend (E19.1/E33): you co-tag a colleague's frischen Stand. -->
        <div class="fremd" role="note">
          <span class="dot attention" aria-hidden="true"></span>
          <span class="fremd-text">{verdict.fremd_warnung.satz}</span>
        </div>
      {/if}

      {#if verdict.begruendung_noetig}
        <!-- The protokollierter Satz — the soft block's deliberate, recorded override (§22.1). -->
        <label class="field">
          <span class="label field-label">Begründung — wird protokolliert</span>
          <input
            class="input mono"
            bind:value={begruendung}
            placeholder="z. B. Prototypenstand, Testprotokoll folgt"
            spellcheck="false"
            autocomplete="off"
            disabled={busy}
          />
        </label>
      {/if}
    </div>

    <footer class="foot">
      <button class="key ghost" onclick={onCancel} disabled={busy}>
        <span class="label">Abbrechen</span>
      </button>

      {#if verdict.harter_block}
        <!-- Harter Block: the button is AUS. The Ausweg lives in the row above, not here. -->
        <span class="gesperrt label" aria-disabled="true">
          gesperrt durch Aufgabe
        </span>
      {:else if verdict.begruendung_noetig}
        <!-- Weicher Block: „Trotzdem freigeben" behind the schwarze Gate-Taste + the logged Satz. -->
        <GateTaste
          consent="Ich gebe trotz offenem Punkt bewusst frei"
          label="Trotzdem freigeben"
          busyLabel="gebe frei …"
          {busy}
          disabled={satzFehlt}
          onPress={press}
        />
      {:else}
        <!-- Alles sauber (or warning-only): the calm „Taggen"-Knopf. No black key, no orange. -->
        <button class="key solid label" onclick={press} disabled={busy}>
          {busy ? "…" : "Taggen"}
        </button>
      {/if}
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
    width: min(560px, 100%);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
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

  /* A single dark band at the very top — this dialog carries weight. A harter Block lights it
     in the rationed orange: the loud exception. */
  .strip {
    height: 5px;
    flex: none;
    background: linear-gradient(90deg, #1c1a19, #000 60%, #1c1a19);
  }
  .strip.hart {
    background: linear-gradient(90deg, #1c1a19, var(--accent) 50%, #1c1a19);
  }

  .head {
    padding: 18px 22px 4px;
    flex: none;
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
    overflow-y: auto;
    min-height: 0;
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

  /* The one härte-sortierte Liste. Each row is an LED dot + the point, weighted by Härte. */
  .punkte {
    list-style: none;
    margin: 0 0 16px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .punkt {
    display: grid;
    grid-template-columns: 9px 1fr;
    gap: 11px;
    align-items: start;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    border: 1px solid transparent;
  }
  /* The harter Block is the loud exception: a hairline of rationed orange, nothing more. */
  .punkt.hart {
    background: var(--surface-base);
    border-color: rgba(240, 66, 28, 0.45);
    box-shadow: 0 0 0 1px rgba(240, 66, 28, 0.12);
  }

  .punkt .dot {
    margin-top: 5px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }
  .dot.attention {
    background: var(--led-attention);
    box-shadow: 0 0 6px rgba(240, 66, 28, 0.5);
  }
  .dot.working {
    background: var(--led-working);
  }
  .dot.off {
    background: var(--led-off);
  }

  .punkt-text {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .punkt-kopf {
    display: flex;
    align-items: baseline;
    gap: 9px;
  }
  .haerte {
    color: var(--ink-strong);
    font-size: 10px;
  }
  .punkt.warnung .haerte {
    color: var(--ink-muted);
  }
  .punkt-art {
    color: var(--ink-muted);
    font-size: 10.5px;
  }
  .punkt-label {
    color: var(--ink-default);
    font-size: 13px;
    line-height: 1.4;
    word-break: break-word;
  }

  /* The three Auswege out of a harter Block — flat „task keys", same as the Aufgaben list. */
  .auswege {
    display: flex;
    gap: 6px;
    margin-top: 3px;
  }
  .tk {
    appearance: none;
    cursor: pointer;
    background: var(--key-light);
    color: var(--ink-default);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    padding: 4px 9px;
    font-family: var(--font-label);
    font-size: 11px;
    letter-spacing: 0.02em;
    transition:
      background var(--dur) var(--ease),
      color var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .tk:hover:not(:disabled) {
    background: var(--surface-raised);
    color: var(--ink-strong);
    border-color: var(--ink-muted);
  }
  .tk:disabled {
    cursor: default;
    opacity: 0.5;
  }

  /* The personenübergreifende Warnung — a recorded note, not a block. */
  .fremd {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 10px 12px;
    margin-bottom: 16px;
    border-radius: var(--radius-sm);
    background: var(--surface-base);
    border: 1px solid var(--hairline);
  }
  .fremd .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }
  .fremd-text {
    color: var(--ink-default);
    font-size: 13px;
    line-height: 1.4;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field-label {
    color: var(--ink-muted);
  }
  .input {
    width: 100%;
    background: var(--screen-bg);
    color: var(--screen-fg);
    border: 1px solid #000;
    border-radius: var(--radius-sm);
    padding: 9px 11px;
    font-size: 13px;
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.8);
  }
  .input::placeholder {
    color: #6b6660;
  }
  .input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 22px 20px;
    border-top: 1px solid var(--hairline);
    flex: none;
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
  .key.ghost {
    background: transparent;
    box-shadow: none;
    color: var(--ink-default);
  }
  .key.ghost:hover:not(:disabled) {
    background: var(--surface-sunken);
  }
  /* The calm „Taggen" key — the clean path. A dark seated key, never orange. */
  .key.solid {
    background: var(--key-dark);
    color: var(--screen-fg);
    border-color: #000;
  }
  .key.solid:hover:not(:disabled) {
    background: #2a2724;
  }
  .key.solid:active:not(:disabled) {
    transform: translateY(1px);
  }
  .key:disabled {
    cursor: default;
    opacity: 0.5;
    box-shadow: none;
  }

  /* The hard-block dead state where the button would be — reads dark-and-locked, not faded. */
  .gesperrt {
    color: var(--ink-muted);
    padding: 9px 15px;
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    background: var(--surface-sunken);
    white-space: nowrap;
    cursor: not-allowed;
  }
</style>
