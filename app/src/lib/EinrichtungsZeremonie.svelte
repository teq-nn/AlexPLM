<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { SetupReport } from "./types";

  // The one-time Einrichtungs-Zeremonie (Issue #5, E41). This is the explicit, rare exception
  // where the tool may speak git-near: connect a self-hosted Forgejo/Gitea server, publish the
  // product with the first push, and invite a colleague by handing them the clone URL. The flow
  // is a deliberate, separated ceremony — never part of the silent daily rhythm.
  let {
    productPath,
    report,
    onUpdated,
    onClose,
  }: {
    productPath: string;
    report: SetupReport;
    /** Bubble the refreshed ceremony state up so the shell can settle into daily use. */
    onUpdated: (r: SetupReport) => void;
    onClose: () => void;
  } = $props();

  // The visible step is driven by the (server-decided) stage so a reopened ceremony always lands
  // on the right rung: connect → publish → invite.
  let stage = $derived(report.stage);

  // Connect-step form fields. Credentials stay in this component and go straight to Rust; the
  // returned report only ever carries the credential-free clone URL.
  let host = $state("");
  let owner = $state("");
  let repo = $state("");
  let user = $state("");
  let token = $state("");

  let busy = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);

  async function connect() {
    error = null;
    busy = true;
    try {
      const r = await invoke<SetupReport>("connect_server", {
        path: productPath,
        host,
        owner,
        repo,
        user,
        token,
      });
      // Don't keep the secret around once it's been handed to git.
      token = "";
      onUpdated(r);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function publish() {
    error = null;
    busy = true;
    try {
      const r = await invoke<SetupReport>("publish_to_server", {
        path: productPath,
      });
      onUpdated(r);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function copyClone() {
    if (!report.clone_url) return;
    try {
      await navigator.clipboard.writeText(report.clone_url);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      // clipboard may be unavailable in the WebView; the URL is shown to copy by hand.
    }
  }

  // The connect button is inert until the three URL parts are present (credentials optional).
  let canConnect = $derived(
    host.trim() !== "" && owner.trim() !== "" && repo.trim() !== "",
  );
</script>

<div class="scrim" role="presentation" onclick={() => !busy && onClose()}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="ceremony"
    role="dialog"
    aria-modal="true"
    aria-labelledby="ceremony-title"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="head">
      <span class="label kicker">Einmalige Einrichtung</span>
      <h2 id="ceremony-title" class="title">Produkt teilen</h2>
      <p class="sub label">
        Ein Mal pro Produkt: Server anbinden, veröffentlichen, Kollegen einladen.
      </p>
    </header>

    <!-- The three rungs, lit by the server-decided stage. -->
    <ol class="steps mono" aria-hidden="true">
      <li class:done={stage !== "not-configured"} class:active={stage === "not-configured"}>
        <span class="num">01</span><span class="step-label label">Server</span>
      </li>
      <li
        class:done={stage === "eingerichtet"}
        class:active={stage === "remote-set-not-published"}
      >
        <span class="num">02</span><span class="step-label label">Veröffentlichen</span>
      </li>
      <li class:active={stage === "eingerichtet"}>
        <span class="num">03</span><span class="step-label label">Einladen</span>
      </li>
    </ol>

    <div class="body">
      {#if stage === "not-configured"}
        <!-- Step 1: connect the self-hosted Forgejo/Gitea server. -->
        <p class="lede">
          Gib die Adresse deines selbst-gehosteten <strong>Forgejo / Gitea</strong>-Servers
          und das Produkt-Ziel an. Die Zugangsdaten bleiben lokal — sie werden nie angezeigt.
        </p>

        <div class="form">
          <label class="field">
            <span class="label fk">Server-Adresse</span>
            <input
              class="mono in"
              bind:value={host}
              placeholder="https://forge.example.de"
              autocomplete="off"
              spellcheck="false"
            />
          </label>
          <div class="row">
            <label class="field">
              <span class="label fk">Besitzer / Team</span>
              <input
                class="mono in"
                bind:value={owner}
                placeholder="team"
                autocomplete="off"
                spellcheck="false"
              />
            </label>
            <label class="field">
              <span class="label fk">Produkt-Name</span>
              <input
                class="mono in"
                bind:value={repo}
                placeholder="ember-reverb"
                autocomplete="off"
                spellcheck="false"
              />
            </label>
          </div>
          <div class="row">
            <label class="field">
              <span class="label fk">Benutzer</span>
              <input
                class="mono in"
                bind:value={user}
                placeholder="optional"
                autocomplete="off"
                spellcheck="false"
              />
            </label>
            <label class="field">
              <span class="label fk">Zugangs-Token</span>
              <input
                class="mono in"
                type="password"
                bind:value={token}
                placeholder="optional"
                autocomplete="off"
              />
            </label>
          </div>
        </div>
      {:else if stage === "remote-set-not-published"}
        <!-- Step 2: the first push publishes the product. -->
        <p class="lede">
          Der Server ist angebunden und die Sperren-Prüfung (<span class="mono">locksverify</span>)
          ist aktiv. Jetzt das Produkt <strong>einmalig veröffentlichen</strong> — danach läuft
          alles still im Hintergrund.
        </p>
        {#if report.clone_url}
          <div class="readout mono" role="status">
            <span class="dot ok" aria-hidden="true"></span>
            <span class="rk">Ziel</span>
            <span class="rv">{report.clone_url}</span>
          </div>
        {/if}
      {:else}
        <!-- Step 3: invite a colleague by handing them the clone URL. Settled state. -->
        <p class="lede">
          <strong>Veröffentlicht.</strong> Das Produkt liegt auf dem Server. Lade einen Kollegen
          ein, indem du ihm diese Adresse zum Klonen gibst:
        </p>
        {#if report.clone_url}
          <div class="invite">
            <code class="clone mono">{report.clone_url}</code>
            <button class="key copy" onclick={copyClone}>
              <span class="label">{copied ? "kopiert" : "kopieren"}</span>
            </button>
          </div>
          <p class="hint label">
            Der Kollege legt das Produkt damit als eigene Kopie an — ab dann arbeitet ihr still
            auf demselben Stand.
          </p>
        {/if}
      {/if}

      {#if error}
        <div class="err" role="alert">
          <span class="dot warn" aria-hidden="true"></span>
          <span class="err-text label">{error}</span>
        </div>
      {/if}
    </div>

    <footer class="foot">
      <button class="key ghost" onclick={onClose} disabled={busy}>
        <span class="label">{stage === "eingerichtet" ? "Schließen" : "Später"}</span>
      </button>

      {#if stage === "not-configured"}
        <button class="key go" onclick={connect} disabled={!canConnect || busy}>
          <span class="label">{busy ? "binde an …" : "Server anbinden"}</span>
        </button>
      {:else if stage === "remote-set-not-published"}
        <button class="key go" onclick={publish} disabled={busy}>
          <span class="label">{busy ? "veröffentliche …" : "Veröffentlichen"}</span>
        </button>
      {:else}
        <button class="key go" onclick={onClose}>
          <span class="label">Fertig</span>
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

  .ceremony {
    width: min(560px, 100%);
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow:
      0 24px 60px -16px rgba(8, 7, 6, 0.6),
      0 2px 0 rgba(255, 255, 255, 0.5) inset;
    overflow: hidden;
    animation: ceremony-in 200ms var(--ease) backwards;
  }
  @keyframes ceremony-in {
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

  /* Three rungs: a thin instrument progress strip. Active rung lit, done rungs filled grey. */
  .steps {
    list-style: none;
    margin: 16px 22px 2px;
    padding: 0;
    display: flex;
    gap: 8px;
  }
  .steps li {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--hairline);
    background: var(--surface-sunken);
    color: var(--ink-muted);
    opacity: 0.7;
    transition:
      opacity var(--dur) var(--ease),
      border-color var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .steps li.done {
    opacity: 1;
    background: var(--surface-raised);
    color: var(--ink-default);
  }
  .steps li.active {
    opacity: 1;
    border-color: var(--ink-strong);
    background: var(--key-light);
    color: var(--ink-strong);
  }
  .num {
    font-size: 11px;
    color: var(--ink-muted);
  }
  .steps li.active .num {
    color: var(--ink-strong);
  }
  .step-label {
    font-size: 10.5px;
  }

  .body {
    padding: 14px 22px 18px;
  }
  .lede {
    margin: 0 0 14px;
    color: var(--ink-default);
    font-size: 14px;
    line-height: 1.5;
  }
  .lede strong {
    color: var(--ink-strong);
  }
  .lede .mono {
    font-size: 12.5px;
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

  /* Dark instrument readout for the connected target (step 2). */
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
  .readout .rk {
    color: #6b6660;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 9.5px;
  }
  .readout .rv {
    color: var(--screen-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The invite: a copyable clone URL on a recessed plate + a copy key. */
  .invite {
    display: flex;
    align-items: stretch;
    gap: 10px;
  }
  .clone {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    padding: 10px 12px;
    font-size: 12.5px;
    color: var(--screen-fg);
    background: linear-gradient(180deg, #131110, #0b0a09);
    border-radius: var(--radius-sm);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.9);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: all;
  }
  .hint {
    margin: 12px 0 0;
    color: var(--ink-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 12px;
    line-height: 1.45;
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
  .dot.ok {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--led-free);
    box-shadow: 0 0 6px rgba(60, 154, 75, 0.5);
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
  /* The forward key carries the dark, weighted cap — this ceremony is a deliberate act. */
  .key.go {
    background: var(--key-dark);
    color: var(--key-light);
    border-color: var(--key-dark);
  }
  .key.go:hover:not(:disabled) {
    background: #2a2724;
  }
  .key.copy {
    flex: none;
    white-space: nowrap;
  }
</style>
