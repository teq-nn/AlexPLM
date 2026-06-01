// Domain view returned by the read-only `open_product` Tauri command.
// Mirrors `ProductView` / `Baustein` in src-tauri/src/projection.rs.

export interface Baustein {
  /** Folder name; rendered as a caps label. */
  name: string;
  /** Folder path relative to the product root (muted Mono). */
  path: string;
  /** Representative file relative to the product root, if any. */
  main_file: string | null;
}

export interface ProductView {
  name: string;
  branch: string;
  version: string;
  bausteine: Baustein[];
}

/** A settled save, surfaced as a new Stand. Payload of the `stand-created` event.
 *  Mirrors `Stand` in src-tauri/src/autocommit.rs. No git vocabulary. */
export interface StandEvent {
  /** Product-relative path that settled (forward slashes). */
  path: string;
  /** Machine timestamp, `YYYY-MM-DDTHH:MM:SSZ`. */
  timestamp: string;
}

/** A Stand as held in the UI list: the event payload plus a stable client-side key
 *  so repeated saves of the same path at the same second remain distinct rows. */
export interface Stand extends StandEvent {
  id: number;
}

/** The Art (kind) of a Revision (Issue #41, E42). A new Revision is "prototyp" (lax)
 *  by default; the toggle raises it to "freigabe" (streng + write-protected / schreibgeschützt),
 *  and toggling back is a deliberate reversible "Un-Release". Mirrors `RevisionArt` in
 *  src-tauri/src/graph.rs (serde kebab-case). */
export type RevisionArt = "prototyp" | "freigabe";

/** A node in the dark "display" version tree (Issue #8).
 *  Mirrors `StandNode` in src-tauri/src/graph.rs. */
export interface StandNode {
  /** Stable id (commit hash); the UI keys rows on this, never shown as git. */
  id: string;
  /** Machine timestamp `YYYY-MM-DDTHH:MM:SSZ`. */
  timestamp: string;
  /** Product-relative path recovered from the boring auto message; "." otherwise. */
  path: string;
  /** Human version label if this Stand was promoted to a Revision, else null. */
  revision: string | null;
  /** The Revision-Art (Prototyp/Freigabe — E42); null for a plain Stand. (Issue #41) */
  revision_art: RevisionArt | null;
  /** Whether VERSION_NOTES.md text exists for this Revision. */
  has_notes: boolean;
  /** Whether this node's binary content was offloaded to a cold archive (E36). */
  offloaded: boolean;
  /** Whether this Stand reached the shared line — on `origin/<shared>` as far as this machine
   *  knows (E47, #30). The Versionsbaum marks it „veröffentlicht". Distinct from the Freigabe-Art:
   *  a Prototyp Stand on the published line is veröffentlicht, not freigegeben. */
  veroeffentlicht: boolean;
  /** Bahn (lane) this Stand sits on: 0 is the active trunk, each diverging Zweig its own
   *  positive index. A single linear history is all lane 0. (Issue #28) */
  lane: number;
  /** Domain name of this Stand's Zweig (lane), or null for the trunk / unnamed lines.
   *  The UI shows it once per lane (at the lane's tip). (Issue #28) */
  branch: string | null;
  /** Whether this Stand lies on the active line — it stays clearly marked. (Issue #28) */
  on_active: boolean;
  /** The Stände this one „folgt auf" (direct predecessors): one normally, two where two
   *  Linien were „zusammengeführt". The UI draws a connector to each predecessor in the tree,
   *  making forks and Zusammenführungen visible. Ids only, never shown as git. (Issue #28) */
  parents: string[];
}

/** The version tree + active revision the version bar shows in Mono (Issue #8 / #28).
 *  Mirrors `VersionGraph` in src-tauri/src/graph.rs. */
export interface VersionGraph {
  nodes: StandNode[];
  active_revision: string | null;
  /** Art of the active Revision (Prototyp/Freigabe — E42); null if none. (Issue #41) */
  active_revision_art: RevisionArt | null;
  offloaded_archive: string | null;
  /** Name of the active line (Zweig), echoed for the UI marker; null if unknown. */
  active_branch: string | null;
  /** Number of distinct lanes; 1 for a single linear history. */
  lane_count: number;
}

