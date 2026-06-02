---
name: run-werkbank
description: Start the Werkbank desktop app (Tauri v2 + SvelteKit + Rust) and confirm its window is up. Use when asked to run, start, launch, or "open" the app, or to get it running so a change can be looked at. Launch only — it does not run tests or typechecks (use cargo test / npm run check for those).
---

# Start Werkbank

Werkbank is a **Tauri v2** desktop app: SvelteKit frontend (`app/src`) in a WebView
over a Rust backend (`app/src-tauri`). Binary is `target/debug/app`; product name
`Werkbank`, identifier `de.teqsas.plmwerkzeug`.

> ⚠️ **Stay in the outer repo — never use `app/src-tauri` as cwd.** The repo had a
> dual-`.git` trap. The helper scripts use absolute paths, so no `cd` is needed.

## Start it

```bash
.claude/skills/run-werkbank/scripts/launch.sh     # backgrounds the dev server, waits for ready
```

Run with Bash `run_in_background: true`. It `nohup`s `npm run tauri dev`, polls its
log up to ~240s, then returns leaving the app running. Prints `READY` once the app
launched, or `FAILED` with the cargo error.

Ready signals in `/tmp/werkbank-dev.log`:
- `VITE v… ready` + `Local: http://localhost:1420/` — frontend up.
- `Running \`target/debug/app\`` then `Git-Diagnose-Log: …` — **Rust app launched**
  (this stderr line is the cleanest "it started" marker; no ANSI noise).

Confirm independently: `pgrep -af 'target/debug/app'`. Cold builds compile the whole
Rust tree (minutes); incremental relinks are seconds.

Stop it when done:

```bash
.claude/skills/run-werkbank/scripts/stop.sh       # kills app binary + cargo run + vite
```

## Looking at the running app

Automated screenshots do **not** work on this GNOME-Wayland host — don't try:
GNOME denies the screenshot D-Bus (`AccessDenied`), the portal is interactive-only,
there is no `grim`/`scrot`, and the WebView is a **Wayland-native** window so X11
`import`/`xdotool`/`xwininfo` can't see or grab it. After a `READY`, hand off to the
human for the visual check (tell them what to click).

## Gotchas

- `beforeDevCommand` is `pnpm dev`; both `npm` and `pnpm` are installed and
  `npm run tauri dev` (what the script runs) works.
- UI work goes through the **frontend-design** skill and the existing "stille
  Werkstatt" tokens (`--surface-*`, `--ink-*`, rationed `--accent`).
