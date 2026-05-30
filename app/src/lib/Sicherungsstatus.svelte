<script lang="ts">
  import type { WardenAction } from "./types";

  // The Sicherungsstatus readout (Issue #9, E35/E39): the calm instrument-LCD surface that
  // names the Lock Warden's two push types in the tool's OWN vocabulary — never raw git.
  // The daily sync stays silent; what the user ever sees is "dein Stand ist gesichert" (a
  // private Sicherungs-Push) or "freigegeben" (the public Freigabe-Push that also releases the
  // lock). `auto-unlock` reports the self-healing "Sperre gelöst"; `refuse` shows nothing.
  //
  // Same recessed-LCD language as the "geteilt" readout and the import-outcome chip in
  // +page.svelte: a backlit screen that flickers on the instant a checkpoint fires, then
  // settles to a steady glow. A Freigabe lights the green "free/done" LED (the public act);
  // a Sicherung the quiet grey "working" LED (the private net).

  let { action = null }: { action?: WardenAction | null } = $props();

  type Read = { led: "free" | "working"; word: string; sub: string };

  // Refuse (and null) surface as nothing — the readout simply isn't shown.
  const READOUTS: Record<Exclude<WardenAction, "refuse">, Read> = {
    "freigabe-push": {
      led: "free",
      word: "freigegeben",
      sub: "auf dem geteilten Stand · Sperre gelöst",
    },
    "sicherungs-push": {
      led: "working",
      word: "gesichert",
      sub: "dein Stand ist gesichert",
    },
    "auto-unlock": {
      led: "free",
      word: "Sperre gelöst",
      sub: "sauber — nichts mehr offen",
    },
  };

  const read = $derived(
    action && action !== "refuse" ? READOUTS[action] : null,
  );
</script>

{#if read}
  <!-- keyed on the action so a new checkpoint re-triggers the flicker-in animation -->
  {#key action}
    <span
      class="sicherungsstatus mono"
      role="status"
      aria-live="polite"
      title={read.sub}
    >
      <span class="dot {read.led}" aria-hidden="true"></span>
      <span class="word">{read.word}</span>
      <span class="sep" aria-hidden="true">·</span>
      <span class="sub">{read.sub}</span>
    </span>
  {/key}
{/if}

<style>
  /* A recessed dark LCD readout — the same instrument language as the VersionBar screen and
     the import-outcome / "geteilt" chips: deep inset, faint inner highlight, backlit text. */
  .sicherungsstatus {
    display: inline-flex;
    align-items: baseline;
    gap: 8px;
    padding: 5px 12px;
    border-radius: var(--radius);
    background: linear-gradient(180deg, #131110, #0b0a09);
    box-shadow:
      inset 0 1px 2px rgba(0, 0, 0, 0.9),
      inset 0 0 0 1px rgba(255, 255, 255, 0.03);
    color: var(--screen-fg);
    font-size: 12px;
    letter-spacing: 0.01em;
    /* the checkpoint moment: a brief backlight flicker, then a steady settle */
    animation: lcd-flicker 520ms var(--ease) backwards;
  }

  .dot {
    align-self: center;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    /* a slow breathing pulse so the screen reads as "live", not a static print */
    animation: pulse 2.6s ease-in-out infinite;
  }
  /* Freigabe = the public "done" act -> green free LED */
  .dot.free {
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.55);
  }
  /* Sicherung = the quiet private net -> neutral working LED */
  .dot.working {
    background: var(--led-working);
    box-shadow: 0 0 5px rgba(201, 198, 191, 0.35);
  }

  .word {
    color: var(--screen-fg);
    font-weight: 600;
  }
  .sep {
    color: #4a4641;
  }
  .sub {
    color: #b8b4ad;
  }

  /* Flicker the backlight on at the checkpoint instant, then hold steady — an LCD waking up. */
  @keyframes lcd-flicker {
    0% {
      opacity: 0;
      transform: translateY(-2px);
    }
    35% {
      opacity: 0.55;
    }
    50% {
      opacity: 1;
    }
    62% {
      opacity: 0.7;
    }
    100% {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot {
      animation: none;
    }
  }
</style>
