// Reproducible documentation screenshots for the Werkbank.
//
// The app is a Tauri desktop program: its Svelte frontend talks to a Rust backend over
// `window.__TAURI_INTERNALS__.invoke`. To capture authentic UI without a desktop/GPU, we:
//   1. build the real frontend (`pnpm build` → ../../app/build, a static SPA),
//   2. serve it over localhost,
//   3. load it in headless Chrome with a MOCKED `__TAURI_INTERNALS__` that returns a
//      representative "Ember Reverb" product (the example used throughout the design docs),
//   4. drive the real components into a few states and screenshot the window + element crops.
//
// These are the real Svelte components and the real design tokens — only the backend data is
// stand-in. Run with: `node capture.mjs` (after `pnpm -C ../../app build`).

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";

const HERE = fileURLToPath(new URL(".", import.meta.url));
const BUILD_DIR = resolve(HERE, "../../app/build");
const OUT_DIR = resolve(HERE, "../../docs/img");
const CHROME =
  process.env.CHROME_PATH ||
  "/root/.cache/puppeteer/chrome/linux-131.0.6778.204/chrome-linux64/chrome";

const WIDTH = 1100;
const HEIGHT = 720;

// ── Representative backend data ("Ember Reverb" — the design docs' running example) ──────────
const PRODUCT_ROOT = "/Engineering/Products/ember-reverb";
const TS = "2026-05-30 14:22";

const baseBaustein = (id, name, heimat) => ({
  id,
  version: 1,
  name,
  heimat,
  globs: ["*"],
  ignore: [],
  lfs: [],
  oeffnen: "auto",
  startaufgaben: [],
  default_kanten: [],
  stillgelegt: false,
  herkunft: { from: id, version: 1 },
});

