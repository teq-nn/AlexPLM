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
