//! Deterministic structural sermon linter.
//!
//! This is *not* an AI writing system. It inspects a canonical [`Sermon`] AST
//! (Track B) and, for the illustration-fatigue rule, the pastor.db archive
//! (Track C), then reports structural conditions without ever rewriting sermon
//! content.
//!
//! Rules (V1, locked):
//!
//! | rule id                     | severity | trigger                                        |
//! |-----------------------------|----------|------------------------------------------------|
//! | `missing-big-idea`          | error    | metadata lacks a meaningful Big Idea            |
//! | `orphaned-movement`         | warning  | movement lacks a usable scripture warrant       |
//! | `missing-application`       | warning  | no meaningful `application` block               |
//! | `illustration-fatigue-90d`  | warning  | illustration identity reused within 90 days     |
//!
//! Scripture warrant resolution delegates to Track A's [`crate::reference`]
//! (`Resolution::Definite` / `Ambiguous` / `Invalid`); this module never
//! defines its own reference semantics. An ambiguous warrant stays ambiguous
//! (no finding); an invalid or non-reference warrant is reported, never
//! silently repaired or clamped.

use crate::directive::SourceSpan;
use crate::error::Result;
use crate::reference::{self, Resolution};
use crate::sermon::{slugify, Movement, Sermon};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

// ---- Stable rule identifiers ----------------------------------------------

pub const RULE_MISSING_BIG_IDEA: &str = "missing-big-idea";
pub const RULE_ORPHANED_MOVEMENT: &str = "orphaned-movement";
pub const RULE_MISSING_APPLICATION: &str = "missing-application";
pub const RULE_ILLUSTRATION_FATIGUE_90D: &str = "illustration-fatigue-90d";

/// The number of days that defines the illustration-fatigue window.
pub const ILLUSTRATION_FATIGUE_WINDOW_DAYS: i64 = 90;

// ---- Finding model ---------------------------------------------------------

/// Explicit, deterministic finding severity. No subjective scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Identity of the AST block a finding refers to, where relevant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockIdentity {
    /// Block kind, e.g. `"movement"`.
    pub kind: String,
    /// Movement order, when the block carries one.
    pub order: Option<u32>,
    /// Block title, when present.
    pub title: Option<String>,
}

/// A historical sermon that reuses the same illustration identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedSermon {
    pub sermon_id: String,
    pub date_preached: Option<String>,
    pub illustration_key: String,
    pub label: String,
}

/// One structural finding. Carries enough structured information for the
/// frontend/Tauri layer to render and locate it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Unique, deterministic finding id.
    pub id: String,
    /// Stable rule identifier (one of the four `RULE_*` constants).
    pub rule_id: String,
    pub severity: Severity,
    pub sermon_id: String,
    /// Human-readable explanation.
    pub message: String,
    /// Byte-offset source location, where available.
    pub span: Option<SourceSpan>,
    /// Movement/block identity, where relevant.
    pub block: Option<BlockIdentity>,
    /// For `illustration-fatigue-90d`: prior sermons responsible for the reuse.
    pub related: Vec<RelatedSermon>,
}

// ---- Line/column mapping ---------------------------------------------------

/// 1-based line and column (column counted in Unicode scalar values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCol {
    pub line: usize,
    pub column: usize,
}

/// Deterministically map a byte offset into `source` to a 1-based line/column.
///
/// Offsets past the end of `source` clamp to the final position. Column is
/// counted in Unicode scalar values (not bytes) for human readability.
pub fn line_col_at(source: &str, offset: usize) -> LineCol {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = source[line_start..offset].chars().count() + 1;
    LineCol { line, column }
}

// ---- AST structural checks -------------------------------------------------

