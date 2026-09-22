//! Chain Study tests — synthetic fixtures only.
//!
//! Every fixture here is generated in-memory from the locked schema. No
//! proprietary chain content, no external datasets, no network. The fixture
//! graph covers a small, fully-audited corner of scripture chosen so each
//! required behavior is independently observable.

#![cfg(test)]

use super::*;
use crate::canon::schema_ext::{CANON_EXT_COLUMNS, CANON_EXT_TABLES};
use crate::provenance::ProvenanceClass;
use crate::schema::{CANON_SCHEMA, PASTOR_SCHEMA};
use rusqlite::Connection;

// ── Fixture builders ─────────────────────────────────────────────────────────

/// Install the bible-books rows (needed by every verse join).
fn seed_books(conn: &Connection) {
    for (num, osis, name, testament) in crate::books::BOOKS {
        conn.execute(
            "INSERT INTO bible_books(book_num, osis_id, name, testament, canonical_order)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![num, osis, name, testament, num],
        )
        .unwrap();
    }
}

/// Insert a verse; returns its canon id.
fn verse(conn: &Connection, book: i64, chapter: i64, num: i64, text: &str) -> i64 {
    conn.execute(
        "INSERT INTO bible_verses(book_num, chapter, verse, text_kjv) VALUES (?1,?2,?3,?4)",
        rusqlite::params![book, chapter, num, text],
    )
    .unwrap();
    conn.query_row(
        "SELECT id FROM bible_verses WHERE book_num=?1 AND chapter=?2 AND verse=?3",
        rusqlite::params![book, chapter, num],
        |r| r.get(0),
    )
    .unwrap()
}

/// Register the default (approved) canon sources, exactly as the builder does.
fn register_default_sources(conn: &Connection) {
    for adapter in crate::canon::adapters::default_registry() {
        adapter.source().upsert(conn).unwrap();
    }
}

/// An approved-registry-shape source id that is NOT in the engine allow-list —
/// simulates any dataset whose provenance the engine must refuse (proprietary
/// chains, research packets, AI output). It is registered in `sources` (like
/// any imported dataset) but never admitted by the engine.
const REJECTED_SOURCE_ID: &str = "unapproved-fixture-source";

fn apply_ext_columns(conn: &Connection) {
    for (table, column, decl) in CANON_EXT_COLUMNS {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {decl};");
        // Ignore "duplicate column" errors when called twice.
        let _ = conn.execute_batch(&sql);
    }
}

fn fresh_canon() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(CANON_SCHEMA).unwrap();
    conn.execute_batch(CANON_EXT_TABLES).unwrap();
    apply_ext_columns(&conn);
    seed_books(&conn);
    register_default_sources(&conn);
    conn
}

fn fresh_pastor() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(PASTOR_SCHEMA).unwrap();
    conn
}

/// The shared fixture graph.
///
/// Seed: John 6:35 (book 43).
///
/// ```text
/// depth 1:  John 6:37 (xref rank 60)      John 6:44 (xref 50 + rule edge 1.0)
///           Matthew 6:33 (xref 60 — pure tie with John 6:37)
/// depth 2:  John 6:47 (xref 30 via John 6:37)
///           Romans 8:30 (xref 20 via John 6:44)
/// depth 3 (out of bounds): Romans 8:31 (xref 10 via Romans 8:30)
/// rule edge: John 6:35 --concordance 1.0--> John 6:44
///            rule_id = chain-study-1.0/concordance-fallback
/// rejected:  John 6:35 --proprietary-chain--> Romans 8:28 (unapproved source)
/// length bound: 20 low-rank (5) filler neighbors Revelation 1:1..1:20
/// ```
struct Fixture {
    canon: Connection,
    pastor: Connection,
}

