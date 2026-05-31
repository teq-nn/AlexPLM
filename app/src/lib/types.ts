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

/** A node in the dark "display" version tree (Issue #8).
 *  Mirrors `StandNode` in src-tauri/src/graph.rs. */
export interface StandNode {
  /** Stable id (commit hash); the UI keys rows on this, never shown as git. */
  id: string;
  /** Machine timestamp `YYYY-MM-DDTHH:MM:SSZ`. */
  timestamp: string;
  /** Product-relative path recovered from the boring auto message; "." otherwise. */
  path: string;
  /** Human version label if this Stand was promoted to a Meilenstein, else null. */
  milestone: string | null;
  /** Whether VERSION_NOTES.md text exists for this Meilenstein. */
  has_notes: boolean;
  /** Whether this node's binary content was offloaded to a cold archive (E36). */
  offloaded: boolean;
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

/** The version tree + active milestone the version bar shows in Mono (Issue #8 / #28).
 *  Mirrors `VersionGraph` in src-tauri/src/graph.rs. */
export interface VersionGraph {
  nodes: StandNode[];
  active_milestone: string | null;
  offloaded_archive: string | null;
  /** Name of the active line (Zweig), echoed for the UI marker; null if unknown. */
  active_branch: string | null;
  /** Number of distinct lanes; 1 for a single linear history. */
  lane_count: number;
}

/** A manual „abgeleitet von" edge: `derived` „stammt aus" `source` (Issue #10).
 *  Both are product-relative artifact paths. Mirrors `Edge` in src-tauri/src/edges.rs. */
export interface Edge {
  /** The derivation — made *from* `source`. */
  derived: string;
  /** The source the derivation „stammt aus". */
  source: string;
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

/** A foreign lock for the live "fremde Sperren" panel. Mirrors `ForeignLock` in lib.rs. */
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