const MOCK = {
  openPath: PRODUCT_ROOT,

  product: {
    name: "Ember Reverb",
    branch: "main",
    version: "v0.4",
    bausteine: [
      { name: "elektronik", path: "elektronik", main_file: "elektronik/ember.kicad_pro" },
      { name: "mechanik", path: "mechanik", main_file: "mechanik/enclosure.f3d" },
      { name: "firmware", path: "firmware", main_file: null },
    ],
  },

  stack: {
    toolstack: "Geräteentwicklung",
    bausteine: [
      baseBaustein("kicad", "KiCad", "elektronik"),
      baseBaustein("fusion", "Fusion 360", "mechanik"),
      baseBaustein("zephyr", "Zephyr", "firmware"),
    ],
  },

  werkbank: {
    karten: [
      {
        artefakt_id: "kicad:elektronik",
        baustein: "KiCad",
        ordner: "elektronik",
        hauptdatei: "elektronik/ember.kicad_pro",
        dateien: [
          "elektronik/ember.kicad_pro",
          "elektronik/ember.kicad_sch",
          "elektronik/ember.kicad_pcb",
          "elektronik/sym-lib-table",
          "elektronik/fp-lib-table",
        ],
        primaer: "datei",
        ziel: `${PRODUCT_ROOT}/elektronik/ember.kicad_pro`,
        projektion: { status: "geaendert", stale: false },
      },
      {
        artefakt_id: "fusion:mechanik",
        baustein: "Fusion 360",
        ordner: "mechanik",
        hauptdatei: "mechanik/enclosure.f3d",
        dateien: [
          "mechanik/enclosure.f3d",
          "mechanik/enclosure.step",
          "mechanik/front_panel.dxf",
        ],
        primaer: "datei",
        ziel: `${PRODUCT_ROOT}/mechanik/enclosure.f3d`,
        projektion: { status: "vorhanden", stale: false },
      },
      {
        artefakt_id: "zephyr:firmware",
        baustein: "Zephyr",
        ordner: "firmware",
        hauptdatei: null,
        dateien: ["firmware/src/main.c", "firmware/prj.conf", "firmware/CMakeLists.txt"],
        primaer: "ordner",
        ziel: `${PRODUCT_ROOT}/firmware`,
        projektion: { status: "vorhanden", stale: false },
      },
    ],
    unzugeordnet: [
      {
        arbeitsbereich: "dokumentation",
        dateien: ["dokumentation/manual.md", "dokumentation/testprotokoll.pdf"],
      },
    ],
  },

  // Per-artifact LED status (read back from git lfs locks + worktree). KiCad-Hauptdatei is free
  // (green), the Gehäuse is held by a colleague (the single orange "laute" accent).
  signals: [
    { path: "elektronik/ember.kicad_pro", status: "free" },
    {
      path: "mechanik/enclosure.f3d",
      status: "locked-by-other",
      locked_by: "Ben",
      locked_at: TS,
      tooltip: `gesperrt von Ben seit ${TS}`,
    },
  ],
  foreign: [
    { path: "mechanik/enclosure.f3d", owner: "Ben", locked_at: TS, tooltip: `gesperrt von Ben seit ${TS}` },
  ],

  graph: {
    nodes: [
      { id: "a1c0de1", timestamp: "2026-05-12T09:12:03Z", path: ".", milestone: "v0.1", milestone_art: "freigabe", has_notes: true, offloaded: false, veroeffentlicht: true, lane: 0, branch: "main", on_active: true, parents: [] },
      { id: "a2c0de2", timestamp: "2026-05-15T11:40:55Z", path: "elektronik/ember.kicad_sch", milestone: null, milestone_art: null, has_notes: false, offloaded: false, veroeffentlicht: true, lane: 0, branch: null, on_active: true, parents: ["a1c0de1"] },
      { id: "a3c0de3", timestamp: "2026-05-18T16:02:11Z", path: ".", milestone: "v0.2", milestone_art: "freigabe", has_notes: true, offloaded: false, veroeffentlicht: true, lane: 0, branch: null, on_active: true, parents: ["a2c0de2"] },
      { id: "a4c0de4", timestamp: "2026-05-21T10:21:47Z", path: "mechanik/enclosure.f3d", milestone: null, milestone_art: null, has_notes: false, offloaded: false, veroeffentlicht: true, lane: 0, branch: null, on_active: true, parents: ["a3c0de3"] },
      { id: "a5c0de5", timestamp: "2026-05-24T13:55:09Z", path: ".", milestone: "v0.3", milestone_art: "freigabe", has_notes: true, offloaded: false, veroeffentlicht: true, lane: 0, branch: null, on_active: true, parents: ["a4c0de4"] },
      { id: "b1c0de6", timestamp: "2026-05-26T09:30:00Z", path: "mechanik/enclosure.f3d", milestone: null, milestone_art: null, has_notes: false, offloaded: false, veroeffentlicht: false, lane: 1, branch: "alternate-enclosure", on_active: false, parents: ["a5c0de5"] },
      { id: "a6c0de7", timestamp: "2026-05-28T15:08:32Z", path: "elektronik/ember.kicad_sch", milestone: null, milestone_art: null, has_notes: false, offloaded: false, veroeffentlicht: true, lane: 0, branch: null, on_active: true, parents: ["a5c0de5"] },
      { id: "a7c0de8", timestamp: "2026-05-30T14:20:00Z", path: ".", milestone: "v0.4", milestone_art: "prototyp", has_notes: false, offloaded: false, veroeffentlicht: false, lane: 0, branch: null, on_active: true, parents: ["a6c0de7"] },
    ],
    active_milestone: "v0.4",
    active_milestone_art: "prototyp",
    offloaded_archive: null,
    active_branch: "main",
    lane_count: 2,
  },

  edges: { edges: [], warnings: [], vorschlaege: [] },

  // App-wide Konto (ADR 0004): one server identity, reused for all products.
  konto: { base_url: "https://forgejo.nilius.online", account: "anna" },

  // Produkt-Registry for the Produktliste switcher (Issue #73).
  products: [
    { path: PRODUCT_ROOT, name: "ember-reverb" },
    { path: "/Engineering/Products/lumen-drive", name: "lumen-drive" },
    { path: "/Engineering/Products/tidal-sensor", name: "tidal-sensor" },
  ],

  // Freigabe-Gate verdict (Issue #52): a weicher Block needing a logged reason.
  gateVerdict: {
    punkte: [
      { haerte: "weich", art: "fehlende-pflicht", ref_id: "Testprotokoll", label: "Pflicht-Artefakt Testprotokoll fehlt" },
      { haerte: "warnung", art: "aufgabe", ref_id: "t2", label: "BOM exportieren (JLCPCB)" },
    ],
    knopf: "trotzdem-freigeben",
    harter_block: false,
    begruendung_noetig: true,
    fremd_warnung: null,
  },

  tasks: [
    { id: "t1", title: "Schaltplan: Eingangsstufe entrauschen", kind: "aufgabe", status: "offen", link: { kind: "artefakt", ref: "elektronik" }, due: "2026-06-05", blocks_everywhere: false, created_at: "2026-05-28T08:00:00Z" },
    { id: "t2", title: "BOM exportieren (JLCPCB)", kind: "aufgabe", status: "offen", link: null, due: null, blocks_everywhere: false, created_at: "2026-05-29T08:00:00Z" },
    { id: "t3", title: "Gehäuse-Toleranzen mit Ben klären", kind: "hinweis", status: "offen", link: { kind: "artefakt", ref: "mechanik" }, due: null, blocks_everywhere: false, created_at: "2026-05-29T09:00:00Z" },
    { id: "t4", title: "Testprotokoll v0.3 abgelegt", kind: "aufgabe", status: "erledigt", link: null, due: null, blocks_everywhere: false, created_at: "2026-05-24T09:00:00Z" },
  ],

  setup: {
    stage: "eingerichtet",
    has_remote: true,
    has_published: true,
    clone_url: "https://git.teqsas.de/ember/ember-reverb.git",
  },

  importResult: {
    git_initialized: true,
    locked_count: 7,
    product: {
      name: "Ember Reverb",
      branch: "main",
      version: "v0.1",
      bausteine: [
        { name: "elektronik", path: "elektronik", main_file: "elektronik/ember.kicad_pro" },
      ],
    },
  },

  gate: { decision: "clean-init", has_history: false, shared_clones_exist: false, giant_binaries_in_history: false },
};

