//! Deterministic, offline sermon correlation with inspectable evidence.
//!
//! # Track J engine (authoritative) + V1 scaffold reconciliation
//!
//! This module is Track J's authoritative Sermon Intelligence engine. The V1
//! scaffold is reconciled *around* it — the scoring design, insight-kind
//! vocabulary, DTO field names, and ranking behaviour are unchanged. The
//! reconciliation is strictly additive:
//!
//! 1. **Provenance on every evidence item.** [`Evidence`] gains a `provenance`
//!    field (see [`crate::provenance`]). Track J's existing fields
//!    (`kind`, `label`, `value`, `weight`, `sermon_ids`, `references`) are
//!    untouched. Every evidence item the engine emits is derived from the
//!    pastor's own archive, so its provenance is always
//!    [`ProvenanceClass::YourArchive`] — the only lifetime corpus.
//! 2. **A real admission guard.** [`admit_evidence`] is the single choke point
//!    through which the engine routes evidence. It drops anything that is not
//!    lifetime corpus or static biblical study, so research-packet content can
//!    never become evidence.
//! 3. **Auditable inputs/weights.** [`IntelligenceResult`] gains `inputs` and
//!    `weights`. The weights are the exact Track J scoring weights, now named
//!    constants shared by the scorer and the audit record so they cannot drift.
//!
//! The evidence-is-the-explanation rule is preserved: there is no free-text
//! reasoning field, `summary` is a deterministic short label, and every insight
//! carries at least one evidence item ([`Insight::is_valid`]).

use crate::error::Result;
use crate::provenance::{Provenance, ProvenanceClass};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const ENGINE_VERSION: &str = "sermon-intelligence-1.0";

// ── Track J scoring weights (frozen; shared by scorer and audit record) ──────
/// Weight for a shared primary passage.
pub const W_PRIMARY_PASSAGE_OVERLAP: f64 = 0.30;
/// Weight for shared scripture references.
pub const W_REFERENCE_OVERLAP: f64 = 0.25;
/// Weight for shared Big Idea terms.
pub const W_BIG_IDEA_OVERLAP: f64 = 0.20;
/// Weight for shared title terms.
pub const W_TITLE_OVERLAP: f64 = 0.10;
/// Weight for a shared series.
pub const W_SERIES_OVERLAP: f64 = 0.10;
/// Weight for shared illustrations.
pub const W_ILLUSTRATION_PATTERN: f64 = 0.03;
/// Weight for a shared indexed structure and movement count.
pub const W_STRUCTURE_OVERLAP: f64 = 0.02;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceResult {
    pub engine_version: String,
    pub generated_at: String,
    pub subject_sermon_id: Option<String>,
    pub subject_reference: Option<String>,
    /// Audit: what fed this run. Additive; does not affect scoring.
    pub inputs: IntelligenceInputs,
    /// Audit: the exact weights applied. Additive; does not affect scoring.
    pub weights: IntelligenceWeights,
    pub insights: Vec<Insight>,
}

/// Inputs a scoring run consumed, for reproducibility/audit.
///
/// Track J's engine reads only the pastor's own archive (`pastor.db`), so
/// `provenance_classes` is always `[your-archive]`. This is recorded explicitly
/// so a reviewer can see that no non-lifetime-corpus source ever feeds an
/// insight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceInputs {
    /// Provenance classes that fed this run.
    pub provenance_classes: Vec<ProvenanceClass>,
    /// Deterministic fingerprint of the archive index state
    /// (`MAX(last_indexed_at)`), or `None` when the archive is empty.
    pub archive_fingerprint: Option<String>,
}

/// The exact scoring weights applied by the engine, for reproducibility/audit.
///
/// These are the frozen Track J weights. They are defined once as constants and
/// used both by the scorer and by this record, so the audit view can never drift
/// from the behaviour.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceWeights {
    pub primary_passage_overlap: f64,
    pub reference_overlap: f64,
    pub big_idea_overlap: f64,
    pub title_overlap: f64,
    pub series_overlap: f64,
    pub illustration_pattern: f64,
    pub structure_overlap: f64,
}