fn build_fixture() -> Fixture {
    let canon = fresh_canon();
    let j635 = verse(&canon, 43, 6, 35, "I am that bread of life.");
    let j637 = verse(&canon, 43, 6, 37, "All that the Father giveth me shall come to me;");
    let j644 = verse(&canon, 43, 6, 44, "No man can come to me, except the Father…");
    let m633 = verse(&canon, 40, 6, 33, "For the bread of God is he which cometh down…");
    let j647 = verse(&canon, 43, 6, 47, "Verily, verily, I say unto you, He that believeth…");
    let r830 = verse(&canon, 45, 8, 30, "Moreover whom he did predestinate, them he also called…");
    let r831 = verse(&canon, 45, 8, 31, "What shall we then say to these things?");
    let r828 = verse(&canon, 45, 8, 28, "And we know that all things work together for good…");

    let xref = |from: i64, to: i64, rank: i64, source: Option<&str>| {
        canon
            .execute(
                "INSERT INTO cross_references(from_verse_id, to_verse_id, rank, source_id)
                 VALUES (?1,?2,?3,?4)",
                rusqlite::params![from, to, rank, source],
            )
            .unwrap();
    };

    // Depth-1 (direct). John 6:37 and Matthew 6:33 are a pure rank-60 tie;
    // canonical order must decide their relative order.
    xref(j635, j637, 60, None); // legacy row → DEFAULT_XREF_SOURCE_ID
    xref(j635, m633, 60, Some("openbible-xrefs"));
    xref(j635, j644, 50, Some("openbible-xrefs"));
    // Depth-2.
    xref(j637, j647, 30, Some("openbible-xrefs"));
    xref(j644, r830, 20, Some("openbible-xrefs"));
    // Depth-3 — beyond MAX_SEARCH_DEPTH.
    xref(r830, r831, 10, Some("openbible-xrefs"));

    // Rule edge (explicit, versioned, approved source).
    canon
        .execute(
            "INSERT INTO chain_edges(from_verse, to_verse, kind, weight, source_id, rule_id)
             VALUES (?1,?2,'concordance',1.0,'openbible-xrefs','chain-study-1.0/concordance-fallback')",
            rusqlite::params![j635, j644],
        )
        .unwrap();

    // REJECTED edge: registered in `sources` but not on the approved
    // allow-list (the proprietary-chain shape). Must never surface.
    canon
        .execute(
            "INSERT INTO sources(id, name, license_code, attribution)
             VALUES (?1,'Fixture proprietary chain','ALL RIGHTS RESERVED','not admitted')",
            rusqlite::params![REJECTED_SOURCE_ID],
        )
        .unwrap();
    canon
        .execute(
            "INSERT INTO chain_edges(from_verse, to_verse, kind, weight, source_id, rule_id)
             VALUES (?1,?2,'proprietary-chain',1.0,?3,NULL)",
            rusqlite::params![j635, r828, REJECTED_SOURCE_ID],
        )
        .unwrap();

    // Length-bound fillers: 20 direct neighbors at low rank.
    for i in 1..=20 {
        let vid = verse(&canon, 66, 1, i, "filler verse");
        xref(j635, vid, 5, Some("openbible-xrefs"));
    }

    // Topics (sourced + unsourced/unapproved).
    for (id, name, source) in [
        ("top-bread-of-life", "Bread of Life", "naves-topical"),
        ("top-calling", "Effectual Calling", "torrey-topical"),
        ("top-unapproved", "Should Never Appear", REJECTED_SOURCE_ID),
    ] {
        canon
            .execute(
                "INSERT INTO topics(id, name, source_id) VALUES (?1,?2,?3)",
                rusqlite::params![id, name, source],
            )
            .unwrap();
    }
    let tv = |topic: &str, source: &str, vid: i64| {
        canon
            .execute(
                "INSERT INTO topic_verses(topic_id, verse_id, weight, source_id)
                 VALUES (?1,?2,1.0,?3)",
                rusqlite::params![topic, vid, source],
            )
            .unwrap();
    };
    // Bread of Life: seed + two chain members (supported by both rules).
    tv("top-bread-of-life", "naves-topical", j635);
    tv("top-bread-of-life", "naves-topical", j637);
    tv("top-bread-of-life", "naves-topical", j647);
    // Effectual Calling: exactly ONE chain member, not the seed → below
    // MIN_TOPIC_SUPPORT on both rules.
    tv("top-calling", "torrey-topical", r830);
    // Unapproved-source topic touching the seed → must never be labeled.
    tv("top-unapproved", REJECTED_SOURCE_ID, j635);

    // Pastor archive: two sermons touching chain verses; one touching only a
    // non-chain verse (must never appear).
    let pastor = fresh_pastor();
    let sermon = |id: &str, title: &str, passage: &str| {
        pastor
            .execute(
                "INSERT INTO sermon_index(id, file_path, file_hash, title, primary_passage, big_idea, structure_type)
                 VALUES (?1,?2,'hash',?3,?4,'idea','inverting')",
                rusqlite::params![id, format!("/vault/{id}.md"), title, passage],
            )
            .unwrap();
    };
    sermon("sermon-b", "He Comes to Me", "John 6:35-John 6:47");
    sermon("sermon-a", "Called and Kept", "Romans 8:28-Romans 8:30");
    sermon("sermon-c", "Unrelated", "Jude 1:1-Jude 1:25");
    let link = |sermon_id: &str, book: i64, ch: i64, v: i64| {
        pastor
            .execute(
                "INSERT INTO scripture_sermon_links(sermon_id, book_num, chapter, verse)
                 VALUES (?1,?2,?3,?4)",
                rusqlite::params![sermon_id, book, ch, v],
            )
            .unwrap();
    };
    link("sermon-b", 43, 6, 35);
    link("sermon-b", 43, 6, 37);
    link("sermon-a", 45, 8, 30);
    link("sermon-c", 65, 1, 1); // Jude 1:1 — not a chain verse.

    Fixture { canon, pastor }
}

