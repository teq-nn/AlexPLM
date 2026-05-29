<script lang="ts">
  import type { Baustein } from "./types";
  import Led from "./Led.svelte";

  let { baustein, index = 0 }: { baustein: Baustein; index?: number } = $props();

  // Split the main file into directory + filename so the filename can carry weight
  // while the real path stays visible but muted (the tool never hides the filesystem).
  const file = $derived(baustein.main_file ?? null);
  const fileName = $derived(file ? file.split("/").pop()! : null);
</script>

<article class="card" style:--i={index}>
  <div class="head">
    <Led status="working" title="in Arbeit / ruhend" />
    <h2 class="label name">{baustein.name}</h2>
  </div>

  <div class="body">
    {#if fileName}
      <div class="mono filename" title={file ?? ""}>{fileName}</div>
      <div class="mono path">{file}</div>
    {:else}
      <div class="mono path empty">{baustein.path}</div>
    {/if}
  </div>
</article>

<style>
  .card {
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 14px 15px 15px;
    display: flex;
    flex-direction: column;
    gap: 11px;
    transition:
      border-color var(--dur) var(--ease),
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
    /* staggered reveal on open */
    animation: rise 360ms var(--ease) backwards;
    animation-delay: calc(var(--i) * 35ms);
  }
  .card:hover {
    border-color: var(--key-mid);
    transform: translateY(-1px);
    box-shadow: 0 2px 0 rgba(28, 26, 25, 0.04);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding-bottom: 11px;
    border-bottom: 1px solid var(--hairline);
  }
  .name {
    margin: 0;
    color: var(--ink-strong);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .filename {
    color: var(--ink-default);
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    color: var(--ink-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path.empty {
    font-size: 12px;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