/** One of the three Graph-Raum node verbs (Issue #55, E27).
 *  Mirrors `KnotenVerb` in src-tauri/src/knotenverben.rs (kebab-case). */
export type KnotenVerb =
  | "als-ordner-oeffnen"
  | "von-hier-abzweigen"
  | "zurueckwerfen";

/** The pure Graph-Raum display filter (Issue #55, E45). Hides nodes only — never rewrites.
 *  Mirrors `GraphFilter` in src-tauri/src/knotenverben.rs. */
export interface GraphFilter {
  /** Show variant lines (non-active Zweige)? Default true. */
  varianten: boolean;
  /** Show only Revisionen (promoted Stände)? Default false. */
  nur_revisionen: boolean;
}

/** Result of „Als Ordner öffnen" (Issue #55): the materialised read-only worktree path.
 *  Mirrors `GeoeffneterOrdner` in src-tauri/src/worktreeglue.rs. */
export interface GeoeffneterOrdner {
  /** Absolute path of the materialised folder, forward-slash display. */
  pfad: string;
  /** Whether the folder was freshly created (vs. already present). */
  neu: boolean;
}

/** Woher eine Kante stammt — die drei Herkunftsstufen (E20, Issue #56). Mirrors `Herkunft`
 *  in src-tauri/src/edges.rs. Reine Anzeige/Pflege; die Stale-Logik ist herkunfts-blind. */
export type KantenHerkunft = "hand" | "baustein-default" | "paar-default";

/** A „abgeleitet von" edge: `derived` „stammt aus" `source` (Issue #10/#56).
 *  Both are product-relative artifact paths. Mirrors `Edge` in src-tauri/src/edges.rs. */
export interface Edge {
  /** The derivation — made *from* `source`. */
  derived: string;
  /** The source the derivation „stammt aus". */
  source: string;
  /** Herkunftsstufe (E20); fehlt sie (Altbestand), gilt "hand". */
  herkunft?: KantenHerkunft;
}

/** Ein deterministischer Baustein-Paar-Default-Vorschlag (E20, Issue #56): per Klick zu einer
 *  echten Kante bestätigt, nie automatisch. Mirrors `KantenVorschlag` in defaultkanten.rs. */
export interface KantenVorschlag {
  derived: string;
  source: string;
  baustein_id: string;
  partner_id: string;
}

/** A fired Stale-Warnung: the derivation is older than its source (E26).
 *  Mirrors `StaleWarning` in src-tauri/src/edges.rs. */
export interface StaleWarning {
  derived: string;
  source: string;
  source_timestamp: string;
  derived_timestamp: string;
}

/** Manual edges + their Stale-Warnungen, returned in one round-trip (Issue #10).
 *  Mirrors `EdgeView` in src-tauri/src/edgestore.rs. Opt-in: zero edges = no warnings. */
export interface EdgeView {
  edges: Edge[];
  warnings: StaleWarning[];
  /** Offene Baustein-Paar-Default-Vorschläge (E20, Issue #56). */
  vorschlaege?: KantenVorschlag[];
}

// Outcome of the clean, non-destructive import (Issue #3, E38).
// Mirrors `ImportResult` in src-tauri/src/import.rs.
export interface ImportResult {
  /** True if this run ran `git init`; false if an existing repo was left as-is. */
  git_initialized: boolean;
  /** Number of leaf files marked `lockable` in `.gitattributes`. */
  locked_count: number;
  /** Read-only projection of the freshly imported product. */
  product: ProductView;
}

// The one Import Gate decision (Issue #7, E38).
// Mirrors `GateDecision` in src-tauri/src/import_gate.rs.
export type GateDecision = "clean-init" | "migrate-behind-gate" | "refuse";

// The gate's verdict for a folder plus the facts it rests on.
// Mirrors `GateReport` in src-tauri/src/import.rs.
export interface GateReport {
  decision: GateDecision;
  has_history: boolean;
  shared_clones_exist: boolean;
  giant_binaries_in_history: boolean;
}

