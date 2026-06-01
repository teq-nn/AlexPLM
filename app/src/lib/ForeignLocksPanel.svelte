<script lang="ts">
  import type { ForeignLock } from "./types";

  // The live "Belegte Bausteine" panel (E37): which Bausteine colleagues currently hold. Read purely
  // from `git lfs locks`, never a presence
  // service. A dark instrument-display zone — the same LCD language as the VersionBar — because
  // foreign coordination state belongs to the "screen" world, not the warm chassis.
  let { locks = [] }: { locks?: ForeignLock[] } = $props();

  // Show only the filename loudly; the path stays as a muted second line (we never hide the
  // filesystem). Owner + timestamp carry the "gesperrt von X seit …" coordination.
  const fileName = (p: string) => p.split("/").pop() ?? p;
</script>

<aside class="panel" aria-label="Belegte Bausteine">
  <div class="head">
    <span class="label title">Belegte Bausteine</span>
    <span class="count mono">{locks.length.toString().padStart(2, "0")}</span>
  </div>

  <div class="list">
    {#if locks.length === 0}
      <p class="idle mono">keine belegten Bausteine</p>
    {:else}
      {#each locks as lock (lock.path + lock.owner)}
        <div class="row" title={lock.tooltip}>
          <span class="dot" aria-hidden="true"></span>
          <div class="row-body">
            <div class="file mono">{fileName(lock.path)}</div>
            <div class="path mono">{lock.path}</div>
            <div class="meta mono">
              gesperrt von <span class="who">{lock.owner}</span>
              <span class="since">seit {lock.locked_at}</span>
            </div>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</aside>

<style>
  .panel {
    flex: none;
    width: 252px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--screen-bg);
    color: var(--screen-fg);
    border-left: 1px solid #000;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 13px 14px;
    border-bottom: 1px solid #1c1a18;
  }
  .title {
    color: #8a857d;
  }
  .count {
    font-size: 13px;
    font-weight: 600;
    color: var(--screen-fg);
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 10px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .idle {
    color: #6b6660;
    font-size: 12px;
    padding: 6px 4px;
  }

  /* Each foreign lock: a recessed dark chip with an orange LED — the rationed loud accent,
     here because someone else holding a lock is exactly the "attention" state. */
  .row {
    display: flex;
    gap: 9px;
    padding: 10px 11px;
    border-radius: var(--radius);
    background: linear-gradient(180deg, #151312, #0c0b0a);
    box-shadow:
      inset 0 1px 2px rgba(0, 0, 0, 0.8),
      inset 0 0 0 1px rgba(255, 255, 255, 0.025);
    animation: row-in 240ms var(--ease) backwards;
  }
  .dot {
    flex: none;
    margin-top: 3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 7px 1px color-mix(in srgb, var(--accent) 70%, transparent);
  }
  .row-body {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .file {
    font-size: 12px;
    font-weight: 600;
    color: var(--screen-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    font-size: 10px;
    color: #6b6660;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    margin-top: 3px;
    font-size: 10px;
    color: #b8b4ad;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta .who {
    color: var(--accent);
    font-weight: 600;
  }
  .meta .since {
    color: #6b6660;
  }

  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
