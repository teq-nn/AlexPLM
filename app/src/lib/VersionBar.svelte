<script lang="ts">
  import type { RevisionArt, ProductView } from "./types";

  let {
    product,
    activeRevision = null,
    activeRevisionArt = null,
  }: {
    product: ProductView | null;
    activeRevision?: string | null;
    activeRevisionArt?: RevisionArt | null;
  } = $props();

  // The version bar's largest, brightest element is the active Revision (E28/§24):
  // the durable human version. Until a Stand is promoted there is none — say so honestly
  // rather than invent a number.
  let version = $derived(activeRevision ?? null);
  // The Art rides next to the version (E42): a released Revision reads as a calm,
  // muted "Freigabe · schreibgeschützt" — never orange (the toggle is a considered act,
  // not the laute Ausnahme). A Prototyp is the lax, quiet default.
  let isFreigabe = $derived(activeRevisionArt === "freigabe");
</script>

<header class="bar">
  <div class="screen">
    <div class="crumbs mono">
      {#if product}
        <span class="product">{product.name}</span>
        <span class="sep">·</span>
        <span class="branch">{product.branch}</span>
        <span class="sep">·</span>
        {#if version}
          <span class="version">{version}</span>
          {#if activeRevisionArt}
            <span
              class="art label"
              class:freigabe={isFreigabe}
              title={isFreigabe
                ? "Freigabe — schreibgeschützt"
                : "Prototyp — lax"}
            >
              {#if isFreigabe}
                <span class="lock" aria-hidden="true"></span>Freigabe
              {:else}
                Prototyp
              {/if}
            </span>
          {/if}
        {:else}
          <span class="version none">— keine Revision —</span>
        {/if}
      {:else}
        <span class="idle">kein Produkt geöffnet</span>
      {/if}
    </div>

    <div class="right label">
      {#if product}
        <span class="count mono"
          >{product.bausteine.length.toString().padStart(2, "0")}</span
        >
        <span class="count-label">Bausteine</span>
        <span class="divider"></span>
      {/if}
      <span class="view">Ansicht</span>
    </div>
  </div>
</header>

<style>
  .bar {
    background: var(--screen-bg);
    padding: 14px 16px 16px;
  }

  /* Recessed LCD: inset shadow + faint scanline texture for instrument feel. */
  .screen {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 13px 16px;
    border-radius: var(--radius);
    background:
      linear-gradient(180deg, #131110, #0b0a09);
    box-shadow:
      inset 0 1px 2px rgba(0, 0, 0, 0.9),
      inset 0 0 0 1px rgba(255, 255, 255, 0.03),
      0 0.5px 0 rgba(255, 255, 255, 0.04);
    overflow: hidden;
  }
  .screen::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: repeating-linear-gradient(
      0deg,
      rgba(255, 255, 255, 0.018) 0px,
      rgba(255, 255, 255, 0.018) 1px,
      transparent 1px,
      transparent 3px
    );
    mix-blend-mode: screen;
  }

  .crumbs {
    color: var(--screen-fg);
    font-size: 15px;
    letter-spacing: 0.01em;
    display: flex;
    align-items: baseline;
    gap: 9px;
    min-width: 0;
  }
  .product {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sep {
    color: #4a4641;
  }
  .branch {
    color: #b8b4ad;
  }
  /* The active Revision: the largest, brightest element — a hint of 7-segment display. */
  .version {
    color: var(--screen-fg);
    font-weight: 700;
    font-size: 18px;
    letter-spacing: 0.02em;
    line-height: 1;
    text-shadow: 0 0 8px rgba(232, 230, 225, 0.22);
  }
  .version.none {
    color: #5a564f;
    font-size: 13px;
    font-weight: 500;
    letter-spacing: 0;
    text-shadow: none;
  }

  /* Revision-Art chip (E42): a small recessed caps tag next to the version. Prototyp is
     the quiet, lax default (dim grey). Freigabe reads brighter + a tiny lock glyph — the
     calm "schreibgeschützt" signal, NOT orange (the toggle is a considered act, never the
     laute Ausnahme). */
  .art {
    align-self: center;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 9px;
    color: #7a766f;
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
  }
  .art.freigabe {
    color: var(--screen-fg);
    background: rgba(232, 230, 225, 0.08);
    box-shadow: inset 0 0 0 1px rgba(232, 230, 225, 0.22);
  }
  /* A tiny padlock drawn in CSS — a shackle arc over a body — so the write-protect reads
     instantly without a glyph font. */
  .lock {
    position: relative;
    width: 8px;
    height: 9px;
    flex: none;
  }
  .lock::before {
    content: "";
    position: absolute;
    left: 1px;
    top: 3px;
    width: 6px;
    height: 5px;
    border-radius: 1px;
    background: currentColor;
  }
  .lock::after {
    content: "";
    position: absolute;
    left: 2px;
    top: 0;
    width: 4px;
    height: 5px;
    border: 1.2px solid currentColor;
    border-bottom: 0;
    border-radius: 3px 3px 0 0;
  }
  .idle {
    color: #6b6660;
    font-size: 13px;
  }

  .right {
    display: flex;
    align-items: center;
    gap: 9px;
    color: #6b6660;
    flex: none;
  }
  .count {
    color: var(--screen-fg);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0;
  }
  .count-label {
    color: #6b6660;
  }
  .divider {
    width: 1px;
    height: 12px;
    background: #322e2a;
  }
  .view {
    color: #6b6660;
  }
</style>