// ── The injected Tauri mock (runs in the browser before app scripts) ─────────────────────────
function installTauriMock(M) {
  function route(cmd) {
    if (cmd.startsWith("plugin:event|")) return 0;
    if (cmd === "plugin:dialog|open") return M.openPath;
    if (cmd.startsWith("plugin:opener|")) return null;
    switch (cmd) {
      case "open_product": return M.product;
      case "import_product": return M.importResult;
      case "evaluate_gate": return M.gate;
      case "read_version_graph": return M.graph;
      case "read_edges": return M.edges;
      case "list_tasks": return M.tasks;
      case "read_werkbank_cmd": return M.werkbank;
      case "read_product_stack": return M.stack;
      case "read_setup_state": return M.setup;
      case "read_status": return M.signals;
      case "read_foreign_locks": return M.foreign;
      case "sync_product": return { status: "aktuell" };
      case "sichern": return "sicherungs-push";
      case "run_checkpoint":
      case "freigeben": return "refuse";
      case "sweep_clean_locks": return [];
      case "lock_artifact": return true;
      // App-wide Konto + product switcher + release gate (current build).
      case "read_konto": return M.konto;
      case "save_konto": return M.konto;
      case "list_products": return M.products;
      case "search_products":
        return { hits: [], offline: [], searched: M.products.length, total: M.products.length };
      case "evaluate_freigabe_gate": return M.gateVerdict;
      default: return null;
    }
  }
  const cbs = {};
  let cbid = 0;
  window.__TAURI_INTERNALS__ = {
    transformCallback(cb) { const id = ++cbid; cbs[id] = cb; return id; },
    unregisterCallback(id) { delete cbs[id]; },
    convertFileSrc(p) { return p; },
    invoke(cmd) { return Promise.resolve(route(cmd)); },
  };
}