impl IntelligenceWeights {
    /// The engine's frozen default weights (Track J values, verbatim).
    pub fn engine_defaults() -> Self {
        IntelligenceWeights {
            primary_passage_overlap: W_PRIMARY_PASSAGE_OVERLAP,
            reference_overlap: W_REFERENCE_OVERLAP,
            big_idea_overlap: W_BIG_IDEA_OVERLAP,
            title_overlap: W_TITLE_OVERLAP,
            series_overlap: W_SERIES_OVERLAP,
            illustration_pattern: W_ILLUSTRATION_PATTERN,
            structure_overlap: W_STRUCTURE_OVERLAP,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Insight {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub score: f64,
    pub evidence: Vec<Evidence>,
    pub related_sermon_ids: Vec<String>,
}

impl Insight {
    /// **Evidence is the explanation.** An insight is valid iff it carries at
    /// least one evidence item. There is intentionally no free-text reasoning
    /// field; `summary` is a deterministic short label.
    pub fn is_valid(&self) -> bool {
        !self.evidence.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub kind: String,
    pub label: String,
    pub value: String,
    pub weight: f64,
    pub sermon_ids: Vec<String>,
    pub references: Vec<String>,
    /// Where this evidence came from. Track J evidence is always derived from
    /// the pastor's own archive, so this is always `your-archive`.
    pub provenance: Provenance,
}

/// **The constitutional admission guard.**
///
/// Only lifetime-corpus (`your-archive`) and static biblical-study provenance
/// may become Sermon Intelligence evidence. Research-packet content — and any
/// other class — is dropped here. The engine routes every evidence item through
/// this single choke point, so packet content can never become evidence even if
/// a future change tried to inject it.
pub fn admit_evidence(evidence: Evidence) -> Option<Evidence> {
    if crate::research_packet::is_intelligence_eligible(&evidence.provenance) {
        Some(evidence)
    } else {
        None
    }
}

#[derive(Clone)]
struct Row {
    id: String,
    title: String,
    passage: String,
    big_idea: String,
    series: Option<String>,
    structure: String,
    body: String,
}

pub fn related_sermons(
    conn: &Connection,
    sermon_id: &str,
    limit: usize,
) -> Result<IntelligenceResult> {
    let rows = rows(conn)?;
    let Some(subject) = rows.iter().find(|r| r.id == sermon_id) else {
        return Ok(result(conn, Some(sermon_id), None, vec![]));
    };
    let subject_refs = verse_set(conn, sermon_id)?;
    let subject_ill = illustration_set(conn, sermon_id)?;
    let mut insights = Vec::new();
    for candidate in rows.iter().filter(|r| r.id != sermon_id) {
        let candidate_refs = verse_set(conn, &candidate.id)?;
        let candidate_ill = illustration_set(conn, &candidate.id)?;
        let mut evidence = Vec::new();
        add_bool(
            &mut evidence,
            "primary-passage-overlap",
            "Same primary passage",
            subject.passage == candidate.passage && !subject.passage.is_empty(),
            W_PRIMARY_PASSAGE_OVERLAP,
            &candidate.id,
            &subject.passage,
        );
        let shared_refs: BTreeSet<_> = subject_refs
            .intersection(&candidate_refs)
            .cloned()
            .collect();
        add_set(
            &mut evidence,
            "reference-overlap",
            "Shared scripture references",
            &shared_refs,
            W_REFERENCE_OVERLAP,
            &candidate.id,
        );
        add_terms(
            &mut evidence,
            "big-idea-overlap",
            "Shared Big Idea terms",
            &subject.big_idea,
            &candidate.big_idea,
            W_BIG_IDEA_OVERLAP,
            &candidate.id,
        );
        add_terms(
            &mut evidence,
            "title-overlap",
            "Shared title terms",
            &subject.title,
            &candidate.title,
            W_TITLE_OVERLAP,
            &candidate.id,
        );
        add_bool(
            &mut evidence,
            "series-overlap",
            "Same series",
            subject.series.is_some() && subject.series == candidate.series,
            W_SERIES_OVERLAP,
            &candidate.id,
            subject.series.as_deref().unwrap_or(""),
        );
        let shared_ill: BTreeSet<_> = subject_ill.intersection(&candidate_ill).cloned().collect();
        add_set(
            &mut evidence,
            "illustration-pattern",
            "Shared illustrations",
            &shared_ill,
            W_ILLUSTRATION_PATTERN,
            &candidate.id,
        );
        let structure_match = subject.structure == candidate.structure
            && movement_count(&subject.body) == movement_count(&candidate.body);
        add_bool(
            &mut evidence,
            "structure-overlap",
            "Same indexed structure and movement count",
            structure_match,
            W_STRUCTURE_OVERLAP,
            &candidate.id,
            &format!(
                "{}; movements={}",
                subject.structure,
                movement_count(&subject.body)
            ),
        );
        // Constitutional admission: drop any evidence that is not lifetime
        // corpus / biblical study. Track J evidence is always archive-derived,
        // so this is a no-op today — but it makes the guard real and enforced.
        let evidence: Vec<Evidence> = evidence.into_iter().filter_map(admit_evidence).collect();
        if evidence.is_empty() {
            continue;
        }
        let score = round6(evidence.iter().map(|e| e.weight).sum());
        insights.push(Insight {
            id: format!("related-sermon:{}:{}", sermon_id, candidate.id),
            kind: "related-sermon".into(),
            title: candidate.title.clone(),
            summary: format!("{} measurable relationship factor(s)", evidence.len()),
            score,
            evidence,
            related_sermon_ids: vec![candidate.id.clone()],
        });
    }
    insights.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.related_sermon_ids.cmp(&b.related_sermon_ids))
    });
    insights.truncate(limit);
    Ok(result(conn, Some(sermon_id), None, insights))
}

pub fn sermon_insights(
    conn: &Connection,
    sermon_id: &str,
    limit: usize,
) -> Result<IntelligenceResult> {
    related_sermons(conn, sermon_id, limit)
}

pub fn passage_history(conn: &Connection, reference: &str) -> Result<IntelligenceResult> {
    let mut stmt = conn.prepare("SELECT id,title,date_preached,series FROM sermon_index WHERE primary_passage=?1 ORDER BY COALESCE(date_preached,''),id")?;
    let matches: Vec<(String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![reference], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<std::result::Result<_, _>>()?;
    if matches.is_empty() {
        return Ok(result(conn, None, Some(reference), vec![]));
    }
    let ids: Vec<String> = matches.iter().map(|x| x.0.clone()).collect();
    let details = matches
        .iter()
        .map(|x| {
            format!(
                "{}|{}|{}|{}",
                x.0,
                x.1,
                x.2.as_deref().unwrap_or(""),
                x.3.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let evidence: Vec<Evidence> = vec![Evidence {
        kind: "passage-history".into(),
        label: "Indexed sermons with exact primary passage".into(),
        value: details,
        weight: 1.0,
        sermon_ids: ids.clone(),
        references: vec![reference.into()],
        provenance: Provenance::your_archive(ids.first().cloned().unwrap_or_default()),
    }]
    .into_iter()
    .filter_map(admit_evidence)
    .collect();
    Ok(result(
        conn,
        None,
        Some(reference),
        vec![Insight {
            id: format!("passage-history:{reference}"),
            kind: "passage-history".into(),
            title: format!("Passage history: {reference}"),
            summary: format!("{} indexed sermon(s)", ids.len()),
            score: 1.0,
            evidence,
            related_sermon_ids: ids,
        }],
    ))
}

fn result(conn: &Connection, id: Option<&str>, reference: Option<&str>, insights: Vec<Insight>) -> IntelligenceResult {
    let generated_at: String = conn
        .query_row(
            "SELECT COALESCE(MAX(last_indexed_at),'1970-01-01T00:00:00Z') FROM sermon_index",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
    IntelligenceResult {
        engine_version: ENGINE_VERSION.into(),
        generated_at: generated_at.clone(),
        subject_sermon_id: id.map(str::to_string),
        subject_reference: reference.map(str::to_string),
        inputs: IntelligenceInputs {
            provenance_classes: vec![ProvenanceClass::YourArchive],
            archive_fingerprint: Some(generated_at),
        },
        weights: IntelligenceWeights::engine_defaults(),
        insights,
    }
}
fn rows(conn: &Connection) -> Result<Vec<Row>> {
    let mut s=conn.prepare("SELECT s.id,s.title,s.primary_passage,s.big_idea,s.series,s.structure_type,COALESCE(b.body_content,'') FROM sermon_index s LEFT JOIN sermon_body b ON b.sermon_id=s.id ORDER BY s.id")?;
    let mapped = s.query_map([], |r| {
        Ok(Row {
            id: r.get(0)?,
            title: r.get(1)?,
            passage: r.get(2)?,
            big_idea: r.get(3)?,
            series: r.get(4)?,
            structure: r.get(5)?,
            body: r.get(6)?,
        })
    })?;
    let out = mapped.collect::<std::result::Result<_, _>>()?;
    Ok(out)
}
fn verse_set(conn: &Connection, id: &str) -> Result<BTreeSet<String>> {
    let mut s=conn.prepare("SELECT book_num,chapter,verse FROM scripture_sermon_links WHERE sermon_id=?1 ORDER BY book_num,chapter,verse")?;
    let mapped = s.query_map(params![id], |r| {
        Ok(format!(
            "{}.{}.{}",
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?
        ))
    })?;
    let out = mapped.collect::<std::result::Result<_, _>>()?;
    Ok(out)
}
fn illustration_set(conn: &Connection, id: &str) -> Result<BTreeSet<String>> {
    let mut s=conn.prepare("SELECT illustration_key FROM illustration_usage WHERE sermon_id=?1 ORDER BY illustration_key")?;
    let mapped = s.query_map(params![id], |r| r.get(0))?;
    let out = mapped.collect::<std::result::Result<_, _>>()?;
    Ok(out)
}
fn terms(s: &str) -> BTreeSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|x| x.len() > 2)
        .collect()
}
fn add_terms(
    out: &mut Vec<Evidence>,
    kind: &str,
    label: &str,
    a: &str,
    b: &str,
    max: f64,
    id: &str,
) {
    let x: Vec<_> = terms(a).intersection(&terms(b)).cloned().collect();
    if !x.is_empty() {
        let denom = terms(a).union(&terms(b)).count().max(1) as f64;
        out.push(Evidence {
            kind: kind.into(),
            label: label.into(),
            value: x.join(", "),
            weight: round6(max * x.len() as f64 / denom),
            sermon_ids: vec![id.into()],
            references: vec![],
            provenance: Provenance::your_archive(id),
        })
    }
}
fn add_bool(
    out: &mut Vec<Evidence>,
    kind: &str,
    label: &str,
    yes: bool,
    weight: f64,
    id: &str,
    value: &str,
) {
    if yes {
        out.push(Evidence {
            kind: kind.into(),
            label: label.into(),
            value: value.into(),
            weight,
            sermon_ids: vec![id.into()],
            references: if kind.contains("passage") {
                vec![value.into()]
            } else {
                vec![]
            },
            provenance: Provenance::your_archive(id),
        })
    }
}
fn add_set(
    out: &mut Vec<Evidence>,
    kind: &str,
    label: &str,
    set: &BTreeSet<String>,
    max: f64,
    id: &str,
) {
    if !set.is_empty() {
        out.push(Evidence {
            kind: kind.into(),
            label: label.into(),
            value: set.len().to_string(),
            weight: round6(max * (set.len() as f64 / 3.0).min(1.0)),
            sermon_ids: vec![id.into()],
            references: if kind == "reference-overlap" {
                set.iter().cloned().collect()
            } else {
                vec![]
            },
            provenance: Provenance::your_archive(id),
        })
    }
}
fn movement_count(body: &str) -> usize {
    body.matches(":::movement").count()
}
fn round6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PASTOR_SCHEMA;
    use std::collections::HashSet;
    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(PASTOR_SCHEMA).unwrap();
        for (id, title, p, b, s, st, body) in [
            (
                "a",
                "Hope in Trial",
                "Rom.8.28",
                "hope suffering glory",
                Some("Romans"),
                "verse",
                ":::movement x",
            ),
            (
                "b",
                "Glory Through Trial",
                "Rom.8.28",
                "hope glory",
                Some("Romans"),
                "verse",
                ":::movement y",
            ),
            (
                "c",
                "Mercy",
                "John.3.16",
                "mercy love",
                None,
                "narrative",
                "",
            ),
        ] {
            c.execute("INSERT INTO sermon_index(id,file_path,file_hash,title,primary_passage,big_idea,series,structure_type) VALUES(?1,?2,'h',?3,?4,?5,?6,?7)",params![id,format!("{id}.md"),title,p,b,s,st]).unwrap();
            c.execute("INSERT INTO sermon_body VALUES(?1,?2)", params![id, body])
                .unwrap();
        }
        for id in ["a", "b"] {
            c.execute(
                "INSERT INTO scripture_sermon_links VALUES(?1,45,8,28)",
                params![id],
            )
            .unwrap();
            c.execute("INSERT INTO illustration_usage(sermon_id,illustration_key,label,use_count) VALUES(?1,'anchor','Anchor',1)",params![id]).unwrap();
        }
        c
    }
    #[test]
    fn deterministic_ranking_evidence_and_ties() {
        let c = db();
        for id in ["d", "e"] {
            c.execute("INSERT INTO sermon_index(id,file_path,file_hash,title,primary_passage,big_idea,series,structure_type) VALUES(?1,?2,'h','Distinct','Acts.1.1','distinct words','Romans','verse')", params![id, format!("{id}.md")]).unwrap();
            c.execute("INSERT INTO sermon_body VALUES(?1,':::movement z')", params![id]).unwrap();
        }
        let a = related_sermons(&c, "a", 10).unwrap();
        let b = related_sermons(&c, "a", 10).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.insights[0].related_sermon_ids, ["b"]);
        assert_eq!(a.insights[1].score, a.insights[2].score);
        assert_eq!(a.insights[1].related_sermon_ids, ["d"]);
        assert_eq!(a.insights[2].related_sermon_ids, ["e"]);
        assert!(a.insights.iter().all(|i| !i.evidence.is_empty()));
        assert!(
            (a.insights[0].score - a.insights[0].evidence.iter().map(|e| e.weight).sum::<f64>())
                .abs()
                < 1e-6
        );
    }
    #[test]
    fn named_components_are_traceable() {
        let c = db();
        let r = related_sermons(&c, "a", 10).unwrap();
        let kinds: HashSet<_> = r.insights[0]
            .evidence
            .iter()
            .map(|e| e.kind.as_str())
            .collect();
        for k in [
            "primary-passage-overlap",
            "reference-overlap",
            "big-idea-overlap",
            "title-overlap",
            "series-overlap",
            "illustration-pattern",
            "structure-overlap",
        ] {
            assert!(kinds.contains(k), "{k}");
        }
    }
    #[test]
    fn unknown_and_missing_canon_are_safe() {
        let c = db();
        assert!(related_sermons(&c, "missing", 5)
            .unwrap()
            .insights
            .is_empty());
        assert!(passage_history(&c, "Unknown.1.1")
            .unwrap()
            .insights
            .is_empty());
    }
    #[test]
    fn passage_history_is_grounded() {
        let c = db();
        let r = passage_history(&c, "Rom.8.28").unwrap();
        assert_eq!(r.insights[0].related_sermon_ids, ["a", "b"]);
        assert_eq!(r.insights[0].evidence[0].references, ["Rom.8.28"]);
    }

    // ── V1 reconciliation tests (additive) ──────────────────────────────────

    #[test]
    fn every_evidence_item_is_archive_provenance_and_insights_are_valid() {
        let c = db();
        for r in [
            related_sermons(&c, "a", 10).unwrap(),
            sermon_insights(&c, "a", 10).unwrap(),
            passage_history(&c, "Rom.8.28").unwrap(),
        ] {
            for insight in &r.insights {
                assert!(insight.is_valid(), "every insight must carry evidence");
                for e in &insight.evidence {
                    assert_eq!(e.provenance.class, ProvenanceClass::YourArchive);
                    assert!(crate::provenance::is_lifetime_corpus(&e.provenance));
                }
            }
        }
    }

    #[test]
    fn admission_guard_drops_packet_evidence() {
        let packet = Evidence {
            kind: "reference-overlap".into(),
            label: "External article".into(),
            value: "RESEARCH_PACKET_SENTINEL_9F3A7".into(),
            weight: 0.5,
            sermon_ids: vec!["a".into()],
            references: vec![],
            provenance: Provenance::research_packet(
                "a",
                "att-1",
                "Research packet: x.pdf",
                Some(1),
            ),
        };
        assert!(admit_evidence(packet).is_none());
        // Control: archive evidence is admitted.
        let archive = Evidence {
            kind: "reference-overlap".into(),
            label: "Romans 8:28".into(),
            value: "Rom.8.28".into(),
            weight: 0.9,
            sermon_ids: vec!["b".into()],
            references: vec!["Rom.8.28".into()],
            provenance: Provenance::your_archive("b"),
        };
        assert!(admit_evidence(archive).is_some());
    }

    #[test]
    fn audit_inputs_and_weights_are_deterministic_and_frozen() {
        let c = db();
        let r = related_sermons(&c, "a", 10).unwrap();
        assert_eq!(r.inputs.provenance_classes, vec![ProvenanceClass::YourArchive]);
        assert_eq!(r.weights, IntelligenceWeights::engine_defaults());
        // The frozen Track J weights, verbatim.
        assert_eq!(r.weights.primary_passage_overlap, 0.30);
        assert_eq!(r.weights.reference_overlap, 0.25);
        assert_eq!(r.weights.big_idea_overlap, 0.20);
        assert_eq!(r.weights.title_overlap, 0.10);
        assert_eq!(r.weights.series_overlap, 0.10);
        assert_eq!(r.weights.illustration_pattern, 0.03);
        assert_eq!(r.weights.structure_overlap, 0.02);
    }
}
