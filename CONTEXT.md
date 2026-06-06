# Context — Werkbank

Glossary of the domain language. No implementation details — those live in code and in `design-docs/adr/`.

## Produkt
The top-level container, one per sellable end-product/project. Holds all development data in real folders on disk; the cloud is only a git remote (backup/multi-device/sharing), never a file store.

## Arbeitsbereich
A real leaf folder inside a Produkt (`elektronik/`, `mechanik/`, `firmware/`). The tool invents no second structure beside the filesystem. A parent folder with no rules is just a rule-less group ("Modul"), not a new object.

## Baustein
A reusable, per-tool bundle of tool knowledge (typically one per tool: KiCad, Fusion, Zephyr). Bundles, for one tool: Heimat-Ordner, Artefakt-Globs, Ignore-Presets, LFS-Muster, Öffnen-Aktion, optional Startaufgaben, and internal Default-Kanten. Defined once, reused across Produkte.

## Heimat-Ordner
The Arbeitsbereich a Baustein governs in a given Produkt (e.g. KiCad → `elektronik/`). Defaulted by the Bibliothek, resolvable per Produkt at onboarding.

## Bibliothek
The shared store of standard toolstacks and single Bausteine, living **outside** any Produkt. Source of templates only.

## Produkt-Stack
The set of Bausteine active in one Produkt, stored as a **copy** of the Bibliothek originals (anti-drift: a Bibliothek edit must never alter a running Produkt). Lives in the Produkt's `_plm` store.

## Sediment
When a Baustein is stillgelegt (label-only), its Ignore/LFS lines stay behind in the dotfiles as inert "sediment"; nothing is moved or deleted. Makes a tool swap nearly fully reversible.

## Waise
A tracked file that matches no Artefakt-Glob: it lies in the Unzugeordnet-Fach of its Arbeitsbereich (only the label is missing), so nothing is lost by omission and the folder context survives as an assignment hint.

## Konto
The single, app-wide identity used for every action against the self-hosted Forgejo/Gitea server: one Server-Adresse and one set of Zugangsdaten, set once and reused for all Produkte. There is exactly one Konto — the tool talks to one server. Owning a Konto is what lets a Produkt be veröffentlicht and a colleague invited; a Produkt names only its own owner/repo, never its own credentials.

## Baustein-Revision
A per-Baustein named release point with its own Art (Prototyp/Freigabe), bumped independently of the other Bausteine and minted as a durable Tag (`firmware/v1.1`). The Art rides here, not on the product as a whole. (E51)

## Produkt-Revision
A whole-tree product snapshot assembled from **one chosen release tag per Pflicht-Baustein** — a constructed snapshot whose tree matches the product BOM (e.g. new PCB Rev B + carried-over FW 1.1), with no Submodule. A WIP Baustein ships its previous release or is updated in time. Needs ≥1 release per Pflicht-Baustein; the first one seeds an initial Baustein-Revision from the current state. (E51, E52)

## Rekonstruierbar
A third Baustein path-class beside Ignore and LFS: paths deliberately **not tracked but reconstructable from a committed manifest** (Python `venv`, `west`/ESP-IDF modules, PlatformIO deps). The Baustein commits source + pinned manifest, not the vendored framework. A nested `.git`/Submodul is treated as an opaque, ignored boundary. (E50)

## Integrations-Aufgabe
An opt-in, cross-Baustein blocking Aufgabe ("needs fw test") raised against a specific source revision; the receiving Baustein answers yes/no (a "no" keeps the hard block). It blocks only the Produkt-Revision compose, never the target Baustein's own release, and is one-shot — the next source revision is re-flagged by hand, or not. (E53)

## Absichts-Sperre
A lock recorded **locally** when the Lock-Server is unreachable at open (offline, server down), reconciled against the server's locks on reconnect; a collision becomes a laute Ausnahme. Until confirmed, the card shows „offline bearbeitet, Sperre nicht bestätigt" rather than a silent all-clear. (E49)
