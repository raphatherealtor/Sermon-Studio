//! # THE CONSTITUTIONAL TEST
//!
//! This test proves the product's central invariant end-to-end with a sentinel
//! phrase: **`RESEARCH_PACKET_SENTINEL_9F3A7`**.
//!
//! The sentinel is deliberately placed inside a research-packet attachment's
//! extracted text. The test then proves, in order, that the sentinel:
//!
//! 1. **IS** stored in the packet metadata / extracted text (so the fixture is
//!    real and the test is not vacuous);
//! 2. is **NOT** lifetime corpus (`is_lifetime_corpus` is false for packets);
//! 3. is **NOT** in pastor-authored content (packet paths are not indexable);
//! 4. is **NOT** in the pastor-authored FTS index (a real `pastor.db` FTS5
//!    search returns zero rows for the sentinel while a control token matches);
//! 5. is **NOT** eligible as Sermon Intelligence evidence — proven against the
//!    **real Track J engine** ([`related_sermons`], [`sermon_insights`],
//!    [`passage_history`]) running over a real `pastor.db`, plus the engine's
//!    own admission guard ([`admit_evidence`]).
//!
//! Step 5 is the reconciliation upgrade over the V1 scaffold: instead of only
//! exercising an isolated fake predicate, the test runs the authoritative engine
//! and asserts that (a) no evidence it emits contains the sentinel, (b) every
//! evidence item it emits is `your-archive` (lifetime corpus), and (c) the
//! engine's real admission guard drops packet-derived evidence.

use rusqlite::Connection;
use sermon_core::intelligence::{
    admit_evidence, passage_history, related_sermons, sermon_insights, Evidence, Insight,
};
use sermon_core::provenance::{is_lifetime_corpus, Provenance, ProvenanceClass};
use sermon_core::research_packet::{
    is_indexable_as_pastor_content, is_intelligence_eligible, ExtractedPage,
    ResearchPacketManifest,
};
use std::path::Path;

const SENTINEL: &str = "RESEARCH_PACKET_SENTINEL_9F3A7";
const SERMON_ID: &str = "11111111-1111-4111-8111-111111111111";
const SERMON_ID_2: &str = "22222222-2222-4222-8222-222222222222";

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/research_packet_sentinel")
}

