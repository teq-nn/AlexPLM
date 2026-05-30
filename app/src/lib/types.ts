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
