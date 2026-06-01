# Context — PLM-Werkzeug

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
