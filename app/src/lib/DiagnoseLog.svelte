<script lang="ts">
  // Issue #54-Folge — Diagnose-Log-Panel. The two push types are swallowed by design (the silent
  // vocabulary), so a push that does nothing leaves no trace in the UI. This panel breaks that
  // silence ON DEMAND only: a recessed, toggleable readout of the backend's git/sync diagnostic
  // ring — every Lock-Warden decision (incl. the common „Refuse" = nothing to push) and every real
  // git exit + stderr. It never changes behavior; it only lets the user SEE why. Stays out of the
  // daily rhythm: closed by default, polls only while open.
  import { invoke } from "@tauri-apps/api/core";

  let {
    open = false,
    onClose = () => {},
  }: {
    open?: boolean;
    onClose?: () => void;
  } = $props();

  let lines = $state<string[]>([]);
  let filePath = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;
  // Stick to the bottom (newest) unless the user has scrolled up to read history.
  let view: HTMLDivElement | null = $state(null);
  let pinned = $state(true);

  async function refresh() {
    try {
      lines = await invoke<string[]>("read_git_log");
      if (pinned && view) view.scrollTop = view.scrollHeight;
    } catch {
      // The panel is purely diagnostic; a read hiccup must not surface as a loud error.
    }
  }

  async function loadPath() {
    try {
      filePath = await invoke<string | null>("git_log_path");
    } catch {
      filePath = null;
    }
  }

  async function clear() {
    try {
      await invoke("clear_git_log");
      lines = [];
    } catch {
      // ignore — the on-disk file remains the durable record anyway
    }
  }

  function onScroll() {
    if (!view) return;
    // Within ~24px of the bottom counts as „following the tail".
    pinned = view.scrollHeight - view.scrollTop - view.clientHeight < 24;
  }

  // Poll only while open — closed panel does zero work, keeping the daily rhythm quiet.
  $effect(() => {
    if (open) {
      void loadPath();
      void refresh();
      timer = setInterval(() => void refresh(), 1500);
      return () => {
        if (timer !== null) clearInterval(timer);
        timer = null;
      };
    }
  });

  // Tag each line by its leading [kind] so git failures read hotter than the calm warden trace.
  function kindOf(line: string): "git" | "warden" | "other" {
    const m = line.match(/\]\s+\[(\w+)\]/);
    const k = m?.[1];
    return k === "git" || k === "warden" ? k : "other";
  }
  function isFail(line: string): boolean {
    return /stderr:|FEHLER|TIMEOUT|FAILED/.test(line);
  }
</script>

{#if open}
  <section class="panel" aria-label="Diagnose">
    <header class="bar">
      <span class="dot" aria-hidden="true"></span>
      <span class="title label">Diagnose · Sync &amp; Sicherung</span>
      <span class="tally mono">{lines.length.toString().padStart(3, "0")}</span>
      <span class="spacer"></span>
      <button class="act label" onclick={clear} title="Anzeige leeren (Datei bleibt erhalten)">leeren</button>
      <button class="act label" onclick={onClose} title="Schließen">schließen</button>
    </header>

    <div class="view" bind:this={view} onscroll={onScroll}>
      {#if lines.length === 0}
        <p class="empty mono">— noch keine Ereignisse — eine Sicherung/Freigabe auslösen —</p>
      {:else}
        {#each lines as line, i (i)}
          <pre class="line mono" class:git={kindOf(line) === "git"} class:warden={kindOf(line) === "warden"} class:fail={isFail(line)}>{line}</pre>
        {/each}
      {/if}
    </div>

    {#if filePath}
      <footer class="foot mono" title={filePath}>
        <span class="foot-k label">Datei</span>
        <code class="foot-path">{filePath}</code>
      </footer>
    {/if}
  </section>
{/if}

<style>
  .panel {
    position: fixed;
    right: 16px;
    bottom: 16px;
    width: min(640px, calc(100vw - 32px));
    max-height: 56vh;
    display: flex;
    flex-direction: column;
    background: var(--surface-base);
    border: 1px solid var(--ink-strong);
    border-radius: var(--radius);
    box-shadow: 0 18px 48px -16px rgba(0, 0, 0, 0.55);
    z-index: 60;
    overflow: hidden;
    animation: rise 200ms var(--ease) backwards;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface-sunken);
  }
  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 99px;
    background: var(--ink-muted);
    box-shadow: 0 0 6px -1px var(--ink-muted);
  }
  .title {
    flex: none;
    color: var(--ink-default);
    font-size: 10px;
  }
  .tally {
    flex: none;
    font-size: 10px;
    color: var(--ink-muted);
    background: var(--surface-raised);
    border-radius: 99px;
    padding: 1px 7px;
  }
  .spacer {
    flex: 1;
  }
  .act {
    appearance: none;
    cursor: pointer;
    background: none;
    border: none;
    color: var(--ink-muted);
    font-size: 9px;
    padding: 2px 4px;
    transition: color var(--dur) var(--ease);
  }
  .act:hover {
    color: var(--ink-strong);
  }

  .view {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .empty {
    color: var(--ink-muted);
    font-size: 11px;
    opacity: 0.7;
    margin: 8px 0;
  }
  .line {
    margin: 0;
    font-size: 10.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--ink-muted);
  }
  /* The warden trace is the calm baseline; a real git command reads a touch stronger. */
  .line.git {
    color: var(--ink-default);
  }
  .line.warden {
    color: var(--ink-muted);
  }
  /* A failure (stderr / FEHLER / TIMEOUT) is the one thing the user is hunting — make it hot. */
  .line.fail {
    color: var(--accent);
    background: var(--key-light);
    border-radius: var(--radius-sm);
    padding: 1px 4px;
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-top: 1px solid var(--hairline);
    background: var(--surface-sunken);
    font-size: 9.5px;
  }
  .foot-k {
    flex: none;
    color: var(--ink-muted);
    font-size: 9px;
  }
  .foot-path {
    flex: 1;
    min-width: 0;
    color: var(--ink-default);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
