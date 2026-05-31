<script lang="ts">
  // Issue #47 — Unzugeordnet-Fach pro Arbeitsbereich. A Waise is a tracked file that simply
  // lacks a label; nothing is lost by omission, the folder context is the assignment hint.
  // This drawer stays deliberately quiet — routine, recessed, collapsible — so unlabeled files
  // are visible without competing with the Artefakt-Karten. Hand-assignment is only a correction.
  import type { UnzugeordnetFach } from "./types";

  let {
    fach,
    // Open the Waise file via the OS default program (one click, like the cards).
    onOpen = (_file: string) => {},
    // Hand-assignment is ONLY a correction (Issue #47): quiet, hidden-until-asked.
    onAssign = undefined,
  }: {
    fach: UnzugeordnetFach;
    onOpen?: (file: string) => void;
    onAssign?: ((file: string) => void) | undefined;
  } = $props();

  // Default collapsed: the drawer announces its count, the files open on demand.
  let open = $state(false);
  const title = $derived(fach.arbeitsbereich || "Produktwurzel");
  const count = $derived(fach.dateien.length);
  const fileName = (p: string) => p.split("/").pop() ?? p;
</script>

<section class="fach" class:open>
  <button class="bar" onclick={() => (open = !open)} aria-expanded={open}>
    <span class="chevron mono" aria-hidden="true">{open ? "▾" : "▸"}</span>
    <span class="label heading">Unzugeordnet</span>
    <span class="mono area">{title}</span>
    <span class="mono tally">{count.toString().padStart(2, "0")}</span>
  </button>

  {#if open}
    <ul class="list">
      {#each fach.dateien as f (f)}
        <li class="row">
          <button class="file" onclick={() => onOpen(f)} title={`${f} öffnen`}>
            <span class="mono fname">{fileName(f)}</span>
            <span class="mono fpath">{f}</span>
          </button>
          {#if onAssign}
            <button class="assign label" onclick={() => onAssign?.(f)} title="Diesem Artefakt zuordnen">
              zuordnen …
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .fach {
    background: var(--surface-base);
    border: 1px dashed var(--hairline);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .fach.open {
    border-style: solid;
  }

  .bar {
    appearance: none;
    cursor: pointer;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    background: none;
    border: none;
    padding: 9px 12px;
    text-align: left;
    transition: background var(--dur) var(--ease);
  }
  .bar:hover {
    background: var(--surface-sunken);
  }
  .chevron {
    flex: none;
    font-size: 10px;
    color: var(--ink-muted);
  }
  .heading {
    color: var(--ink-muted);
    font-size: 10px;
    flex: none;
  }
  .area {
    flex: 1;
    min-width: 0;
    color: var(--ink-default);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tally {
    flex: none;
    font-size: 10px;
    color: var(--ink-muted);
    background: var(--surface-sunken);
    border-radius: 99px;
    padding: 1px 7px;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 4px 8px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: drop 200ms var(--ease) backwards;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: var(--radius-sm);
    padding: 2px 4px;
  }
  .row:hover {
    background: var(--surface-raised);
  }

  /* The whole file line opens it — a quiet target, no key chrome (Waisen stay recessive). */
  .file {
    appearance: none;
    cursor: pointer;
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: 3px 2px;
    display: flex;
    align-items: baseline;
    gap: 9px;
    text-align: left;
  }
  .fname {
    flex: none;
    color: var(--ink-default);
    font-size: 12px;
  }
  .fpath {
    flex: 1;
    min-width: 0;
    color: var(--ink-muted);
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file:hover .fname {
    color: var(--ink-strong);
  }

  .assign {
    appearance: none;
    cursor: pointer;
    flex: none;
    background: none;
    border: none;
    color: var(--ink-muted);
    font-size: 9px;
    padding: 0 2px;
    transition: color var(--dur) var(--ease);
  }
  .assign:hover {
    color: var(--ink-default);
  }

  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