// ── A tiny static server for the built SPA ───────────────────────────────────────────────────
const MIME = {
  ".html": "text/html", ".js": "text/javascript", ".css": "text/css",
  ".json": "application/json", ".png": "image/png", ".svg": "image/svg+xml",
  ".woff": "font/woff", ".woff2": "font/woff2", ".ico": "image/x-icon",
};
function serve(dir) {
  return new Promise((res) => {
    const server = createServer(async (req, reqRes) => {
      try {
        const url = decodeURIComponent((req.url || "/").split("?")[0]);
        let file = join(dir, url);
        if (url === "/" || !existsSync(file)) file = join(dir, "index.html"); // SPA fallback
        const body = await readFile(file);
        reqRes.writeHead(200, { "content-type": MIME[extname(file)] || "application/octet-stream" });
        reqRes.end(body);
      } catch {
        reqRes.writeHead(404); reqRes.end("not found");
      }
    });
    server.listen(0, "127.0.0.1", () => res({ server, port: server.address().port }));
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  if (!existsSync(BUILD_DIR)) {
    console.error(`Build not found at ${BUILD_DIR} — run \`pnpm -C app build\` first.`);
    process.exit(1);
  }
  const { server, port } = await serve(BUILD_DIR);
  const base = `http://127.0.0.1:${port}/`;

  const browser = await puppeteer.launch({
    executablePath: CHROME,
    headless: "new",
    args: ["--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage", "--force-color-profile=srgb"],
    defaultViewport: { width: WIDTH, height: HEIGHT, deviceScaleFactor: 2 },
  });

  async function newPage() {
    const page = await browser.newPage();
    await page.evaluateOnNewDocument(installTauriMock, MOCK);
    return page;
  }

  const shots = [];
  async function shot(name, target, opts = {}) {
    const p = join(OUT_DIR, `${name}.png`);
    if (typeof target === "string") {
      const el = await opts.page.$(target);
      if (!el) throw new Error(`selector not found for ${name}: ${target}`);
      await el.screenshot({ path: p });
    } else {
      await target.screenshot({ path: p, ...opts });
    }
    shots.push(name);
  }

  // Scene 1 — empty state ("Ordner wählen")
  {
    const page = await newPage();
    await page.goto(base, { waitUntil: "networkidle0" });
    await page.waitForSelector(".empty-panel");
    await sleep(400);
    await shot("leer-startbildschirm", page, { page });
    await page.close();
  }

  // Helper: open the example product from the empty state, land in the Werkbank.
  async function openExampleProduct(page) {
    await page.goto(base, { waitUntil: "networkidle0" });
    await page.waitForSelector(".empty-panel");
    await page.evaluate(() => {
      const btn = [...document.querySelectorAll("button")].find((b) =>
        b.textContent.includes("Produkt öffnen"),
      );
      btn?.click();
    });
    await page.waitForSelector(".grid");
    await page.waitForSelector("header.bar .version");
    await sleep(700); // fonts settle + LEDs/foreign locks read back
  }

  // Helper: click a button whose visible text contains `text`.
  async function clickByText(page, text) {
    await page.evaluate((t) => {
      const btn = [...document.querySelectorAll("button")].find((b) =>
        b.textContent.includes(t),
      );
      btn?.click();
    }, text);
  }

  // Scene 2 — a populated product (open the example "Ember Reverb")
  {
    const page = await newPage();
    await openExampleProduct(page);

    await shot("werkbank-uebersicht", page, { page }); // hero: whole window
    await shot("versionsleiste", "header.bar", { page });
    await shot("entrybar", ".entrybar", { page });      // Raum-Schalter, Suche, Konto, Diagnose
    await shot("sync-leiste", ".actions", { page });    // Sichern/Holen + sync status
    await shot("werkzeugkasten-leiste", ".stackbar", { page });
    await shot("artefakt-karten", ".grid", { page });
    await shot("artefakt-karte-einzeln", ".grid > *:first-child", { page });
    await shot("unzugeordnet-fach", ".waisen", { page });
    await shot("fremde-sperren", 'aside.panel[aria-label="Fremde Sperren"]', { page });
    await shot("versionsbaum", ".tree-col", { page });
    await shot("aufgaben", ".tasks-block", { page });
    await page.close();
  }

  // Scene 3 — the Graph-Raum (Verlauf) with its node verbs + filters
  {
    const page = await newPage();
    await openExampleProduct(page);
    await clickByText(page, "Verlauf · Graph");
    await sleep(500);
    // The GraphRaum renders a full-width dark room.
    const room = (await page.$(".raum")) || (await page.$(".stage"));
    await shot("graph-raum", room, { page });
    await page.close();
  }

  // Scene 4 — the Produktliste / Verlauf switcher popover
  {
    const page = await newPage();
    await openExampleProduct(page);
    await clickByText(page, "Produktliste");
    await sleep(400);
    const pop = await page.$(".produktliste .popover");
    if (pop) await shot("produktliste", pop, { page });
    else await shot("produktliste", ".entrybar", { page });
    await page.close();
  }

  // Scene 5 — the global Konto panel (Einstellungen)
  {
    const page = await newPage();
    await page.goto(base, { waitUntil: "networkidle0" });
    await page.waitForSelector(".entrybar");
    await clickByText(page, "Einstellungen");
    await sleep(500);
    const modal = await page.$(".scrim");
    if (modal) await shot("konto-panel", modal, { page });
    else await shot("konto-panel", page, { page });
    await page.close();
  }

  await browser.close();
  server.close();
  console.log(`Captured ${shots.length} screenshots into docs/img:`);
  for (const s of shots) console.log(`  • ${s}.png`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
