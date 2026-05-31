<script lang="ts">
  // The „schwarze Gate-Taste" (Issue #46, E43/E27/E38): the one black, separated danger key
  // for the deliberate „Historie anfassen" moment. It is walled off on its own recessed dark
  // plate and is INERT until the user toggles an explicit consent — only then does the black
  // cap light and become pressable, so it is never clicked by accident. The arm checkbox
  // resets after a press so the consent is spent each time.
  //
  // E43 keeps the dangerous mechanics hidden: this shell carries the AFFORDANCE (the heavy,
  // armed press), the caller supplies the honest-but-non-plumbing wording. It is the reusable
  // black-key shell the destructive HistorieGate uses and the Freigabe-Gate (Issue #52) wires
  // into for its „Historie anfassen"-grade actions; #52 owns the three-stage block logic and
  // simply renders this key with its own `consent` / `label` / `onPress`.

  let {
    /** The explicit consent sentence beside the box (e.g. „Ich schreibe die Historie bewusst um"). */
    consent,
    /** The key cap label once armed (e.g. „Historie umschreiben"). */
    label,
    /** Shown on the cap while the action runs. */
    busyLabel = "…",
    busy = false,
    /** Block the whole zone (consent + key) — e.g. a hard block upstream. */
    disabled = false,
    /** Fired on the deliberate armed press. The caller runs the dangerous action. */
    onPress,
  }: {
    consent: string;
    label: string;
    busyLabel?: string;
    busy?: boolean;
    disabled?: boolean;
    onPress: () => void;
  } = $props();

  // Two-step arming: the danger key is dead until the user toggles consent. Spent on press.
  let armed = $state(false);

  function press() {
    if (!armed || busy || disabled) return;
    onPress();
    armed = false;
  }
</script>

<!-- The separated black danger zone: arm, then the heavy key becomes pressable. -->
<div class="danger" class:armed class:disabled>
  <label class="arm">
    <input type="checkbox" bind:checked={armed} disabled={busy || disabled} />
    <span class="arm-box" aria-hidden="true"></span>
    <span class="label arm-text">{consent}</span>
  </label>

  <button class="key danger-key" onclick={press} disabled={!armed || busy || disabled}>
    <span class="label">{busy ? busyLabel : label}</span>
  </button>
</div>

<style>
  /* The separated danger zone — visually walled off, dark, on its own recessed plate.
     Matches the HistorieGate black-key language exactly (same tokens, same weight). */
  .danger {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--radius);
    border: 1px solid #2a2724;
    background: linear-gradient(180deg, #1a1817, #0e0d0c);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.7);
  }
  .danger.disabled {
    opacity: 0.55;
  }

  .arm {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }
  .danger.disabled .arm {
    cursor: not-allowed;
  }
  .arm input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }
  .arm-box {
    width: 15px;
    height: 15px;
    flex: none;
    border-radius: var(--radius-sm);
    border: 1px solid #4a4641;
    background: #0b0a09;
    box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.8);
    transition:
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .danger.armed .arm-box {
    background: var(--led-attention);
    border-color: var(--led-attention);
    box-shadow: 0 0 8px rgba(240, 66, 28, 0.55);
  }
  .arm input:focus-visible + .arm-box {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .arm-text {
    color: #b8b4ad;
    font-size: 10.5px;
    max-width: 13ch;
    line-height: 1.25;
  }
  .danger.armed .arm-text {
    color: var(--screen-fg);
  }

  /* The black danger key: dark cap, only lit and pressable once armed. */
  .key {
    appearance: none;
    cursor: pointer;
    border-radius: var(--radius);
    padding: 9px 15px;
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .key .label {
    color: inherit;
  }
  .danger-key {
    background: #000;
    color: #6b6660;
    border: 1px solid #322e2a;
    box-shadow: none;
    white-space: nowrap;
  }
  .danger-key:disabled {
    opacity: 1; /* it reads dark-and-dead rather than faded */
    cursor: not-allowed;
    color: #4a4641;
  }
  .danger.armed .danger-key:not(:disabled) {
    color: var(--accent-ink);
    border-color: var(--led-attention);
    box-shadow:
      0 0 0 1px rgba(240, 66, 28, 0.45),
      0 0 14px -2px rgba(240, 66, 28, 0.5);
    cursor: pointer;
  }
  .danger.armed .danger-key:not(:disabled):hover {
    background: #0a0a0a;
  }
  .danger.armed .danger-key:not(:disabled):active {
    transform: translateY(1px);
  }
</style>