// Auto-Lock & Status-Signale (Issue #6, E37). Derived purely from `git lfs locks` +
// worktree status — no second source of truth.

/** Derived per-artifact status. Mirrors `ArtifactStatus` in src-tauri/src/locks.rs.
 *  free → green LED, in-progress → grey, locked-by-other → orange (loud exception). */
export type ArtifactStatus = "free" | "in-progress" | "locked-by-other";

/** One artifact's LED signal. Mirrors `ArtifactSignal` in src-tauri/src/locks.rs. */
export interface ArtifactSignal {
  /** Product-relative path the signal is for. */
  path: string;
  status: ArtifactStatus;
  /** Foreign lock owner, present iff status === "locked-by-other". */
  locked_by?: string;
  /** Foreign lock timestamp, present iff status === "locked-by-other". */
  locked_at?: string;
  /** Ready tooltip "gesperrt von X seit …", present iff foreign-locked. */
  tooltip?: string;
}

/** A foreign lock for the live "Belegte Bausteine" panel. Mirrors `ForeignLock` in lib.rs. */
export interface ForeignLock {
  path: string;
  owner: string;
  locked_at: string;
  /** "gesperrt von X seit …" */
  tooltip: string;
}

// The one-time Einrichtungs-Zeremonie (Issue #5, E41). Mirrors `SetupReport` /
// `SetupStage` in src-tauri/src/setup.rs. This is the rare, explicit exception where
// git-near wording is allowed; the daily sync stays silent everywhere else.

/** Where a product stands in the one-time ceremony. Mirrors `SetupStage`. */
export type SetupStage =
  | "not-configured"
  | "remote-set-not-published"
  | "eingerichtet";

/** The ceremony state for a product. Mirrors `SetupReport` in src-tauri/src/setup.rs. */
export interface SetupReport {
  /** Drives whether the ceremony or the settled readout shows. */
  stage: SetupStage;
  /** Whether a server (remote) is connected. */
  has_remote: boolean;
  /** Whether the product has been published (first push done). */
  has_published: boolean;
  /** Credential-free clone URL to hand a colleague, once a server is connected. */
  clone_url: string | null;
}

// The app-wide Konto (ADR 0004, Issue #90): exactly ONE server identity for the self-hosted
// Forgejo/Gitea, set once and reused for all products. The Server-Adresse is persisted app-level
// (JSON), the credentials only in the OS keystore. The token NEVER leaves the backend — neither
// `read_konto` nor `save_konto` ever return it.

/** The Konto view the backend hands the frontend: the normalized Base-URL + the angemeldete
 *  account. NEVER carries the token. Mirrors `KontoView` in src-tauri/src/konto.rs. `read_konto`
 *  returns `KontoView | null` (null = kein Konto eingerichtet); `save_konto` returns `KontoView`. */
export interface KontoView {
  /** The normalized server Base-URL `scheme://host[:port]`. */
  base_url: string;
  /** The angemeldete account name (confirmed via `GET /api/v1/user` on save). */
  account: string;
}

// The Lock Warden's two push types (Issue #9, E35). The pure, safety-critical core returns
// EXACTLY ONE of these per checkpoint. The UI never speaks raw git — only the tool's own
// vocabulary (the daily sync stays silent; this is the calm "gesichert / freigegeben" readout).

/** The single action the Lock Warden decides. Mirrors `WardenAction` in src-tauri/src/warden.rs.
 *  - `freigabe-push`   → published to the shared stand + lock released ("freigegeben");
 *  - `sicherungs-push` → private backup only ("dein Stand ist gesichert");
 *  - `auto-unlock`     → a held lock on a clean path was released ("Sperre gelöst");
 *  - `refuse`          → nothing to do (surfaced as nothing). */
export type WardenAction =
  | "freigabe-push"
  | "sicherungs-push"
  | "auto-unlock"
  | "refuse";

