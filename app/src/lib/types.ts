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
