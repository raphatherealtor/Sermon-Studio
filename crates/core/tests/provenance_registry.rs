//! Tests for the provenance source registry fixture.
//!
//! The fixture lists representative study sources with their license code,
//! attribution string, source label, and a version placeholder. These tests
//! assert that each maps cleanly onto a [`Provenance`] DTO, is classified as
//! `biblical-study`, is **not** lifetime corpus, and serializes stably.

use serde::Deserialize;
use sermon_core::provenance::{is_lifetime_corpus, Provenance, ProvenanceClass};
use std::path::Path;

#[derive(Debug, Deserialize)]
struct SourceFixture {
    source_id: String,
    source_label: String,
    license_code: String,
    attribution: String,
    source_version: String,
}

fn load_fixture() -> Vec<SourceFixture> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/provenance_sources.json");
    let json = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&json).unwrap()
}

#[test]
fn fixture_has_the_expected_sources() {
    let sources = load_fixture();
    let ids: Vec<&str> = sources.iter().map(|s| s.source_id.as_str()).collect();
    for expected in [
        "stepbible-tagnt",
        "openbible-xrefs",
        "strongs-pd",
        "naves-topical",
        "torrey-topical",
    ] {
        assert!(ids.contains(&expected), "fixture missing {expected}");
    }
}

#[test]
fn every_source_maps_to_a_biblical_study_provenance() {
    for s in load_fixture() {
        let p = Provenance::biblical_study(
            s.source_id.clone(),
            s.source_label.clone(),
            s.license_code.clone(),
            s.attribution.clone(),
        );
        assert_eq!(p.class, ProvenanceClass::BiblicalStudy);
        assert!(!is_lifetime_corpus(&p), "study data is never lifetime corpus");
        assert_eq!(p.source_id.as_deref(), Some(s.source_id.as_str()));
        assert_eq!(p.license_code.as_deref(), Some(s.license_code.as_str()));
        assert!(!p.attribution.as_deref().unwrap().is_empty());
        assert!(!s.source_version.is_empty());
    }
}

#[test]
fn source_provenance_serializes_stably() {
    for s in load_fixture() {
        let p = Provenance::biblical_study(
            s.source_id.clone(),
            s.source_label.clone(),
            s.license_code.clone(),
            s.attribution.clone(),
        );
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"class\":\"biblical-study\""));
        assert!(json.contains(&format!("\"source_id\":\"{}\"", s.source_id)));
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}

#[test]
fn license_codes_are_from_the_expected_set() {
    for s in load_fixture() {
        assert!(
            matches!(s.license_code.as_str(), "PD" | "CC-BY-4.0" | "CC-BY-SA"),
            "unexpected license code {} for {}",
            s.license_code,
            s.source_id
        );
    }
}