// The stiller Sync + Sync Decider (Issue #11, E41). The daily net-sync runs SILENTLY: the user
// only ever sees "aktuell / gesichert" in the calm status readout — never push/pull/merge. The
// ONE exception is a real, unmergeable contradiction: the stiller Sync stops and asks a single
// domain-language question (the single orange-frame attention moment), never a git conflict marker.

/** Which stand the user keeps in a loud exception. Mirrors `StandChoice` in src/syncdecider.rs. */
export type StandChoice = "mine" | "theirs";

/** One choosable stand in the loud question, with a domain label (never git wording).
 *  Mirrors `StandOption` in src-tauri/src/syncdecider.rs. */
export interface StandOption {
  choice: StandChoice;
  /** e.g. "mein Stand" / "Bens Stand". */
  label: string;
}

/** The domain-language question shown in the single orange-frame loud exception. Carries NO git
 *  conflict marker by construction. Mirrors `LoudQuestion` in src-tauri/src/syncdecider.rs. */
export interface LoudQuestion {
  /** „dein und Bens Gehäuse-Stand widersprechen sich — welcher gilt?" */
  frage: string;
  /** The contested artifacts, named as artifacts (never git refs). At least one. */
  artefakte: string[];
  /** The two stands to choose between. */
  optionen: StandOption[];
}

/** The quiet daily sync status in the tool's OWN vocabulary (E41). `laute-ausnahme` is the only
 *  one that raises the voice. Mirrors `SyncStatus` in src-tauri/src/syncglue.rs — serde external
 *  tagging: the two quiet states are bare strings, the loud one carries the question. */
export type SyncStatus =
  | "aktuell"
  | "gesichert"
  | { "laute-ausnahme": LoudQuestion };

/** Outcome of one silent daily sync pass. Mirrors `SyncOutcome` in src-tauri/src/syncglue.rs. */
export interface SyncOutcome {
  status: SyncStatus;
}

/** Outcome of a publish attempt (Issue #44). Mirrors `PublishOutcome` in src-tauri/src/setup.rs —
 *  serde internal tagging (`kind`): `published` carries the refreshed ceremony state, while
 *  `laute-ausnahme` carries the same domain-language question the daily sync raises, because the
 *  chosen Server-Repo already held a contradicting unmergeable Stand. The user answers it with the
 *  SAME resolve flow as the sync, then re-publishes. Never a git marker. */
export type PublishOutcome =
  | ({ kind: "published" } & SetupReport)
  | ({ kind: "laute-ausnahme" } & LoudQuestion);

// Baustein-Modell & Bibliothek (Issue #39, ADR 0002/0003). A Baustein bundles per-tool knowledge;
// the Bibliothek is the shared template source; a Produkt-Stack is a self-contained ANTI-DRIFT
// copy in `_plm/stack.json`. Lockability is NOT a Baustein field (it lives in the classifier).

/** Öffnen-Aktion of an artifact card. Mirrors `Oeffnen` in src-tauri/src/baustein.rs.
 *  `auto` → dominant file else folder (PRD §14). */
export type Oeffnen = "auto" | "datei" | "ordner";

/** Art of a Startaufgabe: Aufgabe (mandatory, can block) vs Hinweis (never blocks) — PRD §27.
 *  Mirrors `AufgabenTyp` in src-tauri/src/baustein.rs. */
export type AufgabenTyp = "aufgabe" | "hinweis";

/** A Startaufgabe seeded when a Baustein is onboarded. Mirrors `Startaufgabe` in baustein.rs. */
export interface Startaufgabe {
  titel: string;
  typ: AufgabenTyp;
  /** Whether this hard-blocks the Freigabe-Gate. Always false for a Hinweis. */
  blockiert: boolean;
}

/** An internal Default-Kante: a derived glob „stammt aus" a source glob. Pattern-based (PRD §13).
 *  Mirrors `DefaultKante` in src-tauri/src/baustein.rs. */
export interface DefaultKante {
  derived_glob: string;
  source_glob: string;
}

/** A Baustein-Paar-Default-Kante (E20, Issue #56): "wenn Partner `partner_id` auch im Stack ist,
 *  schlage `derived_glob` ← `source_glob` vor". Mirrors `PaarDefaultKante` in baustein.rs. */
