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
}

/** The version tree + active milestone the version bar shows in Mono (Issue #8).
 *  Mirrors `VersionGraph` in src-tauri/src/graph.rs. */
export interface VersionGraph {
  nodes: StandNode[];
  active_milestone: string | null;
  offloaded_archive: string | null;
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
