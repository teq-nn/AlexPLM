---
name: release-werkbank
description: Cut a new Werkbank release — read what merged since the last tag, decide the SemVer bump from the count and kind of features/fixes, set the version everywhere, write the CHANGELOG entry, commit and tag. Use when asked to release Werkbank, bump the version, cut a release, or "make a new version".
---

# Werkbank-Release schneiden

This is the LLM-driven side of Werkbank's versioning (Issue #105). The *app's own*
version is shown in the UI by the **Versionsschild** (bottom-right corner) and read from
`tauri.conf.json`. Your job here is to decide what the next version should be — from what
actually changed — and to stamp it consistently.

> This is the Werkbank **software** version. It is unrelated to a Produkt's
> Stand/Revision versions (`VersionBar`, `docs/konzepte/versionen.md`). Never touch those.

## What a release is

A release is a single commit on `main` that (1) bumps the version in all four files, (2)
adds a CHANGELOG entry summarising what merged, tagged `vX.Y.Z`. Releases are cut from
`main` with a clean working tree.

## Step 1 — find the baseline

```bash
git fetch --tags origin
git describe --tags --abbrev=0 2>/dev/null || echo "NO_TAGS"   # last release tag, or first ever
grep -m1 '"version"' app/src-tauri/tauri.conf.json             # current version in the tree
```

- If a tag exists, the baseline is that tag.
- If `NO_TAGS`, this is the **first** release; the baseline is the start of history and the
  CHANGELOG does not yet exist — you will create it in Step 4.

## Step 2 — gather what changed since the baseline

Prefer merged PRs (they carry the human-readable titles); fall back to commit subjects.

```bash
# Merged PRs since the last tag's date (most reliable summary of intentional change):
LAST=$(git describe --tags --abbrev=0 2>/dev/null)
SINCE=$( [ -n "$LAST" ] && git log -1 --format=%cI "$LAST" || echo "" )
gh pr list --state merged --base main --limit 200 \
   ${SINCE:+--search "merged:>$SINCE"} \
   --json number,title,labels,mergedAt -q '.[] | "\(.number)\t\(.title)\t\([.labels[].name]|join(","))"'

# Fallback / cross-check — raw commit subjects in the range:
git log ${LAST:+$LAST..}HEAD --no-merges --format='%s'
```

Read the titles and labels. Map each change to a kind:
- **breaking** — removes/renames a command, a stored-data layout, or user-visible contract.
- **feature** — a new capability (issues labelled `enhancement`, "Add …", "neuer …").
- **fix** — a bug fix, correctness or polish (issues labelled `bug`, "Fix …", "… reparieren").
- **chore/docs** — internal only; counts toward the entry but not toward the bump.

## Step 3 — decide the SemVer bump

Pre-1.0 (current line is `0.y.z`) Werkbank treats the **minor** as the breaking slot, per
SemVer's 0.x convention — a stable 1.0 has not been declared:

| What merged | 0.y.z bump | ≥1.0.0 bump |
|---|---|---|
| any **breaking** change | `0.(y+1).0` | `(x+1).0.0` |
| one or more **features**, no breaking | `0.(y+1).0` | `x.(y+1).0` |
| only **fixes** / chore / docs | `0.y.(z+1)` | `x.y.(z+1)` |

"how many features/bugs" (Issue #105) sharpens, but does not override, this: when a release
bundles **many** features and fixes, say so in the entry and take the higher slot; a release
with a single tiny fix is a patch. State your reasoning in one line before bumping, e.g.
*"4 features + 6 fixes, no breaking → minor: 0.1.0 → 0.2.0."*

When the call is genuinely ambiguous (e.g. one borderline-breaking change), surface the two
candidate versions and ask the user which to cut — do not guess silently.

## Step 4 — stamp the version and write the CHANGELOG

Set the version in all four files with the helper (it validates SemVer and keeps
`Cargo.lock` in step):

```bash
.claude/skills/release-werkbank/scripts/setze-version.sh X.Y.Z
```

Then prepend an entry to `CHANGELOG.md` at the repo root (create the file with a
`# Änderungsprotokoll` heading if it does not exist). Keep-a-Changelog shape, German, newest
on top, ISO date, grouped by kind, each line ending with its PR/issue number:

```markdown
## [X.Y.Z] — JJJJ-MM-TT

_N Funktionen, M Korrekturen._

### Hinzugefügt
- Kurzbeschreibung der neuen Fähigkeit (#123)

### Behoben
- Was repariert wurde (#118)

### Geändert
- Verhaltensänderung (#120)
```

Use today's date from the environment context, not a guessed one. Omit empty groups. The
one-line `_N Funktionen, M Korrekturen._` italic summary is the "how many" the issue asks
for — keep it honest to what you counted.

## Step 5 — verify, commit, tag

```bash
cd app && pnpm check && cd ..            # frontend still typechecks with the new version
git add -A
git commit -m "Release vX.Y.Z"
git tag -a vX.Y.Z -m "Werkbank vX.Y.Z"
```

Show the user the diff and the new CHANGELOG entry. **Do not push the commit or tag unless
the user asks** — pushing a tag is hard to take back. Tell them the push command:
`git push origin main vX.Y.Z`.

## Guardrails

- Refuse to release with a dirty working tree or off `main` unless the user insists.
- Never invent a version that is not strictly greater than the current one.
- If `gh` is unavailable (headless/no auth), fall back to `git log` subjects and say so in
  the entry rather than producing a thin CHANGELOG silently.