export interface PaarDefaultKante {
  partner_id: string;
  derived_glob: string;
  source_glob: string;
}

/** A reusable per-tool Baustein. Mirrors `Baustein` in src-tauri/src/baustein.rs. */
export interface Baustein {
  /** Stable kebab id, e.g. "kicad". */
  id: string;
  /** Monotone integer version; carries the version-gated seeding (ADR 0003). */
  version: number;
  name: string;
  /** Default Heimat-Ordner (Arbeitsbereich), e.g. "elektronik". */
  heimat: string;
  /** Artefakt-Globs, ORDERED: [0] is the Hauptdatei rule. */
  globs: string[];
  /** Ignore presets (marker-block lines for .gitignore). */
  ignore: string[];
  /** LFS patterns (marker-block lines for .gitattributes). */
  lfs: string[];
  oeffnen: Oeffnen;
  startaufgaben: Startaufgabe[];
  default_kanten: DefaultKante[];
  /** Paar-Default-Kanten (E20, Issue #56): Vorschläge, sobald ein Partner-Baustein im Stack liegt. */
  paar_default_kanten?: PaarDefaultKante[];
  /** Label-only stillgelegt (PRD §10). */
  stillgelegt: boolean;
}

/** A named, ordered selection of Baustein ids. Mirrors `Toolstack` in src-tauri/src/baustein.rs. */
export interface Toolstack {
  id: string;
  name: string;
  baustein_ids: string[];
}

/** The local Bibliothek as returned by `list_bibliothek`. Mirrors `BibliothekView` in lib.rs. */
export interface BibliothekView {
  bausteine: Baustein[];
  toolstacks: Toolstack[];
}

/** Provenance stamp of a copied Baustein: from which Bibliothek id + version it was copied.
 *  Display only — NO live link (ADR 0003). Mirrors `Herkunft` in src-tauri/src/stackstore.rs. */
export interface Herkunft {
  from: string;
  version: number;
}

/** A Baustein full-copy inside the Produkt-Stack: the whole definition (flattened) plus its
 *  provenance stamp. Mirrors `StackBaustein` in src-tauri/src/stackstore.rs. */
export interface StackBaustein extends Baustein {
  herkunft: Herkunft;
}

/** A product's copied Produkt-Stack in `_plm/stack.json` — the anti-drift copy (ADR 0002/0003).
 *  Self-contained; a later Bibliothek edit never reaches it. Mirrors `ProduktStack` in
 *  src-tauri/src/stackstore.rs. */
export interface ProduktStack {
  /** Optional display name of the chosen standard Toolstack. */
  toolstack?: string;
  bausteine: StackBaustein[];
}

// Pattern-Zuordnung → Artefakt-Karten + Unzugeordnet-Fach (Issue #47). Tracked files become
// Artefakt-Karten by convention via the pure Pattern-Zuordnung core; unlabeled tracked files
// (Waisen) land in an Unzugeordnet-Fach per Arbeitsbereich. One click on a card opens its
// dominant file or its folder via the OS default program — no per-file bureaucracy.

/** The derived primary action of a card (PRD §14). `datei` → open the Hauptdatei in the OS default
 *  program; `ordner` → open the folder. Mirrors `PrimaerAktion` in src-tauri/src/zuordnung.rs. */
export type PrimaerAktion = "datei" | "ordner";

/** The live, derived Artefakt-Karten-Status from Git (Issue #53, E26) — never stored, always
 *  read back. Mirrors `KartenStatus` in src-tauri/src/kartenstatus.rs (serde kebab-case). The
 *  card is "im Alltag fast stumm": `vorhanden` is the quiet normal case, `geaendert`/`fehlt` are
 *  the loud "prüf-mich" cases, `uebernommen` a quiet hint, `ignoriert` the silent out-of-band one.
 *  Note the misspelling-free tokens follow the Rust enum names (ä → ae). */
export type KartenStatus =
  | "vorhanden"
  | "geaendert"
  | "fehlt"
  | "uebernommen"
  | "ignoriert";

