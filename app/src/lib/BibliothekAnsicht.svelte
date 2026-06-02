<script lang="ts">
  // Die Bibliothek-Ansicht (Issue #108, Slice 1): eine app-weite, produkt-unabhängige Schau
  // der vorhandenen Bausteine (ADR 0003 — lebt AUSSERHALB jedes Produkts). Read-only in dieser
  // Stufe: die Karten zeigen den Baustein wie auf der Werkbank (Name, Heimat, Muster-/Aufgaben-
  // Zähler, ein Glob-Auszug), wirken klickbar, tun aber noch nichts — Bearbeiten/Anlegen/Löschen
  // kommen in späteren Stufen. Quelle ist das bestehende `cmd.listBibliothek`; kein neues Kommando.
  //
  // Bewusst KEIN herkunft-Badge (mitgeliefert/eigen) — der gehört in Slice 5 (server-autoritativ).
  import { onMount } from "svelte";
  import { cmd } from "$lib/commands";
  import type { Baustein } from "$lib/types";

  let {
    onClose,
  }: {
    /** Zurück zur normalen Werkbank-Bühne. */
    onClose: () => void;
  } = $props();

  let bausteine = $state<Baustein[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(load);

  async function load() {
    loading = true;
    error = null;
    try {
      const view = await cmd.listBibliothek();
      bausteine = view.bausteine;
    } catch (e) {
      // Eine Lese-Hiccup darf die Schau nicht sprengen (Haus-Stil: degradieren, nie krachen).
      error = String(e);
      bausteine = [];
    } finally {
      loading = false;
    }
  }

  // TODO (Slice 2+): Karte/„+ Neuer Baustein" öffnen den Voll-Editor (cmd.saveBausteinCmd).
  // In dieser Stufe sind beide bewusst no-ops, damit die Karten schon klickbar wirken.
  function openBaustein(_b: Baustein) {
    // no-op bis Slice 2 die Bearbeitung verdrahtet.
  }
  function createBaustein() {
    // no-op bis Slice 2 das Anlegen verdrahtet.
  }
</script>

<section class="bibliothek">
  <header class="bhead">
    <div class="btitle">
      <span class="label sk">Magazin</span>
      <h1 class="bh">Bibliothek</h1>
    </div>
    <button class="back" onclick={onClose}>
      <span class="label">← zur Werkbank</span>
    </button>
  </header>

  <div class="bbody">
    {#if loading}
      <p class="notice mono">lädt …</p>
    {:else if error}
      <p class="notice mono">{error}</p>
    {:else}
      <div class="toolbar">
        <span class="label sk"
          >{bausteine.length.toString().padStart(2, "0")} Bausteine in der Bibliothek</span
        >
      </div>

      <div class="gallery">
        <!-- Ghost-Kachel „+ Neuer Baustein": klickbar, aber noch ohne Wirkung (Slice 2). -->
        <button class="card ghostcard" onclick={createBaustein}>
          <span class="plus" aria-hidden="true">+</span>
          <span class="label">Neuer Baustein</span>
        </button>

        {#each bausteine as b (b.id)}
          <!-- Karte spiegelt den Werkbank-Karten-Look. Klickbar (Slice 2 verdrahtet Bearbeiten). -->
          <button
            class="card"
            class:retired={b.stillgelegt}
            onclick={() => openBaustein(b)}
          >
            <div class="ctop">
              <span class="cname">{b.name}</span>
            </div>
            <span class="cid mono"
              >{b.id}{#if b.stillgelegt} · stillgelegt{/if}</span
            >

            <div class="cstats">
              <span class="stat"
                ><span class="sval mono">{b.heimat}</span
                ><span class="slab label">Heimat</span></span
              >
              <span class="stat"
                ><span class="sval mono">{b.globs.length}</span
                ><span class="slab label">Muster</span></span
              >
              <span class="stat"
                ><span class="sval mono">{(b.startaufgaben ?? []).length}</span
                ><span class="slab label">Aufgaben</span></span
              >
            </div>

            <div class="globpeek">
              {#each b.globs.slice(0, 4) as g (g)}
                <span class="gp mono">{g}</span>
              {/each}
              {#if b.globs.length > 4}
                <span class="gp more mono">+{b.globs.length - 4}</span>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</section>

<style>
  .bibliothek {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background-color: var(--surface-base);
    /* warm grain, mirroring the Werkbank work area so the Bibliothek reads as the same instrument */
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.025'/%3E%3C/svg%3E");
  }

  .bhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--hairline);
  }
  .btitle {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sk {
    color: var(--ink-muted);
    font-size: 10px;
  }
  .bh {
    margin: 0;
    font-size: 19px;
    font-weight: 700;
    color: var(--ink-strong);
    letter-spacing: -0.01em;
  }
  .back {
    appearance: none;
    cursor: pointer;
    flex: none;
    background: transparent;
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 8px 14px;
    color: var(--ink-default);
    transition:
      background var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .back:hover {
    background: var(--surface-raised);
    border-color: var(--ink-muted);
  }
  .back .label {
    color: inherit;
  }

  .bbody {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 18px 16px 28px;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 2px 14px;
  }

  .notice {
    color: var(--ink-muted);
    font-size: 13px;
  }

  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(248px, 1fr));
    gap: 12px;
  }

  /* Card mirrors the Werkbank artifact card: raised surface, hairline, seated highlight. As a
     button it lifts subtly on hover so it reads as clickable, even though Slice 1 is read-only. */
  .card {
    appearance: none;
    cursor: pointer;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.5) inset;
    transition:
      border-color var(--dur) var(--ease),
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
  }
  .card:hover {
    border-color: var(--ink-muted);
    transform: translateY(-1px);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.5) inset,
      0 2px 6px -3px rgba(8, 7, 6, 0.35);
  }
  .card:active {
    transform: translateY(0);
  }
  .card.retired {
    opacity: 0.6;
  }

  .ghostcard {
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 160px;
    background: transparent;
    border: 1px dashed var(--hairline);
    box-shadow: none;
    color: var(--ink-muted);
  }
  .ghostcard:hover {
    color: var(--ink-strong);
    border-color: var(--ink-muted);
    transform: none;
    box-shadow: none;
  }
  .ghostcard .plus {
    font-size: 26px;
  }
  .ghostcard .label {
    color: inherit;
  }

  .ctop {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }
  .cname {
    font-size: 15px;
    font-weight: 700;
    color: var(--ink-strong);
    letter-spacing: -0.01em;
  }
  .cid {
    font-size: 11px;
    color: var(--ink-muted);
  }
  .cstats {
    display: flex;
    gap: 16px;
    padding: 8px 0;
    border-top: 1px solid var(--hairline);
    border-bottom: 1px solid var(--hairline);
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sval {
    font-size: 13px;
    color: var(--ink-strong);
  }
  .slab {
    font-size: 8.5px;
    color: var(--ink-muted);
  }
  .globpeek {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-height: 20px;
  }
  .gp {
    font-size: 10px;
    color: var(--ink-default);
    padding: 1px 6px;
    background: var(--surface-sunken);
    border-radius: 99px;
  }
  .gp.more {
    color: var(--ink-muted);
    background: transparent;
  }
</style>
