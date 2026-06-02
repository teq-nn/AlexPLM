// The live-status pulse (Candidate 04). The page orchestrator (`routes/+page.svelte`) used to hold
// the 4-second polling loop, its in-flight guard, and the derived Auto-Lock state inline among 37
// other concerns. Lifted here behind a small `start()`/`stop()` interface so the timer lifecycle
// lives in one place (a missed cleanup can't leak), the loop is testable without mounting the page,
// and the page consumes the reactive readout instead of owning it.
//
// Scope is deliberately the STATUS half only: the per-artifact LED `signals`, the `foreignLocks`
// panel, and the Lock Warden's last `wardenAction`. The silent net-sync loop and the loud exception
// stay in the page for now — they straddle the publish ceremony (setup/republish), so folding them
// in is a separate, larger cut.
//
// All state is derived purely by reading git back (`git lfs locks` + worktree status); nothing is
// mirrored or cached as a second truth (Issue #6, E37). The Lock Warden's safety decision lives in
// the Rust core — this only reflects the action it returns.
import { cmd } from "$lib/commands";
import type { ArtifactSignal, ForeignLock, WardenAction, ProductView } from "$lib/types";

/** What the pulse reads from / reports back to the page. Getters, not snapshots: the loop must read
 *  the page's LIVE state at each tick, never a value frozen when the pulse was created. */
export interface StatusPulseDeps {
  /** The open product folder, or null when none is open. */
  productPath: () => string | null;
  /** The open product — its Bausteine carry the Hauptdateien that each get an LED. */
  product: () => ProductView | null;
  /** Extra Hauptdateien to light an LED for: the Artefakt-Karten (#47) live in the Werkbank, whose
   *  shape the page owns, so it hands their paths in rather than the pulse reaching into it. */
  extraPaths: () => string[];
  /** Surface a real, loud coordination error — a foreign-held lock, or a failed status read. */
  onError: (message: string) => void;
}

/** The reactive readout + control surface the page consumes. */
export interface StatusPulse {
  /** Per-artifact Auto-Lock LED, keyed on the product-relative Hauptdatei (E37). */
  readonly signals: Record<string, ArtifactSignal>;
  /** Colleagues' currently-held locks, for the foreign-locks panel (E37). */
  readonly foreignLocks: ForeignLock[];
  /** The Lock Warden's last decided action, reflected by the Sicherungsstatus readout (E35). */
  readonly wardenAction: WardenAction | null;
  /** Begin (or restart) the 4-second polling loop, pulling once immediately. */
  start(): void;
  /** Stop the loop; safe to call when already stopped (used in onDestroy + reset). */
  stop(): void;
  /** Clear the derived state and stop the loop — for closing/switching products. */
  reset(): void;
  /** Re-read the world from git: per-artifact LED status + the foreign-locks panel. */
  refreshStatus(): Promise<void>;
  /** Run a Lock Warden checkpoint for one artifact and reflect the action it decided.
   *  Best-effort: a push failure (e.g. no server yet) must never break the silent rhythm. */
  runCheckpoint(path: string, revision: boolean): Promise<void>;
  /** Auto-unlock every held lock whose path is now locally clean (Issue #42). Best-effort. */
  sweepCleanLocks(): Promise<void>;
  /** Editing/opening a lockable artifact auto-acquires a `git lfs lock` (E31), then re-reads. */
  editBaustein(mainFile: string | null): Promise<void>;
  /** Reflect a Warden action decided elsewhere (the page's manual Sichern / Freigeben gestures).
   *  A `refuse` lights nothing — the daily rhythm stays silent. */
  noteAction(action: WardenAction): void;
}

export function createStatusPulse(deps: StatusPulseDeps): StatusPulse {
  let signals = $state<Record<string, ArtifactSignal>>({});
  let foreignLocks = $state<ForeignLock[]>([]);
  // `refuse` surfaces as nothing — the daily rhythm stays silent.
  let wardenAction = $state<WardenAction | null>(null);

  let statusTimer: ReturnType<typeof setInterval> | null = null;
  // Guard so a slow status read (a networked `git lfs locks` can take up to the backend bound) never
  // overlaps the next 4-second tick. Without it, ticks pile up faster than they drain. Plain, not
  // `$state` — purely internal, nothing renders off it.
  let statusInFlight = false;

  function noteAction(action: WardenAction) {
    // Only a real action lights the readout; Refuse leaves the rhythm silent.
    if (action !== "refuse") wardenAction = action;
  }

  async function refreshStatus() {
    const productPath = deps.productPath();
    const product = deps.product();
    if (!productPath || !product || statusInFlight) return;
    statusInFlight = true;
    const paths = Array.from(
      new Set(
        [
          ...product.bausteine.map((b) => b.main_file),
          // The convention Artefakt-Karten (#47) carry the LED on their Hauptdatei too.
          ...deps.extraPaths(),
        ].filter((f): f is string => f !== null && f !== undefined),
      ),
    );
    try {
      const [sigs, foreign] = await Promise.all([
        cmd.readStatus(productPath, paths),
        cmd.readForeignLocks(productPath),
      ]);
      signals = Object.fromEntries(sigs.map((s) => [s.path, s]));
      foreignLocks = foreign;
    } catch (e) {
      // Read-only status is best-effort; never blocks the shell (e.g. no LFS remote).
      deps.onError(String(e));
    } finally {
      statusInFlight = false;
    }
  }

  async function runCheckpoint(path: string, revision: boolean) {
    const productPath = deps.productPath();
    if (!productPath) return;
    try {
      const action = await cmd.runCheckpoint(productPath, path, revision);
      noteAction(action);
      // At every checkpoint, self-heal: auto-unlock every held lock whose path is now locally
      // clean (committed, no open edit). The Lock Warden decides per path (Issue #42, E31/E35);
      // the freed binaries rest read-only (frei) again. Best-effort — never breaks the rhythm.
      await sweepCleanLocks();
      await refreshStatus();
    } catch (e) {
      // The two push types are background safety nets; surfacing the raw error would break the
      // silent vocabulary, so we swallow it (a louder, in-tool sync error is a later slice).
    }
  }

  async function sweepCleanLocks() {
    const productPath = deps.productPath();
    if (!productPath) return;
    try {
      await cmd.sweepCleanLocks(productPath);
    } catch (e) {
      // Self-healing is a quiet safety net; a hiccup must never surface as a loud error.
    }
  }

  async function editBaustein(mainFile: string | null) {
    const productPath = deps.productPath();
    if (!productPath || !mainFile) return;
    try {
      await cmd.lockArtifact(productPath, mainFile);
    } catch (e) {
      deps.onError(String(e)); // a foreign-held lock is real, loud coordination — surface it
    }
    await refreshStatus();
  }

  function start() {
    stop();
    void refreshStatus();
    statusTimer = setInterval(() => void refreshStatus(), 4000);
  }

  function stop() {
    if (statusTimer !== null) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  }

  function reset() {
    signals = {};
    foreignLocks = [];
    wardenAction = null;
    stop();
  }

  return {
    get signals() {
      return signals;
    },
    get foreignLocks() {
      return foreignLocks;
    },
    get wardenAction() {
      return wardenAction;
    },
    start,
    stop,
    reset,
    refreshStatus,
    runCheckpoint,
    sweepCleanLocks,
    editBaustein,
    noteAction,
  };
}