/** The derived card projection (Issue #53): the folded Git status PLUS the orthogonal Stale flag.
 *  `status` comes from Git, `stale` from Kanten (E26/E40: no edge ⇒ never stale). A quiet
 *  "vorhanden" card can still be stale. Mirrors `KartenProjektion` in src-tauri/src/kartenstatus.rs.
 *  This is the shape #55 (filters) and #56 (edges) consume. */
export interface KartenProjektion {
  status: KartenStatus;
  /** True iff a Hand-Kante exists and a source is newer than this derivation (E26/E40). */
  stale: boolean;
}

/** An Artefakt-Karte built by convention from tracked files. Mirrors `ArtefaktKarte` in
 *  src-tauri/src/werkbank.rs. */
export interface ArtefaktKarte {
  /** Stable key "<baustein-id>:<ordner>"; the UI keys cards on it. */
  artefakt_id: string;
  /** Human Baustein name (e.g. "KiCad"); the card label. */
  baustein: string;
  /** Artifact folder relative to the product root (forward slashes). */
  ordner: string;
  /** The Hauptdatei (highest glob priority), product-relative; null in the degenerate case. */
  hauptdatei: string | null;
  /** All tracked files of this artifact, product-relative, sorted. */
  dateien: string[];
  /** Derived one-click action: open the dominant file vs. the folder. */
  primaer: PrimaerAktion;
  /** Absolute on-disk target of the primary action (file or folder), for OS-default open. */
  ziel: string | null;
  /** Live, derived Karten-Status + Stale flag (Issue #53, E26) — from Git + Kanten, never stored. */
  projektion: KartenProjektion;
}

/** An Unzugeordnet-Fach per Arbeitsbereich: the Waisen (tracked files lacking a label). Nothing is
 *  lost by omission; the folder context is the assignment hint. Mirrors `UnzugeordnetFach`. */
export interface UnzugeordnetFach {
  /** The Arbeitsbereich (top-level folder name; "" = product root). */
  arbeitsbereich: string;
  /** The Waise files, product-relative, sorted. */
  dateien: string[];
}

/** The product's Werkbank view: Artefakt-Karten + Unzugeordnet-Fächer. Mirrors `WerkbankView` in
 *  src-tauri/src/werkbank.rs. Returned by the `read_werkbank_cmd` command. */
export interface WerkbankView {
  karten: ArtefaktKarte[];
  unzugeordnet: UnzugeordnetFach[];
}

// Baustein stilllegen (Issue #51, E17). Label-only und (fast) umkehrbar: die alten Globs hören auf
// zu greifen → ihre Dateien werden zu Waisen im Unzugeordnet-Fach; die Ignore-/LFS-Marker-Blöcke
// bleiben als Sediment liegen; nichts wird verschoben oder gelöscht.

/** The label-only effect of decommissioning a Baustein. Mirrors `StilllegenWirkung` in
 *  src-tauri/src/stilllegen.rs. */
export interface StilllegenWirkung {
  /** The Baustein's globs that stop matching. */
  erloschene_globs: string[];
  /** Previously-labelled files that become Waisen (orphans), product-relative, sorted. */
  neue_waisen: string[];
  /** The Ignore/LFS marker lines that remain as Sediment in the dotfiles. */
  sediment: string[];
  /** Invariant: nothing is moved or deleted — always true. */
  nichts_bewegt: boolean;
}

/** The result of the stilllegen command: the effect plus the rewritten stack and freshly folded
 *  Werkbank (so the new Waisen show in the Unzugeordnet-Fach at once). Mirrors `StilllegenResult`. */
export interface StilllegenResult {
  wirkung: StilllegenWirkung;
  stack: ProduktStack;
  werkbank: WerkbankView;
}

// The Produkt-Registry + produktübergreifende Live-Suche (Issue #45, E45). The registry is
// PATH-ONLY (no content cached — a second copy would drift, E8/E18); search is a LIVE fan-out
// that opens each reachable product and greps over Dateinamen/`_plm`/`VERSION_NOTES.md`.
// Unreachable products are reported honestly, never silently dropped.

