<script lang="ts">
  // Die Bibliothek-Ansicht (Issue #108, Slice 1): eine app-weite, produkt-unabhängige Schau
  // der vorhandenen Bausteine (ADR 0003 — lebt AUSSERHALB jedes Produkts). Read-only in dieser
  // Stufe: die Karten zeigen den Baustein wie auf der Werkbank (Name, Heimat, Muster-/Aufgaben-
  // Zähler, ein Glob-Auszug), wirken klickbar, tun aber noch nichts — Bearbeiten/Anlegen/Löschen
  // kommen in späteren Stufen. Quelle ist das bestehende `cmd.listBibliothek`; kein neues Kommando.
  //
  // Slice 5 ergänzt: ein Herkunft-Etikett je Karte (mitgeliefert vs. eigen) und das Löschen — beide
  // server-autoritativ aus `view.bundled_ids` abgeleitet (KEINE im Frontend hartcodierte Liste). Eigene
  // Bausteine sind löschbar; mitgelieferte zeigen keine Lösch-Aktion (sie kämen beim nächsten Start
  // ohnehin per Seeding zurück — die Schranke sitzt zusätzlich hart im Backend).
  import { onMount } from "svelte";
  import { cmd } from "$lib/commands";
  import type { Baustein } from "$lib/types";
  import BausteinEditor from "$lib/bibliothek/BausteinEditor.svelte";
  import { emptyBaustein, duplicateDraft } from "$lib/bibliothek/validate";

  let {
    onClose,
  }: {
    /** Zurück zur normalen Werkbank-Bühne. */
    onClose: () => void;
  } = $props();

  let bausteine = $state<Baustein[]>([]);
  // Server-autoritative Herkunft: die Kennungen der gebündelten Defaults. Speist das Etikett
  // (mitgeliefert vs. eigen) und die Lösch-Schranke (nur eigen ist löschbar).
  let bundledIds = $state<string[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  // Welche Karte gerade die stille Löschbestätigung zeigt (`id`), und ob ein Löschen läuft.
  let confirmingDelete = $state<string | null>(null);
  let deleting = $state(false);
  let deleteError = $state<string | null>(null);

  function isBundled(id: string): boolean {
    return bundledIds.includes(id);
  }
  // Der gerade bearbeitete Baustein (Slice 2) bzw. der leere Anlege-Entwurf (Slice 3). Gesetzt ⇒ der
  // Voll-Editor übernimmt die Bühne. `creating` unterscheidet die Absicht: Anlegen prüft die Kennung
  // auf Eindeutigkeit und schreibt mit `isCreate=true` (server-autoritativ), Bearbeiten ist ein Upsert.
  let editing = $state<Baustein | null>(null);
  let creating = $state(false);

  onMount(load);

  async function load() {
    loading = true;
    error = null;
    try {
      const view = await cmd.listBibliothek();
      bausteine = view.bausteine;
      bundledIds = view.bundled_ids;
    } catch (e) {
      // Eine Lese-Hiccup darf die Schau nicht sprengen (Haus-Stil: degradieren, nie krachen).
      error = String(e);
      bausteine = [];
    } finally {
      loading = false;
    }
  }

  // Slice 2 (Bearbeiten): eine Karte öffnet den Voll-Editor, vorgefüllt mit dem gewählten Baustein.
  function openBaustein(b: Baustein) {
    creating = false;
    editing = b;
  }

  // Slice 3 (Anlegen): die Ghost-Kachel öffnet DENSELBEN Voll-Editor im Anlege-Modus mit einem leeren
  // Entwurf. Anlegen == Klon-aus-leer — gleicher Editor, gleicher Schreibpfad (Upsert). Die Kennung
  // ist editierbar + wird aus dem Namen abgeleitet; `version`=1 und `stillgelegt`=false stehen bereits
  // im leeren Entwurf (emptyBaustein).
  function createBaustein() {
    creating = true;
    editing = emptyBaustein();
  }

  // Slice 4 (Duplizieren): eine Karten-Aktion öffnet DENSELBEN Anlege-Pfad, aber vorgefüllt aus dem
  // bestehenden Baustein — `name` + „ (Kopie)", `id` neu abgeleitet, `version`=1, `stillgelegt`=false,
  // alles andere wortwörtlich kopiert (duplicateDraft). Es ist exakt der Anlege-Schreibpfad
  // (creating=true ⇒ isCreate=true), KEIN neues Kommando.
  function duplicateBaustein(b: Baustein) {
    creating = true;
    // $state.snapshot entkoppelt den Proxy hier (nur in .svelte verfügbar); duplicateDraft klont dann
    // das reine Objekt — so wirft das reine .ts-Modul nicht über eine fehlende $state-Rune.
    editing = duplicateDraft($state.snapshot(b));
  }

  // Speichern (Slice 2 + 3): Upsert über cmd.saveBausteinCmd. `isCreate` trägt die Absicht zum
  // server-autoritativen Kern: beim Anlegen wird die Kennung auf Eindeutigkeit geprüft. Bei Erfolg die
  // Galerie aus der zurückgegebenen Wahrheit neu rendern und zurück zur Galerie. Fehler wirft das
  // Kommando — der Editor fängt sie und zeigt sie in seiner Fußleiste an (kein eigenes Try/Catch hier).
  async function saveBaustein(b: Baustein) {
    const view = await cmd.saveBausteinCmd(b, creating);
    bausteine = view.bausteine;
    bundledIds = view.bundled_ids;
    editing = null;
    creating = false;
  }

  // Slice 5 (Löschen): nur für eigene (nicht gebündelte) Bausteine. Stiller zweistufiger Bestätiger
  // direkt auf der Karte (kein Native-Dialog, keine Git-/Technik-Vokabel) — „Löschen" ⇒ „Wirklich
  // entfernen?" mit Bestätigen/Abbrechen. Bei Erfolg die Galerie aus der zurückgegebenen Wahrheit neu
  // rendern. Das Backend lehnt einen gebündelten Löschwunsch zusätzlich hart ab (server-autoritativ).
  function askDelete(id: string) {
    deleteError = null;
    confirmingDelete = id;
  }
  function cancelDelete() {
    confirmingDelete = null;
    deleteError = null;
  }
  async function confirmDelete(id: string) {
    deleting = true;
    deleteError = null;
    try {
      const view = await cmd.deleteBausteinCmd(id);
      bausteine = view.bausteine;
      bundledIds = view.bundled_ids;
      confirmingDelete = null;
    } catch (e) {
      deleteError = String(e);
    } finally {
      deleting = false;
    }
  }
</script>

<section class="bibliothek">
  {#if editing}
    <!-- Voll-Flächen-Editor (Slice 2): der Editor trägt eigene Kopf-/Fußleiste samt „‹ Bibliothek". -->
    <div class="bbody editing">
      <BausteinEditor
        baustein={editing}
        {bausteine}
        create={creating}
        onSave={saveBaustein}
        onCancel={() => {
          editing = null;
          creating = false;
        }}
      />
    </div>
  {:else}
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
          <!-- Karte spiegelt den Werkbank-Karten-Look. Primärklick öffnet Bearbeiten (Slice 2); die
               „Duplizieren"-Aktion in der Ecke öffnet den Anlege-Pfad vorgefüllt (Slice 4). Beide als
               eigene Buttons in einem Wrapper — verschachtelte Buttons sind ungültiges HTML. -->
          <div
            class="cardwrap"
            class:retired={b.stillgelegt}
            class:confirming={confirmingDelete === b.id}
          >
            <button class="card" onclick={() => openBaustein(b)}>
              <div class="ctop">
                <span class="cname">{b.name}</span>
                <!-- Herkunft-Etikett (server-autoritativ): LED-Punkt + Kapitälchen-Sublabel, ruhig.
                     Kein Orange — beide Herkünfte sind Normalzustand, keine laute Ausnahme. -->
                <span
                  class="herkunft"
                  class:eigen={!isBundled(b.id)}
                  title={isBundled(b.id)
                    ? "Mitgeliefert — Teil der Standard-Bibliothek"
                    : "Eigen — von Hand angelegt"}
                >
                  <span class="hdot" aria-hidden="true"></span>
                  <span class="label">{isBundled(b.id) ? "mitgeliefert" : "eigen"}</span>
                </span>
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
            <!-- Hover-Aktionen unten rechts: Duplizieren (für jeden — auch ein mitgelieferter taugt als
                 Vorlage) und Löschen (nur eigene; Mitgelieferte kämen per Seeding zurück, Schranke auch
                 hart im Backend). Auf einem leichten Backing, damit die Texte über dem Glob-Auszug
                 lesbar bleiben. Erst beim Überfahren sichtbar, damit der Primärklick (Bearbeiten) die
                 Karte dominiert — die obere Ecke bleibt dem Herkunft-Etikett. Beim Bestätigen tritt die
                 zweistufige Löschabfrage an ihre Stelle. -->
            {#if confirmingDelete === b.id}
              <div class="delconfirm">
                {#if deleteError}
                  <span class="delerr mono">{deleteError}</span>
                {/if}
                <span class="delask label">Wirklich entfernen?</span>
                <button
                  class="delyes"
                  onclick={() => confirmDelete(b.id)}
                  disabled={deleting}
                >
                  <span class="label">{deleting ? "entfernt …" : "Entfernen"}</span>
                </button>
                <button class="delno" onclick={cancelDelete} disabled={deleting}>
                  <span class="label">Abbrechen</span>
                </button>
              </div>
            {:else}
              <div class="cardactions">
                <button
                  class="act"
                  onclick={() => duplicateBaustein(b)}
                  title="Diesen Baustein als Vorlage für einen neuen kopieren"
                >
                  <span class="label">Duplizieren</span>
                </button>
                {#if !isBundled(b.id)}
                  <button
                    class="act"
                    onclick={() => askDelete(b.id)}
                    title="Diesen eigenen Baustein entfernen"
                  >
                    <span class="label">Löschen</span>
                  </button>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
  {/if}
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
  /* Im Editor füllt die Voll-Flächen-Komponente die Bühne (eigene Kopf-/Fußleiste, eigenes Scroll). */
  .bbody.editing {
    overflow: hidden;
    padding: 16px;
    display: flex;
    flex-direction: column;
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

  /* Wrapper trägt die Karte plus die unaufdringliche „Duplizieren"-Aktion in der Ecke. Verschachtelte
     Buttons sind ungültiges HTML, daher liegen beide nebeneinander im Wrapper (Aktion absolut gesetzt). */
  .cardwrap {
    position: relative;
    display: flex;
    min-width: 0;
  }
  .cardwrap.retired {
    opacity: 0.6;
  }

  /* Card mirrors the Werkbank artifact card: raised surface, hairline, seated highlight. As a
     button it lifts subtly on hover so it reads as clickable. */
  .card {
    appearance: none;
    cursor: pointer;
    text-align: left;
    flex: 1;
    min-width: 0;
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

  /* Hover-Aktionsleiste (Duplizieren · Löschen): ein eigener Streifen am unteren Kartenrand. Statt
     sich auf seinen deckenden Grund zu verlassen (der Glob-Auszug lugte je nach Umbruch darüber
     hervor — Text über Text), blendet beim Überfahren der Glob-Auszug aus (siehe .globpeek) und die
     Leiste tritt in den dann freien Streifen. Die Zeilenhöhe bleibt reserviert (min-height), daher
     springt die Karte nicht. Erst beim Überfahren sichtbar, damit der Primärklick (Bearbeiten)
     dominiert; die obere Ecke bleibt dem Herkunft-Etikett. */
  .cardactions {
    position: absolute;
    bottom: 8px;
    right: 10px;
    left: 10px;
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    padding: 6px 10px;
    background: var(--surface-raised);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    box-shadow: 0 1px 3px -2px rgba(8, 7, 6, 0.4);
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .cardwrap:hover .cardactions,
  .cardactions:focus-within {
    opacity: 1;
  }
  .act {
    appearance: none;
    cursor: pointer;
    background: transparent;
    border: 0;
    padding: 2px;
    color: var(--ink-muted);
    transition: color var(--dur) var(--ease);
  }
  .act:hover {
    color: var(--ink-strong);
  }
  .act .label {
    color: inherit;
    font-size: 9.5px;
  }

  /* Herkunft-Etikett: ruhiger LED-Punkt + Kapitälchen, im Karten-Kopf neben dem Namen. „mitgeliefert"
     ruht gedämpft (off-LED), „eigen" trägt den grünen Frei-Punkt — orange bleibt der lauten Ausnahme
     vorbehalten, beide Herkünfte sind Normalzustand. */
  .herkunft {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: none;
    margin-top: 2px;
  }
  .herkunft .hdot {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--led-off);
  }
  .herkunft.eigen .hdot {
    background: var(--led-free);
  }
  .herkunft .label {
    font-size: 8.5px;
    color: var(--ink-muted);
  }

  /* Bestätigung: kurz, ruhig, deutsch — keine Git-/Technik-Vokabel. Liegt über der unteren Karten-
     kante, damit sie die Karte nicht aufbläht. Bleibt sichtbar (nicht nur on-hover), solange offen. */
  .delconfirm {
    position: absolute;
    bottom: 8px;
    right: 10px;
    left: 10px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 8px;
    background: var(--surface-sunken);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
  }
  .delask {
    margin-right: auto;
    font-size: 9.5px;
    color: var(--ink-default);
  }
  .delerr {
    flex-basis: 100%;
    font-size: 10px;
    color: var(--accent);
  }
  .delyes,
  .delno {
    appearance: none;
    cursor: pointer;
    background: transparent;
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 3px 8px;
    color: var(--ink-default);
    transition:
      border-color var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .delyes:hover:not(:disabled) {
    border-color: var(--ink-strong);
    color: var(--ink-strong);
  }
  .delno:hover:not(:disabled) {
    border-color: var(--ink-muted);
  }
  .delyes:disabled,
  .delno:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .delyes .label,
  .delno .label {
    color: inherit;
    font-size: 9.5px;
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
    transition: opacity var(--dur) var(--ease);
  }
  /* Beim Überfahren (Aktionsleiste tritt auf) oder während der Lösch-Bestätigung weicht der Glob-
     Auszug, damit die Leiste nie mit den Mustern kollidiert. Die reservierte Höhe bleibt, also
     bleibt das Karten-Layout ruhig. */
  .cardwrap:hover .globpeek,
  .cardwrap.confirming .globpeek {
    opacity: 0;
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