/// Run the AST-only structural checks (rules 1–3) over a canonical sermon.
///
/// Pure and infallible: never mutates the sermon and returns findings in a
/// deterministic, document order.
pub fn lint_sermon(sermon: &Sermon, sermon_id: &str) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Rule 1: missing Big Idea.
    if !has_meaningful_big_idea(sermon) {
        findings.push(Finding {
            id: format!("{RULE_MISSING_BIG_IDEA}:{sermon_id}"),
            rule_id: RULE_MISSING_BIG_IDEA.to_string(),
            severity: Severity::Error,
            sermon_id: sermon_id.to_string(),
            message: "sermon is missing a Big Idea: the `big_idea` frontmatter field is absent or empty".to_string(),
            span: None,
            block: None,
            related: Vec::new(),
        });
    }

    // Rule 2: orphaned movements.
    for (idx, movement) in sermon.movements().enumerate() {
        if let Some(message) = orphaned_movement_message(movement) {
            let disambiguator = match movement.span {
                Some(span) => format!("@{}", span.start),
                None => format!(":m{}", idx + 1),
            };
            findings.push(Finding {
                id: format!("{RULE_ORPHANED_MOVEMENT}:{sermon_id}{disambiguator}"),
                rule_id: RULE_ORPHANED_MOVEMENT.to_string(),
                severity: Severity::Warning,
                sermon_id: sermon_id.to_string(),
                message,
                span: movement.span,
                block: Some(BlockIdentity {
                    kind: "movement".to_string(),
                    order: movement.order,
                    title: movement.title.clone(),
                }),
                related: Vec::new(),
            });
        }
    }

    // Rule 3: missing application.
    if !has_meaningful_application(sermon) {
        findings.push(Finding {
            id: format!("{RULE_MISSING_APPLICATION}:{sermon_id}"),
            rule_id: RULE_MISSING_APPLICATION.to_string(),
            severity: Severity::Warning,
            sermon_id: sermon_id.to_string(),
            message: "sermon has no meaningful application block".to_string(),
            span: None,
            block: None,
            related: Vec::new(),
        });
    }

    findings
}

fn has_meaningful_big_idea(sermon: &Sermon) -> bool {
    sermon.big_idea().map(|b| !b.trim().is_empty()).unwrap_or(false)
}

fn has_meaningful_application(sermon: &Sermon) -> bool {
    sermon.applications().any(|a| !a.body.trim().is_empty())
}

/// Return the orphan finding message when a movement lacks a usable warrant.
///
/// Scripture resolution is delegated to Track A. A missing, non-reference, or
/// `Invalid` warrant is reported; a `Definite` or `Ambiguous` warrant is left
/// as-is (ambiguous stays ambiguous).
fn orphaned_movement_message(movement: &Movement) -> Option<String> {
    match movement.warrant.as_deref().map(str::trim) {
        None | Some("") => Some("movement has no scripture warrant".to_string()),
        Some(warrant) => match reference::resolve(warrant) {
            Some(r) if r.resolution == Resolution::Invalid => Some(format!(
                "movement warrant is not a valid scripture reference and is left unrepaired: \"{warrant}\""
            )),
            Some(_) => None,
            None => Some(format!(
                "movement warrant is not a scripture reference: \"{warrant}\""
            )),
        },
    }
}

// ---- Illustration fatigue (90 days) ---------------------------------------

