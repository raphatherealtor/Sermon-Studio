//! Deterministic, offline sermon correlation with inspectable evidence.

use crate::error::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const ENGINE_VERSION: &str = "sermon-intelligence-1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceResult {
    pub engine_version: String,
    pub generated_at: String,
    pub subject_sermon_id: Option<String>,
    pub subject_reference: Option<String>,
    pub insights: Vec<Insight>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub kind: String,
    pub label: String,
    pub value: String,
    pub weight: f64,
    pub sermon_ids: Vec<String>,
    pub references: Vec<String>,
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
            0.30,
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
            0.25,
            &candidate.id,
        );
        add_terms(
            &mut evidence,
            "big-idea-overlap",
            "Shared Big Idea terms",
            &subject.big_idea,
            &candidate.big_idea,
            0.20,
            &candidate.id,
        );
        add_terms(
            &mut evidence,
            "title-overlap",
            "Shared title terms",
            &subject.title,
            &candidate.title,
            0.10,
            &candidate.id,
        );
        add_bool(
            &mut evidence,
            "series-overlap",
            "Same series",
            subject.series.is_some() && subject.series == candidate.series,
            0.10,
            &candidate.id,
            subject.series.as_deref().unwrap_or(""),
        );
        let shared_ill: BTreeSet<_> = subject_ill.intersection(&candidate_ill).cloned().collect();
        add_set(
            &mut evidence,
            "illustration-pattern",
            "Shared illustrations",
            &shared_ill,
            0.03,
            &candidate.id,
        );
        let structure_match = subject.structure == candidate.structure
            && movement_count(&subject.body) == movement_count(&candidate.body);
        add_bool(
            &mut evidence,
            "structure-overlap",
            "Same indexed structure and movement count",
            structure_match,
            0.02,
            &candidate.id,
            &format!(
                "{}; movements={}",
                subject.structure,
                movement_count(&subject.body)
            ),
        );
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
    let evidence = vec![Evidence {
        kind: "passage-history".into(),
        label: "Indexed sermons with exact primary passage".into(),
        value: details,
        weight: 1.0,
        sermon_ids: ids.clone(),
        references: vec![reference.into()],
    }];
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
    IntelligenceResult {
        engine_version: ENGINE_VERSION.into(),
        generated_at: conn.query_row("SELECT COALESCE(MAX(last_indexed_at),'1970-01-01T00:00:00Z') FROM sermon_index", [], |r| r.get(0)).unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        subject_sermon_id: id.map(str::to_string),
        subject_reference: reference.map(str::to_string),
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
}
