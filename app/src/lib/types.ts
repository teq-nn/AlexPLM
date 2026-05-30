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
