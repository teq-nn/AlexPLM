//! Manual "abgeleitet von" edges + the pure Stale-Warnung core (Issue #10).
//!
//! Edges are **opt-in** (E40): a product with zero edges is fully valid and produces no
//! warnings. A **Kante** records that one artifact „stammt aus" another — drawn by hand on
//! the artifact card (no heuristic, no fabricated graph — E21 stays parked). The only thing
//! the tool then claims is the **Stale-Warnung** (E26): a warning fires **iff** a manual edge
//! exists *and* the source artifact is newer than the derivation. **No edge = no warning.**
//!
//! As with `projection.rs`/`graph.rs`, the decision logic here is a **pure function**:
//! edge set + artifact timestamps in, warnings out — **no I/O**. The persistence glue lives
//! in [`crate::edgestore`]; everything testable lives here and is exercised by `#[cfg(test)]`
//! table tests (including the explicit negative "no edge = no warning" case).

use serde::{Deserialize, Serialize};

/// Woher eine Kante stammt — die drei Herkunftsstufen aus E20/Glossar (Issue #56). Reine Anzeige-
/// und Pflege-Information; sie ändert **nichts** an der Stale-Logik (E26 vergleicht nur Zeitstempel).
/// Sie erlaubt aber, Default-Kanten beim Stilllegen still in Ruhe zu schicken (E17) und in der UI
/// Hand- von Automatik-Kanten zu unterscheiden.
// Named `KantenHerkunft` (not just `Herkunft`) to stay distinct from the stackstore `Herkunft`
// copy-stamp — specta exports unique type names, and this matches the long-standing frontend name.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "kebab-case")]
pub enum KantenHerkunft {
    /// Von Hand gezogen (das echt Idiosynkratische) — der Default, auch für Altbestand ohne Feld.
    #[default]
    Hand,
    /// Baustein-Default: Kante **innerhalb** eines Bausteins (Gerber ← Layout), beim Onboarding angelegt.
    BausteinDefault,
    /// Baustein-Paar-Default: über zwei bekannt zusammengehörige Bausteine, per Klick bestätigt.
    PaarDefault,
}

/// A manual „abgeleitet von" edge: `derived` „stammt aus" `source`. Both are product-relative
/// artifact paths (the same identity the [`crate::projection::Baustein`] `path` carries).
///
/// Die `herkunft` trägt die Herkunftsstufe (E20). Alte `kanten.json` ohne das Feld lesen sich als
/// [`KantenHerkunft::Hand`] (serde-Default) — die Stale-Logik ist davon **unberührt**.
#[derive(specta::Type, Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    /// The derivation — the artifact that was made *from* `source`.
    pub derived: String,
    /// The source the derivation „stammt aus".
    pub source: String,
    /// Herkunftsstufe (E20). Fehlt sie auf der Platte, gilt `Hand` (Altbestand/Hand-Kante).
    #[serde(default)]
    pub herkunft: KantenHerkunft,
}

impl Edge {
    /// Eine Hand-Kante (die historische Konstruktor-Semantik — Default-Herkunft `Hand`).
    pub fn new(derived: impl Into<String>, source: impl Into<String>) -> Self {
        Edge {
            derived: derived.into(),
            source: source.into(),
            herkunft: KantenHerkunft::Hand,
        }
    }

    /// Eine Kante mit ausdrücklicher Herkunft (Baustein-/Paar-Default).
    pub fn with_herkunft(
        derived: impl Into<String>,
        source: impl Into<String>,
        herkunft: KantenHerkunft,
    ) -> Self {
        Edge {
            derived: derived.into(),
            source: source.into(),
            herkunft,
        }
    }

    /// Gleiche Endpunkte (Richtung), unabhängig von der Herkunft. Für Dedup über Herkunftsstufen.
    pub fn same_endpoints(&self, other: &Edge) -> bool {
        self.derived == other.derived && self.source == other.source
    }
}

/// An artifact's last-known change time, as a machine timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
/// Lexicographic order on that fixed shape is chronological, so we compare the strings
/// directly — the same convention `graph.rs` uses for newest-first ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStamp {
    /// Product-relative artifact path.
    pub path: String,
    /// Machine timestamp `YYYY-MM-DDTHH:MM:SSZ` of the artifact's newest Stand.
    pub timestamp: String,
}

/// A fired Stale-Warnung: the derivation along the edge it sits on is older than its source.
/// Serialized straight to the UI, which paints the derived card as „needs attention".
#[derive(specta::Type, Debug, Serialize, Clone, PartialEq, Eq)]
pub struct StaleWarning {
    /// The stale derivation (the artifact to re-check).
    pub derived: String,
    /// The source that moved on without it.
    pub source: String,
    /// Source timestamp (newer).
    pub source_timestamp: String,
    /// Derivation timestamp (older).
    pub derived_timestamp: String,
}

