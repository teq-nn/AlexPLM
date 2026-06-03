// ── Verlauf: lokale „zuletzt geöffnet"-Reihenfolge (Issue #73) ────────────────
// The Produkt-Registry is deliberately path-only — registry.rs stores no extra facts that could
// drift from disk (E8/E18). The order „zuletzt geöffnet" in der Suche is therefore a LOCAL view
// convenience, kept the same way the column widths already are: a plain localStorage map of
// path → epoch-ms. Stamped from +page.svelte on every open/import/switch (rememberProduct), read
// + sorted by the „Suche"-Panel. A path with no stamp sorts last (known, but never opened here).

const HISTORY_KEY = "plm.zuletzt-geoeffnet";

export type Verlauf = Record<string, number>;

/** Read the local last-opened map. Best-effort: a missing/corrupt entry reads as empty. */
export function readHistory(): Verlauf {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    return parsed && typeof parsed === "object"
      ? (parsed as Verlauf)
      : {};
  } catch {
    return {};
  }
}

function writeHistory(next: Verlauf) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
  } catch {
    // The Verlauf order is a view convenience; a full/blocked storage must never break the shell.
  }
}

/** Stamp a path as just-opened, returning the updated map (so callers can hold it reactively). */
export function markOpened(path: string): Verlauf {
  const next = { ...readHistory(), [path]: Date.now() };
  writeHistory(next);
  return next;
}

/** Drop a path's stamp, so a product removed from the registry cannot resurface ranked. */
export function forget(path: string): Verlauf {
  const next = { ...readHistory() };
  delete next[path];
  writeHistory(next);
  return next;
}

/** A relative „zuletzt geöffnet" stamp in the tool's quiet vocabulary — never an absolute time.
 *  Returns "" when never opened from here, so the row can simply omit the label. */
export function seit(ts: number | undefined): string {
  if (!ts) return "";
  const mins = Math.floor((Date.now() - ts) / 60000);
  if (mins < 1) return "gerade eben";
  if (mins < 60) return `vor ${mins} min`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `vor ${hrs} h`;
  const days = Math.floor(hrs / 24);
  return days < 7 ? `vor ${days} d` : `vor ${Math.floor(days / 7)} w`;
}