/** One registered product. Path-only plus a derived display name. Mirrors `RegisteredProduct`
 *  in src-tauri/src/registry.rs. */
export interface RegisteredProduct {
  /** Absolute path to the product folder — the single source of truth for this entry. */
  path: string;
  /** Folder name, derived from `path` (a display convenience, never a second fact). */
  name: string;
}

/** Which of a product's three searched sources a hit came from. Mirrors `HitField` in
 *  src-tauri/src/search.rs (serde kebab-case). */
export type HitField = "dateiname" | "plm" | "version-notes";

/** One match inside one product. Mirrors `SearchHit` in src-tauri/src/search.rs. */
export interface SearchHit {
  product_path: string;
  product_name: string;
  field: HitField;
  /** Product-relative file the hit was found in (forward slashes). */
  file: string;
  /** Matched text: a relative file path for `dateiname`, the matched line for content. */
  text: string;
  /** Computed relevance; higher sorts first. */
  score: number;
}

/** A registered product that could not be searched. Mirrors `OfflineProduct`. */
export interface OfflineProduct {
  product_path: string;
  product_name: string;
  /** Human German reason, e.g. "Ordner nicht erreichbar". */
  reason: string;
}

/** The full result of one fan-out search. Mirrors `SearchResult` in src-tauri/src/search.rs. */
export interface SearchResult {
  /** All hits across reachable products, merged + ranked (best first). */
  hits: SearchHit[];
  /** Registered products that could not be opened — reported, never silently dropped. */
  offline: OfflineProduct[];
  /** How many registered products were searched (reachable). */
  searched: number;
  /** Total registered products considered (`searched + offline.length`). */
  total: number;
}

// Aufgaben & Hinweise (Issue #40, PRD US 27–30). First-class objects in the product, stored in
// the product-local `_plm` store. Aufgaben (verpflichtend, *können* blockieren) and Hinweise
// (blockieren nie) are separated PURELY by Blockier-Fähigkeit — not by importance. The block
// DECISION itself is a later slice (Issue #49); here we only carry the kind + the flag.

/** Typ: the ONLY thing separating the two — an `aufgabe` can block, a `hinweis` never does.
 *  Mirrors `TaskKind` in src-tauri/src/tasks.rs. */
export type TaskKind = "aufgabe" | "hinweis";

/** Lifecycle status (no Kanban, US 28). Only `offen` items can ever block.
 *  Mirrors `TaskStatus` in src-tauri/src/tasks.rs. */
export type TaskStatus = "offen" | "erledigt" | "verworfen";

/** The optional Verknüpfung (US 28): Produkt / Version / Arbeitsbereich / Artefakt, or none.
 *  Serde-tagged `{ kind, ref }`. Mirrors `TaskLink` in src-tauri/src/tasks.rs. */
export type TaskLink =
  | { kind: "produkt" }
  | { kind: "version"; ref: string }
  | { kind: "arbeitsbereich"; ref: string }
  | { kind: "artefakt"; ref: string };

/** One Aufgabe or Hinweis. Minimal model: Titel/Status/Typ/Verknüpfung/Fälligkeit + the
 *  „blockiert überall" opt-out (US 30). Mirrors `Task` in src-tauri/src/tasks.rs. */
export interface Task {
  /** Stable opaque id the store assigns; the UI keys rows on it. */
  id: string;
  /** Titel — the one piece of free human text. */
  title: string;
  /** Typ: aufgabe (block-capable) vs. hinweis (never blocks). */
  kind: TaskKind;
  status: TaskStatus;
  /** Optional Verknüpfung; `null` = free-floating. */
  link: TaskLink | null;
  /** Optional Fälligkeit `YYYY-MM-DD`; `null` = kein Termin. */
  due: string | null;
  /** „blockiert überall" opt-out (US 30): block kontextunabhängig (only meaningful for an
   *  Aufgabe; the block decision is Issue #49). */
  blocks_everywhere: boolean;
  /** Creation timestamp `YYYY-MM-DDTHH:MM:SSZ` (the store sets it). */
  created_at: string;
}