/// Detect reuse of the current sermon's illustration identities within the
/// prior 90 days, using pastor.db archive records.
///
/// Identity comparison is by stable `illustration_key` (the deterministic slug
/// the indexer stores) — never by semantic similarity. The current sermon's own
/// history is excluded, only sermons dated within `[current - 90 days, current]`
/// count, and missing/unparseable dates are handled deterministically (a missing
/// current date means no window can be established, so no findings; a missing
/// historical date excludes that record).
pub fn lint_illustration_fatigue(
    conn: &Connection,
    sermon_id: &str,
    date_preached: Option<&str>,
    illustration_labels: &[String],
) -> Result<Vec<Finding>> {
    let keys: Vec<String> = illustration_labels
        .iter()
        .map(|label| slugify(label))
        .filter(|k| !k.is_empty())
        .collect();
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    // The current sermon's date establishes the window. Without a parseable
    // date there is no deterministic "prior 90 days".
    let Some(current_date) = parse_date(date_preached) else {
        return Ok(Vec::new());
    };
    let Some(cutoff) = current_date.checked_sub_signed(chrono::Duration::days(
        ILLUSTRATION_FATIGUE_WINDOW_DAYS,
    )) else {
        return Ok(Vec::new());
    };

    let mut findings = Vec::new();
    for key in &keys {
        let mut stmt = conn.prepare(
            "SELECT i.label, s.id, s.date_preached
             FROM illustration_usage i
             JOIN sermon_index s ON s.id = i.sermon_id
             WHERE i.sermon_id != ?1 AND i.illustration_key = ?2",
        )?;
        let rows = stmt.query_map(params![sermon_id, key], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;

        let mut related: Vec<RelatedSermon> = Vec::new();
        for row in rows {
            let (label, prior_id, prior_date) = row?;
            if prior_id == sermon_id {
                continue;
            }
            let Some(prior) = parse_date(prior_date.as_deref()) else {
                continue; // missing date: cannot place in window → excluded
            };
            if prior < cutoff || prior > current_date {
                continue; // outside the 90-day window
            }
            related.push(RelatedSermon {
                sermon_id: prior_id,
                date_preached: prior_date,
                illustration_key: key.clone(),
                label,
            });
        }

        if !related.is_empty() {
            let label = related
                .first()
                .map(|r| r.label.as_str())
                .unwrap_or(key.as_str());
            findings.push(Finding {
                id: format!("{RULE_ILLUSTRATION_FATIGUE_90D}:{sermon_id}:{key}"),
                rule_id: RULE_ILLUSTRATION_FATIGUE_90D.to_string(),
                severity: Severity::Warning,
                sermon_id: sermon_id.to_string(),
                message: format!(
                    "illustration \"{label}\" was reused within the prior 90 days in {} prior sermon(s)",
                    related.len()
                ),
                span: None,
                block: None,
                related,
            });
        }
    }

    Ok(findings)
}

/// Strictly parse an ISO-8601 `YYYY-MM-DD` preached date.
fn parse_date(s: Option<&str>) -> Option<chrono::NaiveDate> {
    let s = s?.trim();
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer;

    fn parse(raw: &str) -> Sermon {
        Sermon::parse(raw).unwrap()
    }

    fn by_rule<'a>(findings: &'a [Finding], rule: &str) -> Vec<&'a Finding> {
        findings.iter().filter(|f| f.rule_id == rule).collect()
    }

    // ---- Rule 1: missing big idea ------------------------------------------

    #[test]
    fn big_idea_present_yields_no_finding() {
        let s = parse("---\nid: s1\ntitle: \"T\"\nbig_idea: \"God is faithful.\"\n---\n\nBody.\n");
        let findings = lint_sermon(&s, "s1");
        assert!(by_rule(&findings, RULE_MISSING_BIG_IDEA).is_empty());
    }

    #[test]
    fn missing_big_idea_yields_finding() {
        let s = parse("---\nid: s1\ntitle: \"T\"\n---\n\nBody.\n");
        let findings = lint_sermon(&s, "s1");
        let hits = by_rule(&findings, RULE_MISSING_BIG_IDEA);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].severity, Severity::Error);
        assert!(hits[0].message.contains("absent"));
        assert_eq!(hits[0].sermon_id, "s1");
    }

    #[test]
    fn blank_big_idea_counts_as_missing() {
        let s = parse("---\nid: s1\nbig_idea: \"   \"\n---\n\nBody.\n");
        assert_eq!(by_rule(&lint_sermon(&s, "s1"), RULE_MISSING_BIG_IDEA).len(), 1);
    }

    // ---- Rule 2: orphaned movement -----------------------------------------

    #[test]
    fn movement_with_warrant_yields_no_orphan_finding() {
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\n\
             :::movement{order=1 warrant=\"John 15:1-4\"}\nBody.\n:::\n\n\
             :::application\nApply it.\n:::\n",
        );
        assert!(by_rule(&lint_sermon(&s, "s1"), RULE_ORPHANED_MOVEMENT).is_empty());
    }

    #[test]
    fn movement_without_warrant_yields_orphan_finding() {
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\n\
             :::movement{order=1 title=\"First move\"}\nBody.\n:::\n",
        );
        let findings = lint_sermon(&s, "s1");
        let hits = by_rule(&findings, RULE_ORPHANED_MOVEMENT);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].severity, Severity::Warning);
        assert_eq!(hits[0].sermon_id, "s1");
        // Block identity and source span attached.
        let block = hits[0].block.as_ref().unwrap();
        assert_eq!(block.kind, "movement");
        assert_eq!(block.order, Some(1));
        assert!(hits[0].span.is_some());
    }

    #[test]
    fn invalid_warrant_is_reported_not_repaired() {
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\n\
             :::movement{order=1 warrant=\"John 99:1\"}\nBody.\n:::\n",
        );
        let findings = lint_sermon(&s, "s1");
        let hits = by_rule(&findings, RULE_ORPHANED_MOVEMENT);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("unrepaired"));
    }

    #[test]
    fn ambiguous_warrant_stays_ambiguous_without_finding() {
        // "vv.4-7" resolves to Ambiguous; per Track A semantics it stays
        // ambiguous rather than being flagged as orphaned.
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\n\
             :::movement{order=1 warrant=\"vv.4-7\"}\nBody.\n:::\n",
        );
        assert!(by_rule(&lint_sermon(&s, "s1"), RULE_ORPHANED_MOVEMENT).is_empty());
    }

    // ---- Rule 3: missing application ---------------------------------------

    #[test]
    fn application_present_yields_no_finding() {
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\n\
             :::application\nGo and do likewise.\n:::\n",
        );
        assert!(by_rule(&lint_sermon(&s, "s1"), RULE_MISSING_APPLICATION).is_empty());
    }

    #[test]
    fn missing_application_yields_finding() {
        let s = parse("---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\nBody only.\n");
        let findings = lint_sermon(&s, "s1");
        let hits = by_rule(&findings, RULE_MISSING_APPLICATION);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].severity, Severity::Warning);
    }

    #[test]
    fn empty_application_body_counts_as_missing() {
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\n:::application\n\n:::\n",
        );
        assert_eq!(by_rule(&lint_sermon(&s, "s1"), RULE_MISSING_APPLICATION).len(), 1);
    }

    // ---- Rule 4: illustration fatigue (90 days) ----------------------------

    fn seed_db() -> (tempfile::TempDir, Connection) {
        let tmp = tempfile::tempdir().unwrap();
        let conn = indexer::open_pastor_db(&tmp.path().join("pastor.db")).unwrap();
        (tmp, conn)
    }

    fn insert_sermon(conn: &Connection, id: &str, date: Option<&str>) {
        conn.execute(
            "INSERT INTO sermon_index
             (id, file_path, file_hash, title, date_preached, series, liturgical_season,
              primary_passage, exegetical_proposition, big_idea, structure_type)
             VALUES (?1, ?2, '', '', ?3, NULL, NULL, '', NULL, '', '')",
            params![id, format!("{id}.md"), date],
        )
        .unwrap();
    }

    fn insert_usage(conn: &Connection, sermon_id: &str, key: &str, label: &str) {
        conn.execute(
            "INSERT INTO illustration_usage
             (sermon_id, illustration_key, label, first_seen, last_used, use_count)
             VALUES (?1, ?2, ?3, '', '', 1)",
            params![sermon_id, key, label],
        )
        .unwrap();
    }

    #[test]
    fn fatigue_inside_window_reported_with_prior_sermons() {
        let (_tmp, conn) = seed_db();
        insert_sermon(&conn, "prior", Some("2024-06-01"));
        insert_usage(&conn, "prior", "the-farmer", "The farmer");

        let findings = lint_illustration_fatigue(
            &conn,
            "current",
            Some("2024-07-14"),
            &["The farmer".to_string()],
        )
        .unwrap();

        let hits = by_rule(&findings, RULE_ILLUSTRATION_FATIGUE_90D);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].severity, Severity::Warning);
        assert_eq!(hits[0].related.len(), 1);
        assert_eq!(hits[0].related[0].sermon_id, "prior");
        assert_eq!(hits[0].related[0].date_preached.as_deref(), Some("2024-06-01"));
    }

    #[test]
    fn fatigue_outside_window_not_reported() {
        let (_tmp, conn) = seed_db();
        insert_sermon(&conn, "prior", Some("2024-01-01"));
        insert_usage(&conn, "prior", "the-farmer", "The farmer");

        let findings = lint_illustration_fatigue(
            &conn,
            "current",
            Some("2024-07-14"),
            &["The farmer".to_string()],
        )
        .unwrap();
        assert!(by_rule(&findings, RULE_ILLUSTRATION_FATIGUE_90D).is_empty());
    }

    #[test]
    fn fatigue_different_identity_not_reported() {
        let (_tmp, conn) = seed_db();
        insert_sermon(&conn, "prior", Some("2024-06-01"));
        insert_usage(&conn, "prior", "the-shepherd", "The shepherd");

        let findings = lint_illustration_fatigue(
            &conn,
            "current",
            Some("2024-07-14"),
            &["The farmer".to_string()],
        )
        .unwrap();
        assert!(by_rule(&findings, RULE_ILLUSTRATION_FATIGUE_90D).is_empty());
    }

    #[test]
    fn fatigue_excludes_current_sermon_history() {
        let (_tmp, conn) = seed_db();
        // The current sermon's own usage row must not count against itself.
        insert_sermon(&conn, "current", Some("2024-07-14"));
        insert_usage(&conn, "current", "the-farmer", "The farmer");

        let findings = lint_illustration_fatigue(
            &conn,
            "current",
            Some("2024-07-14"),
            &["The farmer".to_string()],
        )
        .unwrap();
        assert!(by_rule(&findings, RULE_ILLUSTRATION_FATIGUE_90D).is_empty());
    }

    #[test]
    fn fatigue_missing_historical_date_excluded_deterministically() {
        let (_tmp, conn) = seed_db();
        insert_sermon(&conn, "prior-nodate", None);
        insert_usage(&conn, "prior-nodate", "the-farmer", "The farmer");

        let findings = lint_illustration_fatigue(
            &conn,
            "current",
            Some("2024-07-14"),
            &["The farmer".to_string()],
        )
        .unwrap();
        assert!(by_rule(&findings, RULE_ILLUSTRATION_FATIGUE_90D).is_empty());
    }

    #[test]
    fn fatigue_missing_current_date_yields_no_findings() {
        let (_tmp, conn) = seed_db();
        insert_sermon(&conn, "prior", Some("2024-06-01"));
        insert_usage(&conn, "prior", "the-farmer", "The farmer");

        let findings = lint_illustration_fatigue(&conn, "current", None, &["The farmer".to_string()])
            .unwrap();
        assert!(findings.is_empty());
    }

    // ---- Cross-cutting guarantees ------------------------------------------

    #[test]
    fn findings_use_only_stable_rule_ids() {
        let s = parse("---\nid: s1\n---\n\n:::movement{order=1}\nBody.\n:::\n");
        let findings = lint_sermon(&s, "s1");
        assert!(!findings.is_empty());
        for f in &findings {
            assert!(
                [
                    RULE_MISSING_BIG_IDEA,
                    RULE_ORPHANED_MOVEMENT,
                    RULE_MISSING_APPLICATION,
                    RULE_ILLUSTRATION_FATIGUE_90D,
                ]
                .contains(&f.rule_id.as_str()),
                "unexpected rule id: {}",
                f.rule_id
            );
            assert!(!f.id.is_empty());
        }
    }

    #[test]
    fn source_spans_attached_where_available() {
        let s = parse(
            "---\nid: s1\nbig_idea: \"God is faithful.\"\n---\n\nIntro.\n\n\
             :::movement{order=1}\nBody.\n:::\n",
        );
        let findings = lint_sermon(&s, "s1");
        let hits = by_rule(&findings, RULE_ORPHANED_MOVEMENT);
        assert_eq!(hits.len(), 1);
        let span = hits[0].span.unwrap();
        assert!(span.start < span.end);
    }

    #[test]
    fn line_col_mapping_is_deterministic() {
        let src = "line one\nline two\nline three";
        assert_eq!(line_col_at(src, 0), LineCol { line: 1, column: 1 });
        assert_eq!(line_col_at(src, 5), LineCol { line: 1, column: 6 });
        // 'line two' starts at byte 9 (after "line one\n").
        assert_eq!(line_col_at(src, 9), LineCol { line: 2, column: 1 });
        assert_eq!(line_col_at(src, 14), LineCol { line: 2, column: 6 });
        // Offset past end clamps to the final position.
        assert_eq!(line_col_at(src, 1000), LineCol { line: 3, column: 11 });
    }

    #[test]
    fn linting_never_mutates_sermon_source() {
        let raw = "---\nid: s1\ntitle: \"T\"\nbig_idea: \"God is faithful.\"\n---\n\n\
                   :::movement{order=1 warrant=\"John 15:1-4\"}\nBody.\n:::\n\n\
                   :::application\nApply it.\n:::\n";
        let s = parse(raw);
        let before = s.to_markdown();
        lint_sermon(&s, "s1");
        assert_eq!(s.to_markdown(), before, "linting must not mutate the sermon");
    }
}
