<script lang="ts">
  import type { Abgleichfrage } from "./types";

  // The „Abgleich beim Öffnen" (Issue #129, E49a) — the SINGLE moment the open raises its voice.
  // On open the tool silently reconciles the real observed state of the three truth-places against
  // its last-seen memory and catches up every drift it can. The ONE drift it cannot silently decide
  // — a contested Sperre: an Artefakt the tool last knew was YOURS is now held by a Kollege — is
  // named here, in the tool's OWN domain language: „Bens Sperre liegt jetzt auf deinem Gehaeuse —
  // wessen Arbeit gilt?". There are NO git conflict markers anywhere (the Rust core guarantees it).
  //
  // The panel names the three truth-places HONESTLY (E49) — the tool never pretends there is one
  // store. Each place is what it is, and each drifts on its own:
  //   · Inhalt  — die Dateien auf der Platte (was du tatsächlich vor dir hast)
  //   · Verlauf — die Git-Historie (wie der Inhalt hierher kam, dauerhaft & geteilt)
  //   · Koordination — die Server-Sperren (flüchtig: wer ein unteilbares Artefakt gerade hält)
  // The contested drift here lives in „Koordination".

  let {
    frage,
    onClose,
  }: {
    frage: Abgleichfrage;
    onClose: () => void;
  } = $props();

  // The honest one-line description of the truth-place the contradiction lives in (E49). Named for
  // what it IS — never a git ref, never „lock".
  const ORTE: Record<Abgleichfrage["ort"], string> = {
    inhalt: "Inhalt — die Dateien auf der Platte",
    verlauf: "Verlauf — die Git-Historie",
    koordination: "Koordination — die Server-Sperren (flüchtig)",
  };
</script>

<div class="scrim" role="presentation">
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="abgleich"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="abgleich-frage"
  >
    <div class="strip" aria-hidden="true"></div>

    <header class="head">
      <span class="kicker label">
        <span class="beacon" aria-hidden="true"></span>
        Beim Öffnen abgeglichen · ein Widerspruch
      </span>
      <h2 id="abgleich-frage" class="frage">{frage.frage}</h2>
    </header>

    <div class="body">
      <!-- The contested artifacts, named as artifacts on a recessed instrument readout — never as
           git refs, never with conflict markers. -->
      <div class="artefakte" role="group" aria-label="Betroffene Artefakte">
        <span class="artefakte-label label">Betroffen</span>
        <ul class="artefakte-list mono">
          {#each frage.artefakte as a (a)}
            <li class="artefakt">
              <span class="led" aria-hidden="true"></span>
              <span class="artefakt-path">{a}</span>
            </li>
          {/each}
        </ul>
      </div>

      <!-- The three truth-places, named honestly (E49): the contested one is marked. -->
      <div class="orte" role="group" aria-label="Die drei Orte der Wahrheit">
        <span class="orte-label label">Worauf der Widerspruch sitzt</span>
        <p class="ort-zeile">{ORTE[frage.ort]}</p>
      </div>

      <p class="hint label">
        Außerhalb hat sich etwas bewegt, das die Werkbank nicht still aufholen kann: zwei Seiten
        halten dieses Artefakt für ihres. Schließe in Ruhe — der Abgleich meldet sich beim nächsten
        Öffnen erneut, bis ihr euch abgestimmt habt. Es geht nichts verloren.
      </p>
    </div>

    <footer class="foot">
      <button class="schliessen" onclick={onClose}>verstanden</button>
    </footer>
  </section>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60; /* the loud open-time moment, on the same plane as the laute Ausnahme */
    display: grid;
    place-items: center;
    padding: 24px;
    background:
      radial-gradient(
        120% 90% at 50% 0%,
        rgba(240, 66, 28, 0.16),
        transparent 60%
      ),
      rgba(8, 7, 6, 0.66);
    backdrop-filter: blur(2px);
    animation: scrim-in 180ms var(--ease);
  }
  @keyframes scrim-in {
    from {
      opacity: 0;
    }
  }

  .abgleich {
    width: min(560px, 100%);
    background: var(--surface-raised);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    box-shadow:
      0 0 0 1px rgba(240, 66, 28, 0.35),
      0 0 38px -6px rgba(240, 66, 28, 0.5),
      0 26px 64px -16px rgba(8, 7, 6, 0.66),
      0 2px 0 rgba(255, 255, 255, 0.55) inset;
    overflow: hidden;
    animation: abgleich-in 240ms var(--ease) backwards;
  }
  @keyframes abgleich-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
  }

  .strip {
    height: 5px;
    background: linear-gradient(
      90deg,
      var(--accent),
      #ff6a3d 50%,
      var(--accent)
    );
  }

  .head {
    padding: 18px 22px 6px;
  }
  .kicker {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--accent);
    margin-bottom: 10px;
  }
  .beacon {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 8px rgba(240, 66, 28, 0.7);
    animation: beacon 1.5s ease-in-out infinite;
  }
  @keyframes beacon {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.45;
      transform: scale(0.82);
    }
  }
  .frage {
    margin: 0;
    font-family: var(--font-label);
    font-weight: 700;
    font-size: 21px;
    line-height: 1.25;
    letter-spacing: -0.01em;
    color: var(--ink-strong);
  }

  .body {
    padding: 14px 22px 6px;
  }

  .artefakte {
    border-radius: var(--radius-sm);
    background: linear-gradient(180deg, #131110, #0b0a09);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.9);
    padding: 10px 12px;
  }
  .artefakte-label {
    display: block;
    color: #6b6660;
    font-size: 9.5px;
    margin-bottom: 7px;
  }
  .artefakte-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .artefakt {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12.5px;
    color: var(--screen-fg);
  }
  .artefakt .led {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--led-attention);
    box-shadow: 0 0 6px rgba(240, 66, 28, 0.6);
  }
  .artefakt-path {
    overflow-wrap: anywhere;
  }

  /* The honest naming of the three truth-places — quiet, on a recessed instrument line. */
  .orte {
    margin-top: 12px;
  }
  .orte-label {
    display: block;
    color: var(--ink-muted);
    font-size: 9.5px;
    margin-bottom: 5px;
  }
  .ort-zeile {
    margin: 0;
    font-size: 12.5px;
    color: var(--ink-default);
  }

  .hint {
    margin: 14px 0 2px;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--ink-default);
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    padding: 16px 22px 20px;
  }
  .schliessen {
    appearance: none;
    cursor: pointer;
    padding: 11px 20px;
    border-radius: var(--radius);
    border: 1px solid var(--hairline);
    background: var(--key-light);
    font-family: var(--font-label);
    font-weight: 700;
    font-size: 14px;
    color: var(--ink-strong);
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.12);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .schliessen:hover {
    border-color: var(--accent);
  }
  .schliessen:active {
    transform: translateY(1px);
    box-shadow: none;
  }
  .schliessen:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .beacon {
      animation: none;
    }
  }
</style>