/// Human reference for fixture asserts — mirrors the engine's formatting.
fn human(book_num: i64, chapter: i64, verse_num: i64) -> String {
    let name = crate::books::BOOKS
        .iter()
        .find(|b| b.0 == book_num)
        .map(|b| b.2)
        .unwrap_or("?");
    format!("{} {}:{}", name, chapter, verse_num)
}

// ── 1. Determinism: same seed + data ⇒ byte-identical result ────────────────

#[test]
fn same_seed_produces_byte_identical_results() {
    let f = build_fixture();
    let a = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let b = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb, "two runs over identical data must be byte-identical");

    // Also across a fresh connection to a rebuilt (content-identical) fixture.
    let f2 = build_fixture();
    let c = chain_study(&f2.canon, Some(&f2.pastor), "John 6:35").unwrap();
    assert_eq!(ja, serde_json::to_string(&c).unwrap());

    assert_eq!(a.engine_version, "chain-study-1.0");
    assert_eq!(a.seed_reference, "John.6.35");
    assert!(a.canon_available);
}

// ── 2. Stable tie-breaking: canonical order decides equal scores ─────────────

#[test]
fn equal_scores_tie_break_by_canonical_order() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let chain = &result.chains[0];

    let m633 = chain
        .references
        .iter()
        .find(|r| r.reference == human(40, 6, 33))
        .expect("Matthew 6:33 (direct xref 60) in chain");
    let j637 = chain
        .references
        .iter()
        .find(|r| r.reference == human(43, 6, 37))
        .expect("John 6:37 (direct xref 60) in chain");
    assert_eq!(
        m633.weight, j637.weight,
        "fixture guarantees a pure tie (both rank 60)"
    );
    let mi = chain
        .references
        .iter()
        .position(|r| r.reference == m633.reference)
        .unwrap();
    let ji = chain
        .references
        .iter()
        .position(|r| r.reference == j637.reference)
        .unwrap();
    assert!(
        mi < ji,
        "equal scores must tie-break to canonical order: Matthew(40) before John(43)"
    );
}

