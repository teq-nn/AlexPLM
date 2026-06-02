// Domain types for the frontend. The command-carried types (everything that crosses the Tauri
// invoke seam) are GENERATED from the Rust structs by tauri-specta into `bindings.ts` and re-exported
// here, so a backend signature change becomes a compile error instead of a silent hand-mirror drift.
// See `commands.ts` for the typed call seam and src-tauri/src/lib.rs for the generator.
//
// Only the handful of types BELOW are still hand-written: they never cross a command return, so the
// generator never sees them — event payloads (`stand-created`) and pure frontend-display shapes.
//
// Note: the product-folder building block that ProductView carries is exported as `ProduktBaustein`
// (the generator keeps it distinct from the catalog `Baustein` in `baustein.rs`).
export * from "./bindings";

import type { Label } from "./bindings";

/** A settled save, surfaced as a new Stand. Payload of the `stand-created` event.
 *  Mirrors `Stand` in src-tauri/src/autocommit.rs. No git vocabulary.
 *  Hand-written: an event payload, never a command return — not in the generated bindings. */
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
  /** Stable commit hash for Stände rehydrated from the version graph on open (Issue #115);
   *  absent for Stände prepended live from a `stand-created` event (those carry no hash and
   *  are always genuinely new). Used only to dedupe the seeded set, never shown as git. */
  hash?: string;
}

/** One of the three Graph-Raum node verbs (Issue #55, E27). Pure frontend-display shape — the
 *  backend `KnotenVerb` (knotenverben.rs) never crosses a command return, so it is hand-written. */
export type KnotenVerb =
  | "als-ordner-oeffnen"
  | "von-hier-abzweigen"
  | "zurueckwerfen";

/** The pure Graph-Raum display filter (Issue #55, E45). Hides nodes only — never rewrites.
 *  Frontend-only; mirrors `GraphFilter` in src-tauri/src/knotenverben.rs (not command-carried). */
export interface GraphFilter {
  /** Show variant lines (non-active Zweige)? Default true. */
  varianten: boolean;
  /** Show only Revisionen (promoted Stände)? Default false. */
  nur_revisionen: boolean;
}

/** A repository label offered in the „Problem melden" picker (Issue #85). Backend `feedback::Label`
 *  exports as `Label` in the bindings; this alias keeps the long-standing frontend name. */
export type RepoLabel = Label;
