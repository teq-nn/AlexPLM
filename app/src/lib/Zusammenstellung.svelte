<script lang="ts">
  import type { ZusammenstellungsBericht, ChecklistenPosten, PostenZustand } from "./types";

  // Die Produkt-Zusammenstellung als **Checkliste** (Issue #140, E52a). Eine Produkt-Revision ist
  // erst vollständig, wenn JEDER verpflichtende Baustein einen Beitrag trägt — einen frischen
  // Freigabe-Stand ODER das bewusste „Vorstand mitnehmen". Optionale Bausteine blockieren NIE.
  // Die Liste liest sich „elektronik ✓ Rev B · firmware ⧖ ausstehend": pro Bereich ein Häkchen
  // (beigetragen / Vorstand mitgenommen), eine Sanduhr (ausstehend) oder ein ruhiges „nicht dabei"
  // (optional & offen). Es gibt KEINE Rollen/Rechte — die Checkliste ist eine geteilte Sicht auf
  // den Reifestand, kein Freigabe-Gate für Personen.
  let { bericht }: { bericht: ZusammenstellungsBericht } = $props();

  // Das menschliche „Rev B" aus dem dauerhaften Release-Tag (`freigabe/<heimat>/<label>`): das
  // letzte Pfad-Segment ist das Freigabe-Label. Kein git-Vokabular nach außen — nur das Label.
  function standLabel(tag: string): string {
    if (!tag) return "";
    const teile = tag.split("/").filter((t) => t.length > 0);
    return teile[teile.length - 1] ?? tag;
  }

  // Das Zeichen vor jeder Zeile: ✓ für einen Beitrag, ⧖ für einen ausstehenden Pflicht-Bereich,
  // – für einen optionalen, der nicht dabei ist.
  function zeichen(z: PostenZustand): string {
    switch (z) {
      case "beigetragen":
      case "vorstand-mitgenommen":
        return "✓";
      case "ausstehend":
        return "⧖";
      case "optional-offen":
        return "–";
    }
  }

  // Die kurze Zustands-Worte rechts der Zeile (deutsch, Werkstatt — kein git-Wort).
  function zustandWort(p: ChecklistenPosten): string {
    switch (p.zustand) {
      case "beigetragen":
        return standLabel(p.release_tag);
      case "vorstand-mitgenommen":
        return `${standLabel(p.release_tag)} · Vorstand`;
      case "ausstehend":
        return "ausstehend";
      case "optional-offen":
        return "nicht dabei";
    }
  }

  function ledKlasse(z: PostenZustand): string {
    switch (z) {
      case "beigetragen":
      case "vorstand-mitgenommen":
        return "ok";
      case "ausstehend":
        return "working";
      case "optional-offen":
        return "off";
    }
  }
</script>

<section class="zus" aria-labelledby="zus-title">
  <header class="head">
    <span class="label kicker">Zusammenstellung</span>
    <h2 id="zus-title" class="title">
      {#if bericht.vollstaendig}
        Vollständig
      {:else}
        Noch unvollständig
      {/if}
    </h2>
  </header>

  <div class="body">
    {#if bericht.posten.length === 0}
      <p class="lede">
        Noch keine Bausteine im Produkt — es gibt nichts zusammenzustellen.
      </p>
    {:else}
      <p class="lede">
        {#if bericht.vollstaendig}
          Jeder verpflichtende Bereich trägt einen Stand bei. Diese Revision kann
          <strong>zusammengestellt</strong> werden.
        {:else}
          Es fehlt noch ein Beitrag in
          <strong>{bericht.ausstehende.join(", ")}</strong> — ein frischer Stand oder
          „alter Stand reicht".
        {/if}
      </p>

      <!-- Die Checkliste, ein Posten je Baustein, in Stack-Reihenfolge. -->
      <ol class="liste">
        {#each bericht.posten as p (p.heimat)}
          <li class="posten" class:ausstehend={p.zustand === "ausstehend"}>
            <span class="zeichen" class:dim={p.zustand === "optional-offen"} aria-hidden="true"
              >{zeichen(p.zustand)}</span
            >
            <span class={`dot ${ledKlasse(p.zustand)}`} aria-hidden="true"></span>
            <div class="text">
              <div class="kopf">
                <span class="bereich">{p.heimat}</span>
                {#if !p.pflicht}
                  <span class="label tag-optional">optional</span>
                {/if}
              </div>
              <span class="zustand mono" class:warte={p.zustand === "ausstehend"}>
                {zustandWort(p)}
              </span>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </div>
</section>

<style>
  .zus {
    display: flex;
    flex-direction: column;
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .head {
    padding: 16px 18px 6px;
    flex: none;
  }
  .kicker {
    color: var(--ink-muted);
    display: block;
    margin-bottom: 5px;
  }
  .title {
    margin: 0;
    font-family: var(--font-label);
    font-weight: 700;
    font-size: 19px;
    letter-spacing: -0.01em;
    color: var(--ink-strong);
  }

  .body {
    padding: 8px 18px 16px;
  }
  .lede {
    margin: 0 0 14px;
    color: var(--ink-default);
    font-size: 13px;
    line-height: 1.5;
  }
  .lede strong {
    color: var(--ink-strong);
  }

  /* Die Checkliste — je Zeile ein Zeichen + LED-Punkt + Bereich/Zustand. Routine = grau. */
  .liste {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .posten {
    display: grid;
    grid-template-columns: 16px 8px 1fr;
    gap: 10px;
    align-items: center;
    padding: 9px 11px;
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    border: 1px solid transparent;
  }
  /* Ein ausstehender Pflicht-Bereich ist der einzige, der Aufmerksamkeit zieht — eine ruhige
     Hairline, kein lautes Orange (das bleibt der seltenen Ausnahme vorbehalten). */
  .posten.ausstehend {
    background: var(--surface-base);
    border-color: var(--hairline);
  }

  .zeichen {
    justify-self: center;
    font-size: 14px;
    color: var(--ink-strong);
    line-height: 1;
  }
  .zeichen.dim {
    color: var(--ink-muted);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }
  .dot.ok {
    background: var(--led-working);
  }
  .dot.working {
    background: var(--led-attention);
  }
  .dot.off {
    background: var(--led-off);
  }

  .text {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    min-width: 0;
  }
  .kopf {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .bereich {
    color: var(--ink-default);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag-optional {
    color: var(--ink-muted);
    font-size: 9.5px;
  }
  .zustand {
    color: var(--ink-muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .zustand.warte {
    color: var(--ink-default);
  }
</style>