#[test]
fn full_reference_ordering_is_weight_desc_then_canonical() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let chain = &result.chains[0];
    let key = |r: &ChainReference| {
        // (negative weight in milli, canonical book/chapter/verse) — ascending
        // sort key that realizes (weight DESC, canonical ASC).
        let milli = (r.weight * 1000.0).round() as i64;
        let (name, rest) = r.reference.split_once(' ').unwrap();
        let (chapter, verse_num) = rest.split_once(':').unwrap();
        let (book_num, chapter_i, verse_i) = crate::books::BOOKS
            .iter()
            .find(|b| b.2 == name)
            .map(|b| (b.0, chapter.parse::<i64>().unwrap_or(0), verse_num.parse::<i64>().unwrap_or(0)))
            .unwrap_or((999, 0, 0));
        (std::cmp::Reverse(milli), book_num, chapter_i, verse_i)
    };
    let mut keys: Vec<_> = chain.references.iter().map(key).collect();
    keys.sort();
    let rendered: Vec<_> = chain.references.iter().map(key).collect();
    assert_eq!(rendered, keys, "engine order must be (weight DESC, canonical ASC)");
}

// ── 3. Every edge has source (or rule) provenance ────────────────────────────

#[test]
fn every_edge_carries_traceable_provenance() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    for chain in &result.chains {
        // Every reference carries biblical-study provenance from a registered,
        // approved source.
        for r in &chain.references {
            assert_eq!(r.provenance.class, ProvenanceClass::BiblicalStudy);
            let sid = r.provenance.source_id.as_deref().expect("source_id present");
            assert!(source_is_approved(sid), "unapproved source {sid}");
            let n: i64 = f
                .canon
                .query_row(
                    "SELECT COUNT(*) FROM sources WHERE id=?1",
                    rusqlite::params![sid],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "source {sid} must be registered in sources");
        }
        // Every evidence item is traceable: non-empty value, biblical-study
        // class, and rule edges carry the versioned rule_id (or explicit
        // source: fallback).
        for e in &chain.evidence {
            assert!(!e.value.is_empty(), "evidence must be traceable");
            if e.kind == "sourced-topic" {
                assert!(chain.source_topics.contains(&e.label));
                assert_eq!(e.provenance.class, ProvenanceClass::BiblicalStudy);
            } else {
                assert_eq!(e.provenance.class, ProvenanceClass::BiblicalStudy);
                assert!(e.provenance.source_id.is_some());
                if e.kind == "rule-edge" {
                    assert!(
                        e.value.starts_with("chain-study-1.0/") || e.value.starts_with("source:"),
                        "rule edge must trace to a versioned rule_id or registered source"
                    );
                }
            }
        }
        // Every reference is explained: the chain carries at least one
        // connection evidence per reference.
        let connection_evidence = chain
            .evidence
            .iter()
            .filter(|e| e.kind != "sourced-topic")
            .count();
        assert!(
            connection_evidence >= chain.references.len(),
            "each reference needs at least one connection explanation"
        );
    }
}

// ── 4. Source topic names only ────────────────────────────────────────────────

#[test]
fn topic_names_come_only_from_sourced_records() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let chain = &result.chains[0];

    // The approved sourced topic is present with its registry name.
    assert!(chain.source_topics.contains(&"Bread of Life".to_string()));
    assert_eq!(chain.name.as_deref(), Some("Bread of Life"));

    // The topic whose membership rows come from an unapproved source must
    // never be labeled, even though it touches the seed verse.
    assert!(
        !chain.source_topics.contains(&"Should Never Appear".to_string()),
        "unapproved-source topics must never be labeled"
    );

    // Effectual Calling has only one chain member and not the seed → below
    // MIN_TOPIC_SUPPORT, so it must not be associated.
    assert!(!chain.source_topics.contains(&"Effectual Calling".to_string()));

    // Every emitted topic name exists verbatim in the topics table.
    for name in &chain.source_topics {
        let n: i64 = f
            .canon
            .query_row(
                "SELECT COUNT(*) FROM topics WHERE name=?1",
                rusqlite::params![name],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "topic name {name} must come from the topics table");
    }
}

// ── 5. Bounded depth ─────────────────────────────────────────────────────────

