//! Honest git nouns — read-only visibility (Issue #135, E55).
//!
//! Since E43 the basic git vocabulary is *sichtbar und erlaubt*; only the gefährliche „Wie"-Mechanik
//! stays hidden. E55 makes the four honest git nouns the HW-Ingenieur may *see* — **Commit, Branch,
//! Tag, Push** — concrete on the existing graph/projection display (the `VersionTree` detail card),
//! WITHOUT making any of them operable. This is a pure string/visibility confirmation, so the test
//! reads the shipped Svelte source of the projection layer and asserts:
//!
//!   1. all four nouns are visibly named as read-only `tip-key` readouts, and
//!   2. nothing operable is added to the detail card — no input/button/recovery formula in it.
//!
//! It is the acceptance-criterion test for #135. There is no frontend test harness in this project;
//! the projection layer's wording is asserted here in Rust, beside the graph_projection tests.

use std::path::PathBuf;

/// The Graph-Raum / projection display the nouns are surfaced on (the existing read-only layer).
fn version_tree_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src")
        .join("lib")
        .join("VersionTree.svelte");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The detail card markup only — the read-only readout where the nouns live. Carved out so the
/// "nothing operable" assertion can't be fooled by the (correctly operable) promote dialog / verb
/// affordances that share the same file. The card is the `{#if tip}` block; it ends where the next
/// top-level block — the (operable) promote dialog `{#if promoting}` — begins.
fn detail_card(src: &str) -> &str {
    let start = src
        .find("{#if tip}")
        .expect("detail card opens with `{#if tip}`");
    let end = src[start..]
        .find("{#if promoting}")
        .map(|i| start + i)
        .expect("the promote dialog `{#if promoting}` follows the detail card");
    &src[start..end]
}

#[test]
fn all_four_honest_git_nouns_are_visibly_named() {
    let src = version_tree_source();
    let card = detail_card(&src);

    // Each noun appears as a read-only `tip-key` label in the detail card (E55). Commit and Branch
    // were already named since E43; Tag and Push are the two E55 adds.
    for noun in ["Commit", "Branch", "Tag", "Push"] {
        let key = format!(r#"<span class="tip-key label">{noun}</span>"#);
        assert!(
            card.contains(&key),
            "honest git noun `{noun}` must be visibly named as a read-only readout (E55)"
        );
    }
}

#[test]
fn the_detail_card_is_not_operable() {
    let src = version_tree_source();
    let card = detail_card(&src);

    // No place to type a git/recovery formula and no key to press: the card is a pure readout. The
    // operable handles (promote dialog, verb menu, gate) live elsewhere and stay walled off (E27/E38).
    for operable in ["<input", "<textarea", "<button", "onclick", "bind:value"] {
        assert!(
            !card.contains(operable),
            "the read-only detail card must contain no operable `{operable}` (E55)"
        );
    }

    // And it is honestly inert: the card frame disables pointer interaction outright.
    assert!(
        card.contains("pointer-events: none") || src.contains("pointer-events: none"),
        "the detail card must be a non-interactive readout"
    );
}

#[test]
fn no_raw_git_or_recovery_command_strings_leak_into_the_view() {
    // E55 keeps the existing domain wording consistent: the nouns are named, but the dangerous „Wie"-
    // mechanik stays hidden (E43). No raw conflict/command/recovery strings appear in the projection.
    let src = version_tree_source();
    for forbidden in [
        "reset --hard",
        "rebase",
        "cherry-pick",
        "reflog",
        "git push",
        "git commit",
        "<<<<<<<",
        ">>>>>>>",
    ] {
        assert!(
            !src.contains(forbidden),
            "raw git/recovery string `{forbidden}` must not appear in the projection view (E43/E55)"
        );
    }
}