// -------------------------------------------------------------------------------------------------
// Aufgaben-Block decision (Issue #49, E42). The pure core decides whether open Aufgaben block a
// checkpoint, carrying the Strenge on the Revision-Art (NOT a Branch-Typ): a "freigabe" is
// blocked by any open Aufgabe, a "prototyp" only by an open „blockiert überall" Aufgabe, and a
// Hinweis never blocks. Surfaced by the `evaluate_task_block` command; Issue #52's Freigabe-Gate
// consumes it. Mirrors `BlockDecision` in src-tauri/src/aufgabenblock.rs.
// -------------------------------------------------------------------------------------------------

/** Whether a checkpoint at the intended Revision-Art is blocked by open Aufgaben, and by which.
 *  Mirrors `BlockDecision` in src-tauri/src/aufgabenblock.rs. */
export interface BlockDecision {
  /** Whether the checkpoint is blocked at all. `true` iff `blocking_task_ids` is non-empty. */
  blocked: boolean;
  /** Ids of the open Aufgaben that block this checkpoint, in input order. Empty ⇔ not blocked. */
  blocking_task_ids: string[];
}

// -------------------------------------------------------------------------------------------------
// Freigabe-Gate (Issue #52, E19/E19.3). The dreistufige Block in ONE context-dependent button:
// open points are collected from open Aufgaben (#49), Waisen/Pflicht (#47) and Stale-Kanten (#10),
// staffed nach Härte (hardest first), and the one button changes label + severity. Surfaced by the
// `evaluate_freigabe_gate` command. Mirrors `GateVerdict` & co in src-tauri/src/freigabegate.rs.
// -------------------------------------------------------------------------------------------------

/** The Härte of an open point — ordered hardest first. Mirrors `Haerte` (serde kebab-case). */
export type Haerte = "hart" | "weich" | "warnung";

/** What kind of open point this is — the source axis behind its Härte. Mirrors `Punktart`. */
export type Punktart = "aufgabe" | "waise" | "fehlende-pflicht" | "stale-kante";

/** The three states of the ONE context-dependent button. Mirrors `KnopfZustand`:
 *  - "taggen": alles sauber (or warning-only) → proceed freely;
 *  - "trotzdem-freigeben": a weicher Block → proceed with a logged Begründung;
 *  - "gesperrt-durch-aufgabe": a harter Block → button off, dismiss only by acting on the task. */
export type KnopfZustand = "taggen" | "trotzdem-freigeben" | "gesperrt-durch-aufgabe";

/** One open point in the härte-sortierte Liste. Mirrors `OffenerPunkt`. */
export interface OffenerPunkt {
  haerte: Haerte;
  art: Punktart;
  /** Task id for an Aufgabe; product-relative path for a Waise/Stale-Kante; Pflicht label. */
  ref_id: string;
  /** Human one-liner naming the point (task title, orphan filename, …). */
  label: string;
}

/** A personenübergreifende Warnung — a colleague's frischer Stand co-tagged. Mirrors `FremdWarnung`. */
export interface FremdWarnung {
  /** The colleague whose Stand is co-tagged. */
  wer: string;
  /** A ready human sentence („du taggst auch X' frischen Stand mit"). */
  satz: string;
}

/** The Freigabe-Gate verdict: the härte-sortierte Liste + the one button's Zustand + the optional
 *  cross-person warning. Mirrors `GateVerdict` in src-tauri/src/freigabegate.rs. */
export interface GateVerdict {
  /** Open points, hardest first (hart, then weich, then warnung); stable within a Härte. */
  punkte: OffenerPunkt[];
  /** The resulting state of the one context-dependent button. */
  knopf: KnopfZustand;
  /** `true` iff a harter Block is present (button off; only the task dismisses it). */
  harter_block: boolean;
  /** `true` iff a protokollierter Satz is required to proceed (weicher Block, no harter). */
  begruendung_noetig: boolean;
  /** The cross-person warning, if a colleague's frischer Stand is being co-tagged. */
  fremd_warnung: FremdWarnung | null;
}