#[test]
fn graph_walk_is_bounded_by_max_search_depth() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let chain = &result.chains[0];

    // Depth-2 members are present…
    assert!(chain.references.iter().any(|r| r.reference == human(43, 6, 47)));
    assert!(chain.references.iter().any(|r| r.reference == human(45, 8, 30)));
    // …every member is within MAX_SEARCH_DEPTH…
    assert!(chain.references.iter().all(|r| r.distance <= MAX_SEARCH_DEPTH));
    // …and distances are honest (direct vs second-hop).
    assert_eq!(
        chain
            .references
            .iter()
            .find(|r| r.reference == human(43, 6, 37))
            .unwrap()
            .distance,
        1
    );
    assert_eq!(
        chain
            .references
            .iter()
            .find(|r| r.reference == human(45, 8, 30))
            .unwrap()
            .distance,
        2
    );
    // Romans 8:31 is only reachable at depth 3 → excluded.
    assert!(
        !chain.references.iter().any(|r| r.reference == human(45, 8, 31)),
        "depth-3 verses must be excluded"
    );
}

// ── 6. Bounded result length ─────────────────────────────────────────────────

#[test]
fn chain_length_is_bounded() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let chain = &result.chains[0];
    assert!(chain.references.len() <= MAX_CHAIN_REFERENCES);
    assert!(result.chains.len() <= MAX_CHAINS);
    assert!(result.archive_connections.len() <= MAX_ARCHIVE_CONNECTIONS);

    // The fixture offers 23 candidates (3 study neighbors + 20 low-rank
    // fillers at depth 1, then 2 more at depth 2); the per-family neighbor
    // bound admits only the top 6 xref edges at the seed, so the low-rank
    // fillers must be dropped in favor of the depth-2 study verses.
    assert!(chain.references.len() <= MAX_CHAIN_REFERENCES);
    for r in &chain.references {
        assert!(
            !r.reference.starts_with("Revelation 1:")
                || r.distance == 1 && r.weight < 0.05,
            "filler verses must not outrank study verses"
        );
    }
    // The depth-2 study verses made the cut despite the filler crowd.
    assert!(chain.references.iter().any(|r| r.reference == human(43, 6, 47)));
    assert!(chain.references.iter().any(|r| r.reference == human(45, 8, 30)));
}

// ── 7. Missing canon.db graceful ─────────────────────────────────────────────

#[test]
fn missing_or_stale_canon_is_calm_unavailable() {
    // Empty schema (no extension, no data) — the "no canon.db" shape.
    let empty = Connection::open_in_memory().unwrap();
    empty.execute_batch(CANON_SCHEMA).unwrap();
    assert!(!canon_ready(&empty));
    let result = chain_study(&empty, None, "John 6:35").unwrap();
    assert!(!result.canon_available);
    assert!(result.chains.is_empty());
    assert!(result.archive_connections.is_empty());
    assert_eq!(result.engine_version, ENGINE_VERSION);

    // Data present but pre-extension (missing source_id column) → unavailable.
    let stale = Connection::open_in_memory().unwrap();
    stale.execute_batch(CANON_SCHEMA).unwrap();
    seed_books(&stale);
    let v = verse(&stale, 43, 6, 35, "…");
    stale
        .execute(
            "INSERT INTO cross_references(from_verse_id, to_verse_id, rank) VALUES (?1, ?1, 1)",
            rusqlite::params![v],
        )
        .unwrap();
    assert!(!canon_ready(&stale), "pre-extension canon must read unavailable");
    let result = chain_study(&stale, None, "John 6:35").unwrap();
    assert!(!result.canon_available);
    assert!(result.chains.is_empty());

    // Unresolvable seed over a healthy canon → honest empty result, no error.
    let f = build_fixture();
    let result = chain_study(&f.canon, None, "not a reference").unwrap();
    assert!(result.chains.is_empty());

    // Resolvable seed with no verse row → empty, no error.
    let result = chain_study(&f.canon, None, "Genesis 1:1").unwrap();
    assert!(result.chains.is_empty());
}

// ── 8. Sermon archive overlay correct ────────────────────────────────────────

