// Frontend-Spiegel der Baustein-Validierung (Issue #108). Die AUTORITÄT ist der reine Rust-Kern
// `baustein::validate_baustein` (src-tauri/src/baustein.rs); dieser Spiegel liefert nur sofortiges
// Live-Feedback im Editor. Beide müssen dieselben Regeln tragen (Handoff §1.9).
import type { Baustein, Oeffnen } from "$lib/types";

// Die generierten `Baustein`-Array-Felder sind optional (serde-Defaults). Der Editor füllt sie immer,
// daher arbeitet der Entwurf mit einer voll-erforderlichen Form — bindet sauber in die Leaf-Editoren
// und bleibt beim Speichern auf `Baustein` zuweisbar.
export type BausteinVoll = Required<Baustein>;

const UMLAUT: Record<string, string> = { ä: "ae", ö: "oe", ü: "ue", ß: "ss" };

/** name → Kebab-Kennung, deutsche Umlaute transliteriert. Spiegelt die Anlege-Ableitung (Handoff §2). */
export function toKebab(name: string): string {
  return name
    .toLowerCase()
    .replace(/[äöüß]/g, (c) => UMLAUT[c] ?? c)
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/** Ein leerer Entwurf mit allen Array-Feldern populiert (Slice 3 nutzt das zum Anlegen). */
export function emptyBaustein(): BausteinVoll {
  return {
    id: "",
    version: 1,
    name: "",
    heimat: "",
    globs: [],
    ignore: [],
    lfs: [],
    oeffnen: "auto" as Oeffnen,
    startaufgaben: [],
    default_kanten: [],
    paar_default_kanten: [],
    stillgelegt: false,
  };
}

/** Ein Anlege-Entwurf aus einem bestehenden Baustein (Duplizieren, Slice 4 / Handoff §1.6). Läuft über
 *  DENSELBEN Anlege-Pfad wie „Neuer Baustein": `name` bekommt „ (Kopie)", die `id` wird aus dem neuen
 *  Namen neu abgeleitet (Kebab) — im Anlege-Modus prüft der Editor sie ohnehin auf Eindeutigkeit —,
 *  `version`=1 und `stillgelegt`=false; ALLES ANDERE wird wortwörtlich übernommen, inkl. der
 *  strukturierten Felder (startaufgaben, default_kanten, paar_default_kanten, globs …).
 *  Erwartet ein bereits entkoppeltes Objekt (der Aufrufer reicht `$state.snapshot(b)` herein — die
 *  $state-Rune gibt es nur in .svelte-Dateien, nicht in diesem reinen .ts-Modul). */
export function duplicateDraft(b: Baustein): BausteinVoll {
  const copy = structuredClone(b) as Baustein;
  const name = `${b.name} (Kopie)`;
  return {
    ...emptyBaustein(),
    ...copy,
    name,
    id: toKebab(name),
    version: 1,
    stillgelegt: false,
  };
}

export type FieldErrors = Partial<
  Record<"name" | "id" | "heimat" | "globs" | "default_kanten" | "paar_default_kanten", string>
>;

const KEBAB = /^[a-z0-9]+(-[a-z0-9]+)*$/;

/** Validierung (Handoff §1.9). Weich gehalten: baumelnde Partner-Refs warnen nur, blockieren nie.
 *  `isCreate=true` (Anlegen, Slice 3) schaltet die Eindeutigkeitsprüfung der Kennung ein; beim
 *  Bearbeiten ist die `id` unveränderlich und der Schreibpfad ein Upsert, daher false. Spiegelt den
 *  reinen Rust-Kern `validate_baustein(b, existing_ids, is_create)` (Autorität). */
export function validate(
  b: Baustein,
  all: Baustein[],
  isCreate: boolean,
): { errors: FieldErrors; warnings: string[] } {
  const errors: FieldErrors = {};
  const warnings: string[] = [];

  if (!b.name.trim()) errors.name = "Name darf nicht leer sein";
  if (!b.heimat.trim()) errors.heimat = "Heimat ist erforderlich";

  if (!b.id) errors.id = "Kennung darf nicht leer sein";
  else if (!KEBAB.test(b.id)) errors.id = "Nur Kleinbuchstaben, Ziffern, Bindestriche";
  else if (isCreate && all.some((x) => x.id === b.id)) errors.id = "Kennung schon vergeben";

  const liveGlobs = b.globs.map((g) => g.trim()).filter(Boolean);
  if (liveGlobs.length === 0) errors.globs = "Mindestens ein Artefakt-Muster";

  for (const k of b.default_kanten ?? []) {
    if (!k.derived_glob.trim() || !k.source_glob.trim()) {
      errors.default_kanten = "Default-Kanten brauchen Quelle und Ableitung";
      break;
    }
  }

  for (const k of b.paar_default_kanten ?? []) {
    if (!k.derived_glob.trim() || !k.source_glob.trim()) {
      errors.paar_default_kanten = "Paar-Kanten brauchen Quelle und Ableitung";
      break;
    }
    if (!k.partner_id.trim()) {
      errors.paar_default_kanten = "Paar-Kanten brauchen einen Partner-Baustein";
      break;
    }
    if (k.partner_id === b.id) {
      errors.paar_default_kanten = "Ein Baustein kann nicht sein eigener Partner sein";
      break;
    }
  }

  for (const k of b.paar_default_kanten ?? []) {
    const pid = k.partner_id.trim();
    if (pid && pid !== b.id && !all.some((x) => x.id === pid)) {
      warnings.push(
        `Partner „${pid}“ liegt nicht in der Bibliothek — der Vorschlag greift erst, wenn er existiert.`,
      );
    }
  }

  return { errors, warnings };
}
