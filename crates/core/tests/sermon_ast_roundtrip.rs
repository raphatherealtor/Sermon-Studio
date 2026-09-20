//! Canonical round-trip integration tests (Track B).
//!
//! Markdown source → canonical Rust sermon representation → Markdown
//! serialization must preserve sermon meaning and archival information.
//! These tests exercise full fixture files on disk, complementing the
//! focused unit tests in `sermon.rs` and `directive.rs`.

use sermon_core::{Block, Sermon};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn archival_unknowns_round_trip_byte_exact() {
    let raw = fixture("archival_unknowns.md");
    let sermon = Sermon::parse(&raw).unwrap();
    let out = sermon.to_markdown();
    assert_eq!(
        out, raw,
        "a document whose directives are all unknown must round-trip byte-for-byte"
    );
    // And a second pass must not drift either.
    let again = Sermon::parse(&out).unwrap().to_markdown();
    assert_eq!(again, raw);
}

#[test]
fn archival_fixture_unknown_directives_identified() {
    let raw = fixture("archival_unknowns.md");
    let sermon = Sermon::parse(&raw).unwrap();
    let names: Vec<&str> = sermon
        .unknown_directives()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(names, vec!["custom-block", "bare-unknown", "weird"]);
    // No known directives in this fixture.
    assert_eq!(sermon.movements().count(), 0);
    assert_eq!(sermon.illustrations().count(), 0);
}

#[test]
fn canonical_full_fixture_semantic_round_trip() {
    let raw = fixture("canonical_full.md");
    let sermon = Sermon::parse(&raw).unwrap();

    // Metadata + Big Idea.
    assert_eq!(sermon.meta.title.as_deref(), Some("Abide in the Vine"));
    assert_eq!(
        sermon.big_idea(),
        Some("Remaining in Christ is the source of fruitfulness.")
    );

    // Three movements: manuscript, outline, combined.
    let movements: Vec<_> = sermon.movements().collect();
    assert_eq!(movements.len(), 3);
    assert!(movements[0].body.is_manuscript_style());
    assert_eq!(movements[0].warrant.as_deref(), Some("John 15:1-4"));
    assert!(movements[1].body.is_outline_style());
    assert!(movements[2].body.is_combined_style());

    // Illustration + application typed.
    assert_eq!(sermon.illustrations().count(), 1);
    assert_eq!(sermon.applications().count(), 1);

    // Exegetical notes are the only private block.
    let private: Vec<_> = sermon.private_notes().collect();
    assert_eq!(private.len(), 1);
    for block in &sermon.blocks {
        assert_eq!(block.is_private_notes(), matches!(block, Block::ExegeticalNotes(_)));
    }

    // Unknown directive survives verbatim inside a mixed document.
    let unknown = sermon.unknown_directives().next().unwrap();
    assert_eq!(unknown.name, "custom-block");
    let out = sermon.to_markdown();
    assert!(out.contains(unknown.raw_source.as_str()));

    // Round-trip is stable and meaning-preserving on the second pass.
    let pass1 = out;
    let sermon2 = Sermon::parse(&pass1).unwrap();
    let pass2 = sermon2.to_markdown();
    assert_eq!(pass1, pass2, "round-trip must be idempotent");
    assert_eq!(sermon2.movements().count(), 3);
    assert_eq!(
        sermon2.movements().next().unwrap().warrant.as_deref(),
        Some("John 15:1-4")
    );
    assert_eq!(sermon2.unknown_directives().count(), 1);
}