#[test]
fn archive_overlay_matches_only_chain_verses_and_never_mutates_the_chain() {
    let f = build_fixture();

    let with = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let without = chain_study(&f.canon, None, "John 6:35").unwrap();

    // The overlay must not mutate the biblical chain.
    assert_eq!(with.chains, without.chains, "archive overlay is additive only");
    assert_eq!(with.canon_available, without.canon_available);

    // Connections exist and carry the archive's own IDs.
    let ids: Vec<&str> = with
        .archive_connections
        .iter()
        .map(|c| c.sermon_id.as_str())
        .collect();
    assert!(ids.contains(&"sermon-a"));
    assert!(ids.contains(&"sermon-b"));
    assert!(!ids.contains(&"sermon-c"), "non-chain sermons must not appear");
    // Stable ordering by existing sermon id.
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);

    let b = with
        .archive_connections
        .iter()
        .find(|c| c.sermon_id == "sermon-b")
        .unwrap();
    assert_eq!(b.title, "He Comes to Me");
    assert!(b.matching_references.contains(&human(43, 6, 37)));

    let a = with
        .archive_connections
        .iter()
        .find(|c| c.sermon_id == "sermon-a")
        .unwrap();
    assert!(a.matching_references.contains(&human(45, 8, 30)));

    // Every provenance is your-archive with the same sermon id.
    for c in &with.archive_connections {
        assert_eq!(c.provenance.class, ProvenanceClass::YourArchive);
        assert_eq!(c.provenance.sermon_id.as_deref(), Some(c.sermon_id.as_str()));
    }
}

// ── 9. Research Packet provenance rejected ───────────────────────────────────

#[test]
fn research_packet_provenance_is_rejected_everywhere() {
    // The admission guard (mirrors intelligence::admit_evidence) admits only
    // biblical-study class for chain edges.
    let mk = |class| ChainEvidence {
        kind: "test".into(),
        label: "test".into(),
        value: "test".into(),
        weight: 1.0,
        provenance: crate::provenance::Provenance {
            class,
            source_id: None,
            source_label: "x".into(),
            source_version: None,
            license_code: None,
            attribution: None,
            sermon_id: Some("s".into()),
            attachment_id: None,
            page: None,
            engine_version: None,
        },
    };
    assert!(admit_edge_evidence(mk(ProvenanceClass::BiblicalStudy)).is_some());
    for class in [
        ProvenanceClass::ResearchPacket,
        ProvenanceClass::SermonIntelligence,
        ProvenanceClass::Armarius,
        ProvenanceClass::YourArchive,
    ] {
        assert!(
            admit_edge_evidence(mk(class)).is_none(),
            "{class:?} must never become chain evidence"
        );
    }

    // End-to-end: a dataset registered in `sources` but not on the approved
    // allow-list (the research-packet shape) never surfaces.
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let classes = result.chains.iter().flat_map(|c| {
        c.references
            .iter()
            .map(|r| &r.provenance.class)
            .chain(c.evidence.iter().map(|e| &e.provenance.class))
    });
    for class in classes {
        assert_eq!(*class, ProvenanceClass::BiblicalStudy);
    }
}

// ── 10. No AI / no network ───────────────────────────────────────────────────

#[test]
fn engine_emits_no_ai_provenance_and_declares_its_version() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    for chain in &result.chains {
        for e in &chain.evidence {
            assert_ne!(e.provenance.class, ProvenanceClass::SermonIntelligence);
            assert_ne!(e.provenance.class, ProvenanceClass::Armarius);
            assert!(e.provenance.engine_version.is_none());
        }
    }
    assert_eq!(result.engine_version, "chain-study-1.0");
    assert_eq!(result.parameters, ChainStudyParameters::engine_defaults());
    assert_eq!(result.parameters.max_search_depth, MAX_SEARCH_DEPTH);
    assert_eq!(
        result.parameters.max_chain_references,
        MAX_CHAIN_REFERENCES
    );
}

// ── 11. No proprietary Thompson content ──────────────────────────────────────

