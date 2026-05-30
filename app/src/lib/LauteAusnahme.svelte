<script lang="ts">
  import type { LoudQuestion, StandChoice } from "./types";

  // The "laute Ausnahme" (Issue #11, E41) — the SINGLE place the UI raises its voice. The stiller
  // Sync has hit a real, unmergeable contradiction (a binary or KiCad source both sides changed),
  // so instead of letting a merge silently corrupt the file it STOPS and asks one question in the
  // tool's OWN domain language: „dein und X' Gehäuse-Stand widersprechen sich — welcher gilt?".
  //
  // This is the one orange-frame moment in the whole instrument (ui-stilbeschreibung §1): the
  // rationed accent is finally spent here, on a true exception. There are NO git conflict markers
  // anywhere — the question, the contested artifacts and the two stands are all domain language.
  // The two stands are offered as physical "key" cards; choosing one is a deliberate press.

  let {
    question,
    busy = false,
    onChoose,
  }: {
    question: LoudQuestion;
    busy?: boolean;
    onChoose: (choice: StandChoice) => void;
  } = $props();
</script>

<!-- The loud exception is modal and unskippable: there is no scrim-dismiss and no cancel. A real
     contradiction must be resolved — the user picks whose stand applies. -->
<div class="scrim" role="presentation">
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="ausnahme"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="ausnahme-frage"
  >
    <!-- The one orange strip in the instrument: this is where the voice rises. -->
    <div class="strip" aria-hidden="true"></div>

    <header class="head">
      <span class="kicker label">
        <span class="beacon" aria-hidden="true"></span>
        Widerspruch · welcher Stand gilt?
      </span>
      <h2 id="ausnahme-frage" class="frage">{question.frage}</h2>
    </header>

    <div class="body">
      <!-- The contested artifacts, named as artifacts on a recessed instrument readout —
           never as git refs, never with conflict markers. -->
      <div class="artefakte" role="group" aria-label="Betroffene Artefakte">
        <span class="artefakte-label label">Betroffen</span>
        <ul class="artefakte-list mono">
          {#each question.artefakte as a (a)}
            <li class="artefakt">
              <span class="led" aria-hidden="true"></span>
              <span class="artefakt-path">{a}</span>
            </li>
          {/each}
        </ul>
      </div>

      <p class="hint label">
        Beide Seiten haben diesen Stand geändert. Er lässt sich nicht zusammenführen
        — wähle, welcher gilt. Der andere bleibt als früherer Stand erhalten.
      </p>
    </div>

    <!-- The two stands as physical key cards. One press resolves the exception. -->
    <footer class="foot">
      {#each question.optionen as opt (opt.choice)}
        <button
          class="stand-key"
          class:mine={opt.choice === "mine"}
          onclick={() => onChoose(opt.choice)}
          disabled={busy}
        >
          <span class="stand-which label"
            >{opt.choice === "mine" ? "behalten" : "übernehmen"}</span
          >
          <span class="stand-label">{opt.label}</span>
        </button>
      {/each}
    </footer>
  </section>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60; /* above the ceremony / gate — this is the loudest moment */
    display: grid;
    place-items: center;
    padding: 24px;
    /* a faint warm-orange wash bleeds into the dim scrim: the room itself leans in */
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

  /* The single orange-framed panel. The accent is spent HERE and nowhere else. */
  .ausnahme {
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
    animation: ausnahme-in 240ms var(--ease) backwards;
  }
  @keyframes ausnahme-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
  }

  /* The orange strip — the visual "the voice is raised" cue. */
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
  /* a small pulsing beacon — the only animated orange dot in the app */
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

  /* Recessed instrument readout of the contested artifacts — same dark LCD as the VersionBar. */
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
  /* the contested artifact gets the attention LED — it is the thing in conflict */
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
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    padding: 16px 22px 20px;
  }

  /* The two stands as physical keys: a deliberate press resolves the exception. "Mine" is the
     calm light cap; "theirs" carries the orange edge of the act of taking the colleague's stand.
     Neither is pre-selected — the choice is the user's. */
  .stand-key {
    appearance: none;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 13px 16px;
    border-radius: var(--radius);
    border: 1px solid var(--hairline);
    background: var(--key-light);
    box-shadow: 0 1px 0 rgba(28, 26, 25, 0.12);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .stand-key:hover:not(:disabled) {
    background: #f5f3ee;
    border-color: var(--accent);
  }
  .stand-key:active:not(:disabled) {
    transform: translateY(1px);
    box-shadow: 0 0 0 rgba(28, 26, 25, 0.12);
  }
  .stand-key:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .stand-key:disabled {
    cursor: default;
    opacity: 0.55;
    box-shadow: none;
  }
  .stand-which {
    color: var(--ink-muted);
    font-size: 9.5px;
  }
  .stand-label {
    font-family: var(--font-label);
    font-weight: 700;
    font-size: 15px;
    color: var(--ink-strong);
    letter-spacing: -0.005em;
  }
  /* taking the colleague's stand is the louder act — its key wears the accent edge */
  .stand-key:not(.mine) .stand-which {
    color: var(--accent);
  }
  .stand-key:not(.mine) {
    border-color: rgba(240, 66, 28, 0.45);
  }

  @media (prefers-reduced-motion: reduce) {
    .beacon {
      animation: none;
    }
  }
</style>