/// Build a REAL `pastor.db` with two authored sermons that share a primary
/// passage, references, series, and an illustration — enough for the engine to
/// emit `related-sermon` and `passage-history` insights.
fn authored_pastor_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(sermon_core::schema::PASTOR_SCHEMA).unwrap();

    let sermon_md = std::fs::read_to_string(fixture_dir().join("sermon.md")).unwrap();
    assert!(
        !sermon_md.contains(SENTINEL),
        "the authored sermon must not contain the sentinel"
    );

    for (id, path, title, big_idea) in [
        (
            SERMON_ID,
            "Sermons/romans-8-28.md",
            "Grace Greater Than Our Sin",
            "Grace grounds assurance",
        ),
        (
            SERMON_ID_2,
            "Sermons/romans-8-28-b.md",
            "Assurance in Grace",
            "Grace secures the believer",
        ),
    ] {
        conn.execute(
            "INSERT INTO sermon_index(id, file_path, file_hash, title, primary_passage, big_idea, series, structure_type)
             VALUES (?1, ?2, 'hash', ?3, 'Rom.8.28', ?4, 'Romans', 'expository')",
            rusqlite::params![id, path, title, big_idea],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sermon_body(sermon_id, body_content) VALUES (?1, ?2)",
            rusqlite::params![id, ":::movement one\n:::movement two"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scripture_sermon_links(sermon_id, book_num, chapter, verse) VALUES (?1, 45, 8, 28)",
            rusqlite::params![id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO illustration_usage(sermon_id, illustration_key, label, use_count) VALUES (?1, 'anchor', 'Anchor', 1)",
            rusqlite::params![id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sermons_fts(sermon_id, title, primary_passage, big_idea, body_content)
             VALUES (?1, ?2, 'Rom.8.28', ?3, ?4)",
            rusqlite::params![id, title, big_idea, sermon_md],
        )
        .unwrap();
    }
    conn
}

/// Every string an evidence item can carry, for sentinel scanning.
fn evidence_strings(e: &Evidence) -> Vec<&str> {
    let mut v = vec![e.kind.as_str(), e.label.as_str(), e.value.as_str()];
    v.extend(e.sermon_ids.iter().map(String::as_str));
    v.extend(e.references.iter().map(String::as_str));
    v
}

#[test]
fn sentinel_is_present_in_packet_but_isolated_from_the_archive() {
    let dir = fixture_dir();

    // ── 1. The sentinel IS in the packet metadata / extracted text. ──────────
    let manifest_json = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    let manifest: ResearchPacketManifest = serde_json::from_str(&manifest_json).unwrap();
    manifest.validate().expect("fixture manifest must be valid");
    manifest.validate_ownership(SERMON_ID).expect("fixture ownership");

    let att = &manifest.attachments[0];
    let pages_json = std::fs::read_to_string(
        dir.join("extracted")
            .join(format!("{}.pages.json", att.checksum)),
    )
    .unwrap();
    let pages: Vec<ExtractedPage> = serde_json::from_str(&pages_json).unwrap();
    let extracted_text = pages.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");
    assert!(
        extracted_text.contains(SENTINEL),
        "fixture must actually contain the sentinel in extracted text"
    );

    // The packet's provenance is research-packet.
    let packet_prov = att.provenance(SERMON_ID, Some(1));
    assert_eq!(packet_prov.class, ProvenanceClass::ResearchPacket);

    // ── 2. It is NOT lifetime corpus. ────────────────────────────────────────
    assert!(
        !is_lifetime_corpus(&packet_prov),
        "research-packet content must never be lifetime corpus"
    );
    assert!(!packet_prov.is_lifetime_corpus());

    // ── 3. It is NOT in pastor-authored content. ─────────────────────────────
    let packet_path =
        sermon_core::research_packet::packet_dir(&dir, SERMON_ID).join(&att.stored_filename);
    assert!(
        !is_indexable_as_pastor_content(&packet_path),
        "packet paths must not be indexable as pastor-authored content"
    );
    // Sanity: a real sermon path IS indexable.
    assert!(is_indexable_as_pastor_content(&dir.join("sermon.md")));

    // ── 4. It is NOT in the pastor-authored FTS index. ───────────────────────
    let conn = authored_pastor_db();

    let sentinel_hits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sermons_fts WHERE sermons_fts MATCH ?1",
            ["sentinel"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sentinel_hits, 0, "sentinel must not appear in pastor-authored FTS");

    // Control: the FTS index works and finds authored content.
    let control_hits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sermons_fts WHERE sermons_fts MATCH ?1",
            ["grace"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(control_hits, 2, "control token must be found (FTS is functional)");

    // ── 5. It is NOT eligible as Sermon Intelligence evidence. ───────────────
    // 5a. Run the REAL Track J engine over the real pastor.db.
    let related = related_sermons(&conn, SERMON_ID, 10).unwrap();
    let insights = sermon_insights(&conn, SERMON_ID, 10).unwrap();
    let history = passage_history(&conn, "Rom.8.28").unwrap();

    // The engine actually produced insights (the test is not vacuous).
    assert!(
        !related.insights.is_empty(),
        "engine must emit at least one related-sermon insight"
    );
    assert!(
        !history.insights.is_empty(),
        "engine must emit a passage-history insight"
    );

    // 5b. No evidence the engine emits contains the sentinel, and every evidence
    //     item is lifetime corpus (your-archive). This is the real admission
    //     path: the engine only ever surfaces the pastor's own archive.
    for result in [&related, &insights, &history] {
        assert_eq!(
            result.inputs.provenance_classes,
            vec![ProvenanceClass::YourArchive],
            "the engine must record that only the archive fed the run"
        );
        for insight in &result.insights {
            assert!(insight.is_valid(), "every insight must carry evidence");
            for e in &insight.evidence {
                for s in evidence_strings(e) {
                    assert!(
                        !s.contains(SENTINEL),
                        "sentinel must never appear in engine evidence: {s}"
                    );
                }
                assert_eq!(
                    e.provenance.class,
                    ProvenanceClass::YourArchive,
                    "engine evidence must be archive provenance"
                );
                assert!(is_lifetime_corpus(&e.provenance));
            }
        }
    }

    // 5c. The engine's REAL admission guard drops packet-derived evidence.
    assert!(
        !is_intelligence_eligible(&packet_prov),
        "research-packet provenance must not be SI-eligible"
    );
    let packet_evidence = Evidence {
        kind: "reference-overlap".to_string(),
        label: "External article".to_string(),
        value: SENTINEL.to_string(),
        weight: 0.5,
        sermon_ids: vec![SERMON_ID.to_string()],
        references: vec![],
        provenance: packet_prov.clone(),
    };
    let admitted: Vec<Evidence> = vec![packet_evidence]
        .into_iter()
        .filter_map(admit_evidence)
        .collect();
    assert!(admitted.is_empty(), "packet evidence must be dropped by the guard");

    // An insight whose only evidence is packet content ends up with no evidence
    // and therefore fails the evidence-is-the-explanation rule.
    let insight = Insight {
        id: "i-sentinel".to_string(),
        kind: "related-sermon".to_string(),
        title: "Related sermon".to_string(),
        summary: "1 measurable relationship factor(s)".to_string(),
        score: 0.5,
        evidence: admitted,
        related_sermon_ids: vec![],
    };
    assert!(
        !insight.is_valid(),
        "an insight with only packet evidence must be invalid"
    );

    // And a control: authored evidence IS admitted and validates.
    let authored_evidence = Evidence {
        kind: "reference-overlap".to_string(),
        label: "Romans 8:28".to_string(),
        value: "Rom.8.28".to_string(),
        weight: 0.9,
        sermon_ids: vec![SERMON_ID_2.to_string()],
        references: vec!["Rom.8.28".to_string()],
        provenance: Provenance::your_archive(SERMON_ID_2),
    };
    let admitted_authored: Vec<Evidence> = vec![authored_evidence]
        .into_iter()
        .filter_map(admit_evidence)
        .collect();
    assert_eq!(admitted_authored.len(), 1);
    let ok_insight = Insight {
        id: "i-ok".to_string(),
        kind: "related-sermon".to_string(),
        title: "Related sermon".to_string(),
        summary: "1 measurable relationship factor(s)".to_string(),
        score: 0.9,
        evidence: admitted_authored,
        related_sermon_ids: vec![SERMON_ID_2.to_string()],
    };
    assert!(ok_insight.is_valid());
}