/// Compute the Stale-Warnungen for a set of manual edges over a snapshot of artifact
/// timestamps. **Pure**: no I/O, no clock — edge set + timestamps in, warnings out.
///
/// A warning fires for an edge **iff** both endpoints have a known timestamp **and** the
/// source is strictly newer than the derivation (E26). Consequences of the opt-in rule
/// (E40), all covered by tests:
/// - **No edge ⇒ no warning.** An empty edge set yields an empty result, regardless of
///   timestamps. The tool only claims what a human asserted by drawing the edge.
/// - An edge whose source is **not newer** (equal or older) yields no warning.
/// - An edge with a **missing** endpoint timestamp yields no warning — the tool can't
///   honestly compare what it doesn't know.
pub fn stale_warnings(edges: &[Edge], artifacts: &[ArtifactStamp]) -> Vec<StaleWarning> {
    let stamp_of = |path: &str| -> Option<&str> {
        artifacts
            .iter()
            .find(|a| a.path == path)
            .map(|a| a.timestamp.as_str())
    };

    edges
        .iter()
        .filter_map(|edge| {
            let source_ts = stamp_of(&edge.source)?;
            let derived_ts = stamp_of(&edge.derived)?;
            // Strictly newer source ⇒ the derivation is stale. Equal/older ⇒ fine.
            if source_ts > derived_ts {
                Some(StaleWarning {
                    derived: edge.derived.clone(),
                    source: edge.source.clone(),
                    source_timestamp: source_ts.to_string(),
                    derived_timestamp: derived_ts.to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Add a manual edge to `edges`, returning the new set. Pure set semantics:
/// - de-duplicates (drawing the same edge twice is a no-op);
/// - refuses a self-edge (an artifact cannot „stammen aus" itself) by returning the set
///   unchanged — the UI gesture should never offer it, and the core never fabricates one.
pub fn add_edge(mut edges: Vec<Edge>, edge: Edge) -> Vec<Edge> {
    if edge.derived == edge.source {
        return edges;
    }
    // Dedup über die **Endpunkte** (Richtung), nicht die exakte Herkunft: eine bereits vorhandene
    // Kante (egal welcher Herkunft) zwischen denselben Endpunkten ist ein No-op — eine Hand-Geste
    // auf eine bestehende Default-Kante fabriziert keine zweite Kante.
    if !edges.iter().any(|e| e.same_endpoints(&edge)) {
        edges.push(edge);
    }
    edges
}

/// Remove a manual edge from `edges`, returning the new set. Removing an absent edge is a
/// no-op. Pure — the inverse gesture to [`add_edge`]. Vergleich über die **Endpunkte**, sodass die
/// Lösch-Geste die Kante trifft, gleich welche Herkunft sie trägt.
pub fn remove_edge(mut edges: Vec<Edge>, edge: &Edge) -> Vec<Edge> {
    edges.retain(|e| !e.same_endpoints(edge));
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(path: &str, ts: &str) -> ArtifactStamp {
        ArtifactStamp {
            path: path.to_string(),
            timestamp: ts.to_string(),
        }
    }

    /// The explicit NEGATIVE acceptance test: zero edges ⇒ zero warnings, no matter how the
    /// artifacts' timestamps relate. The tool only claims what a human drew (E40).
    #[test]
    fn no_edge_means_no_warning() {
        let artifacts = vec![
            stamp("mechanik/gehaeuse", "2026-05-30T11:00:00Z"),
            stamp("fertigung/gehaeuse-stl", "2026-01-01T00:00:00Z"),
        ];
        // The source is far newer than the (would-be) derivation, but there is no edge.
        assert!(stale_warnings(&[], &artifacts).is_empty());
    }

    /// A warning fires iff a manual edge exists AND the source is newer than the derivation.
    /// Table over the edge + timestamp relation; the heart of E26.
    #[test]
    fn warning_iff_edge_and_source_newer() {
        // table: (source_ts, derived_ts, expect_warning)
        let cases: &[(&str, &str, bool)] = &[
            // source strictly newer -> stale
            ("2026-05-30T11:00:00Z", "2026-05-30T09:00:00Z", true),
            // source equal -> fine
            ("2026-05-30T09:00:00Z", "2026-05-30T09:00:00Z", false),
            // source older -> fine (derivation already accounts for it)
            ("2026-05-30T08:00:00Z", "2026-05-30T09:00:00Z", false),
            // one second newer is still newer -> stale
            ("2026-05-30T09:00:01Z", "2026-05-30T09:00:00Z", true),
        ];
        for (src_ts, der_ts, expect) in cases {
            let edges = vec![Edge::new("derived/d", "source/s")];
            let artifacts = vec![stamp("source/s", src_ts), stamp("derived/d", der_ts)];
            let warnings = stale_warnings(&edges, &artifacts);
            assert_eq!(
                !warnings.is_empty(),
                *expect,
                "src={src_ts} der={der_ts} expect_warning={expect}"
            );
            if *expect {
                assert_eq!(warnings[0].derived, "derived/d");
                assert_eq!(warnings[0].source, "source/s");
                assert_eq!(warnings[0].source_timestamp, *src_ts);
                assert_eq!(warnings[0].derived_timestamp, *der_ts);
            }
        }
    }

    /// An edge whose endpoint has no known timestamp produces no warning — the tool won't
    /// compare what it cannot see.
    #[test]
    fn missing_endpoint_timestamp_produces_no_warning() {
        let edges = vec![Edge::new("derived/d", "source/s")];
        // Only the source is known; the derivation has no stamp.
        let only_source = vec![stamp("source/s", "2026-05-30T11:00:00Z")];
        assert!(stale_warnings(&edges, &only_source).is_empty());
        // Only the derivation is known; the source has no stamp.
        let only_derived = vec![stamp("derived/d", "2026-05-30T11:00:00Z")];
        assert!(stale_warnings(&edges, &only_derived).is_empty());
    }

    /// Each independent edge is judged on its own; warnings accumulate.
    #[test]
    fn multiple_edges_each_judged_independently() {
        let edges = vec![
            Edge::new("d1", "s1"), // stale
            Edge::new("d2", "s2"), // fine
        ];
        let artifacts = vec![
            stamp("s1", "2026-05-30T11:00:00Z"),
            stamp("d1", "2026-05-30T09:00:00Z"),
            stamp("s2", "2026-05-30T08:00:00Z"),
            stamp("d2", "2026-05-30T09:00:00Z"),
        ];
        let warnings = stale_warnings(&edges, &artifacts);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].derived, "d1");
    }

    #[test]
    fn add_edge_dedupes_and_refuses_self_edge() {
        let edges = add_edge(Vec::new(), Edge::new("d", "s"));
        assert_eq!(edges.len(), 1);
        // drawing the same edge again is a no-op
        let edges = add_edge(edges, Edge::new("d", "s"));
        assert_eq!(edges.len(), 1);
        // a self-edge is refused, set unchanged
        let edges = add_edge(edges, Edge::new("x", "x"));
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn remove_edge_is_the_inverse_and_tolerant_of_absent() {
        let edges = add_edge(Vec::new(), Edge::new("d", "s"));
        let edges = remove_edge(edges, &Edge::new("never", "there")); // no-op
        assert_eq!(edges.len(), 1);
        let edges = remove_edge(edges, &Edge::new("d", "s"));
        assert!(edges.is_empty());
    }

    /// Altbestand ohne `herkunft`-Feld liest sich als Hand-Kante (serde-Default) — kein Bruch der
    /// bestehenden `kanten.json` (Issue #56 baut auf #10 auf, ohne dessen Format zu brechen).
    #[test]
    fn legacy_edge_without_herkunft_reads_as_hand() {
        let legacy = r#"{ "derived": "fertigung/stl", "source": "mechanik/gehaeuse" }"#;
        let edge: Edge = serde_json::from_str(legacy).unwrap();
        assert_eq!(edge.herkunft, KantenHerkunft::Hand);
        // und eine neue Kante mit ausdrücklicher Herkunft rundtrippt
        let d = Edge::with_herkunft("a", "b", KantenHerkunft::BausteinDefault);
        let back: Edge = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    /// Dedup über Endpunkte, nicht Herkunft: eine Hand-Geste auf eine bestehende Default-Kante
    /// fabriziert keine zweite Kante; Löschen trifft die Kante unabhängig von ihrer Herkunft.
    #[test]
    fn add_and_remove_match_on_endpoints_across_herkunft() {
        let edges = add_edge(Vec::new(), Edge::with_herkunft("d", "s", KantenHerkunft::BausteinDefault));
        let edges = add_edge(edges, Edge::new("d", "s")); // gleiche Endpunkte, Hand -> No-op
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].herkunft, KantenHerkunft::BausteinDefault, "erste Kante bleibt");
        // Löschen über die (Hand-)Kante trifft die Default-Kante.
        let edges = remove_edge(edges, &Edge::new("d", "s"));
        assert!(edges.is_empty());
    }

    /// Stale-Warnung ist herkunfts-blind: eine Default-Kante warnt nach genau derselben Regel wie
    /// eine Hand-Kante (E26 vergleicht nur Zeitstempel). Belegt, dass die neuen Quellen die
    /// Warnung speisen, ohne ihre Logik zu ändern.
    #[test]
    fn stale_is_herkunft_blind() {
        let edges = vec![Edge::with_herkunft("d", "s", KantenHerkunft::BausteinDefault)];
        let artifacts = vec![
            stamp("s", "2026-05-30T11:00:00Z"),
            stamp("d", "2026-05-30T09:00:00Z"),
        ];
        let w = stale_warnings(&edges, &artifacts);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].derived, "d");
    }
}