#[test]
fn no_proprietary_thompson_source_is_registered_or_admitted() {
    // The canon source registry contains no Thompson dataset.
    let registry = crate::canon::adapters::default_registry();
    for a in &registry {
        let s = a.source();
        assert!(
            !s.id.to_lowercase().contains("thompson")
                && !s.name.to_lowercase().contains("thompson"),
            "proprietary chain source in registry: {}",
            s.id
        );
    }
    // The engine allow-list is PD/CC-BY only and names its four datasets.
    assert_eq!(
        APPROVED_SOURCE_IDS,
        ["kjv-pd", "openbible-xrefs", "naves-topical", "torrey-topical"]
    );
    for id in APPROVED_SOURCE_IDS {
        assert!(!id.to_lowercase().contains("thompson"));
    }

    // End-to-end: a Thompson-like edge injected into canon (registered in
    // `sources`, weighted above everything else) never reaches the result.
    let f = build_fixture();
    let r828 = f
        .canon
        .query_row(
            "SELECT id FROM bible_verses WHERE book_num=45 AND chapter=8 AND verse=28",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    f.canon
        .execute(
            "UPDATE chain_edges SET weight = 10.0 WHERE to_verse = ?1 AND kind = 'proprietary-chain'",
            rusqlite::params![r828],
        )
        .unwrap();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    let chain = &result.chains[0];
    assert!(
        !chain
            .references
            .iter()
            .any(|r| r.reference == human(45, 8, 28)),
        "the proprietary-fixture edge target must not appear"
    );
    for e in &chain.evidence {
        assert!(!e.label.to_lowercase().contains("thompson"));
        assert!(!e.kind.to_lowercase().contains("proprietary"));
        assert_ne!(e.provenance.source_id.as_deref(), Some(REJECTED_SOURCE_ID));
    }
}

// ── 12. Navigation uses existing sermon IDs ──────────────────────────────────

#[test]
fn archive_connections_carry_existing_sermon_ids() {
    let f = build_fixture();
    let result = chain_study(&f.canon, Some(&f.pastor), "John 6:35").unwrap();
    assert!(!result.archive_connections.is_empty());
    for c in &result.archive_connections {
        // The id must be a real sermon_index id (the UI navigates with it).
        let n: i64 = f
            .pastor
            .query_row(
                "SELECT COUNT(*) FROM sermon_index WHERE id=?1",
                rusqlite::params![c.sermon_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "sermon id {} must exist in the archive", c.sermon_id);
        assert!(!c.title.is_empty());
        assert!(!c.primary_passage.is_empty());
        assert!(!c.matching_references.is_empty());
    }
}

// ── Constants self-test (bounds are the documented contract) ────────────────

#[test]
fn bounding_constants_are_the_documented_frozen_values() {
    assert_eq!(ENGINE_VERSION, "chain-study-1.0");
    assert_eq!(MAX_SEARCH_DEPTH, 2);
    assert_eq!(MAX_NEIGHBORS_PER_NODE, 6);
    assert_eq!(MAX_CHAIN_REFERENCES, 12);
    assert_eq!(MAX_CHAINS, 5);
    assert_eq!(MAX_ARCHIVE_CONNECTIONS, 10);
    assert_eq!(MIN_TOPIC_SUPPORT, 2);
    assert_eq!(MAX_CHAIN_TOPICS, 4);
    assert_eq!(W_XREF_RANK_MILLI, 600);
    assert_eq!(W_RULE_EDGE_MILLI, 300);
    assert_eq!(W_SOURCED_TOPIC_MILLI, 100);
    assert_eq!(DEFAULT_XREF_SOURCE_ID, "openbible-xrefs");
    // Scoring rule spot checks (integer math, no float drift).
    assert_eq!(xref_rank_milli(100), 600);
    assert_eq!(xref_rank_milli(60), 360);
    assert_eq!(xref_rank_milli(5), 30);
    assert_eq!(edge_weight_milli(1.0), 300);
    assert_eq!(edge_weight_milli(2.0), 300, "rule weights cap at 1.0");
}
