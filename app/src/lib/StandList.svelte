<script lang="ts">
  import Led from "./Led.svelte";
  import type { Stand } from "./types";

  let { stands }: { stands: Stand[] } = $props();

  // The most recently arrived Stand gets a one-shot pulse, then settles to grey.
  // Keyed by the list so each new head element animates exactly once on mount.

  // Render the wall-clock time compactly; keep the full machine stamp in the title.
  function clock(ts: string): string {
    // ts is "YYYY-MM-DDTHH:MM:SSZ"
    const t = ts.slice(11, 19);
    return t || ts;
  }
  function day(ts: string): string {
    return ts.slice(0, 10);
  }
</script>

<aside class="rail">
  <div class="rail-head">
    <span class="label title">Commits</span>
    <span class="count mono">{stands.length.toString().padStart(2, "0")}</span>
  </div>

  <div class="strip">
    {#if stands.length === 0}
      <p class="idle label">Noch keine Commits — speichern erzeugt einen</p>
    {:else}
      <ol class="list">
        {#each stands as s, i (s.id)}
          <li class="entry" class:fresh={i === 0}>
            <span class="tick"></span>
            <Led status="working" title="gesichert" />
            <div class="meta">
              <span class="path mono" title={s.path}>{s.path}</span>
              <span class="time mono" title={`${day(s.timestamp)} ${clock(s.timestamp)}`}>
                <span class="t">{clock(s.timestamp)}</span>
                <span class="d">{day(s.timestamp)}</span>
              </span>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </div>
</aside>

<style>
  /* A narrow instrument "log strip" rail — the running ledger of Stände.
     Routine = grey: no orange anywhere here. */
  .rail {
    display: flex;
    flex-direction: column;
    width: 264px;
    flex: none;
    min-height: 0;
    border-left: 1px solid var(--hairline);
    background: var(--surface-sunken);
  }

  .rail-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 11px 14px;
    border-bottom: 1px solid var(--hairline);
  }
  .title {
    color: var(--ink-muted);
  }
  .count {
    color: var(--ink-default);
    font-size: 13px;
    font-weight: 600;
  }

  .strip {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 0;
  }

  .idle {
    color: var(--ink-muted);
    padding: 14px;
    line-height: 1.5;
    text-transform: none;
    letter-spacing: 0;
    font-size: 11px;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  /* Each Stand is a seated row with a left "rail tick" — the ledger reads top-down,
     newest first, like punches on a tape. */
  .entry {
    position: relative;
    display: grid;
    grid-template-columns: 10px 9px 1fr;
    align-items: center;
    gap: 9px;
    padding: 8px 14px 8px 12px;
  }
  .entry + .entry {
    border-top: 1px solid color-mix(in srgb, var(--hairline) 60%, transparent);
  }

  /* The rail tick: a short vertical hairline connecting entries down the strip. */
  .tick {
    justify-self: center;
    width: 1px;
    height: 100%;
    background: var(--hairline);
  }
  .entry:first-child .tick {
    background: linear-gradient(180deg, transparent 0 50%, var(--hairline) 50% 100%);
  }
  .entry:last-child .tick {
    background: linear-gradient(180deg, var(--hairline) 0 50%, transparent 50% 100%);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .path {
    color: var(--ink-default);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl; /* keep the tail (filename) visible when truncating */
    text-align: left;
  }
  .time {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }
  .time .t {
    color: var(--ink-muted);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.02em;
  }
  .time .d {
    color: color-mix(in srgb, var(--ink-muted) 70%, transparent);
    font-size: 10px;
  }

  /* Fresh Stand: a single quiet pulse from a faint highlight back to grey routine.
     No orange — arrival is acknowledged, not alarmed. */
  .entry.fresh {
    animation: settle 1100ms var(--ease) 1;
  }
  @keyframes settle {
    0% {
      background: color-mix(in srgb, var(--surface-raised) 92%, var(--ink-default));
      box-shadow: inset 2px 0 0 var(--led-working);
    }
    100% {
      background: transparent;
      box-shadow: inset 2px 0 0 transparent;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .entry.fresh {
      animation: none;
    }
  }
</style>
