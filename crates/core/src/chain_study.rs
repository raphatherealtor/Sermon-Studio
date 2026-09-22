//! # Chain Study (Wave 5 / Track N)
//!
//! Deterministic, offline scripture-chain engine over the locked canon schema
//! (`cross_references`, `topics`, `topic_verses`, `chain_edges`, `sources`)
//! with a bounded "From Your Archive" overlay from `pastor.db`
//! (`scripture_sermon_links`).
//!
//! ## What this is
//!
//! Given a seed reference the engine:
//!
//! 1. resolves the seed deterministically (`crate::reference`);
//! 2. loads a *bounded* cross-reference / rule-edge neighborhood;
//! 3. ranks members with explicit, versioned numeric rules (integer math);
//! 4. associates a named topic ONLY when a sourced topic record supports it;
//! 5. emits one bounded, readable chain whose every edge is traceable to a
//!    registered `source_id` (or an explicit versioned `rule_id`);
//! 6. overlays sermons from the pastor's own archive that touch chain verses —
//!    as a separate, clearly-scoped overlay that NEVER mutates the chain.
//!
//! ## What this is not
//!
//! * No AI, no embeddings, no LLMs, no network. Pure SQL + integer arithmetic.
//! * No Thompson Chain-Reference content. The engine admits only edges whose
//!   source is registered in canon.db `sources` **and** appears in the
//!   approved allow-list ([`APPROVED_SOURCE_IDS`]) — public-domain / CC-BY
//!   datasets only. Unregistered, unapproved, or research-packet sources are
//!   dropped: there is no unexplained edge.
//!
//! ## Determinism
//!
//! [`ENGINE_VERSION`] plus the same canon data, same pastor archive and same
//! parameters produce byte-identical results. All scoring is integer
//! (`*_MILLI`), every ordering is a total order with canonical
//! `(book, chapter, verse)` tie-breaking, and no wall-clock value enters the
//! output.

use crate::error::Result;
use crate::provenance::{Provenance, ProvenanceClass};
use crate::reference;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The named, deterministic engine version. Same canon data + same pastor
/// archive + same parameters + this version ⇒ identical results.
pub const ENGINE_VERSION: &str = "chain-study-1.0";

// ── Bounding constants (named, versioned, tested) ────────────────────────────

/// Maximum graph distance (BFS hops) from the seed verse.
pub const MAX_SEARCH_DEPTH: u32 = 2;
/// Maximum neighbors expanded per verse, per edge family.
pub const MAX_NEIGHBORS_PER_NODE: usize = 6;
/// Maximum references in a single chain.
pub const MAX_CHAIN_REFERENCES: usize = 12;
/// Maximum chains in a result.
pub const MAX_CHAINS: usize = 5;
/// Maximum "From Your Archive" connections in a result.
pub const MAX_ARCHIVE_CONNECTIONS: usize = 10;
/// Minimum sourced-topic support: a topic is associated with a chain only when
/// it contains the seed verse, or at least this many chain verses.
pub const MIN_TOPIC_SUPPORT: i64 = 2;
/// Maximum sourced topics associated with one chain.
pub const MAX_CHAIN_TOPICS: usize = 4;

// ── Scoring constants (integer milli-units; explicit, versioned rules) ──────

/// Direct cross-reference weight: `min(rank, 100)` OpenBible votes contribute
/// `rank * W_XREF_RANK_MILLI / 100` milli (`rank = 100` ⇒ 600 milli, the
/// dominant factor).
pub const W_XREF_RANK_MILLI: i64 = 600;
/// Explicit rule edge (`chain_edges`) weight: `min(weight, 1.0)` contributes
/// up to [`W_RULE_EDGE_MILLI`] milli.
pub const W_RULE_EDGE_MILLI: i64 = 300;
/// Sourced shared topical membership: [`W_SOURCED_TOPIC_MILLI`] milli per
/// supporting sourced topic, capped at three topics (300 milli).
pub const W_SOURCED_TOPIC_MILLI: i64 = 100;
/// Topic-contribution cap (topics counted per reference).
pub const MAX_TOPICS_PER_REFERENCE: i64 = 3;

/// Approved data sources for chain content. Public-domain / CC-BY datasets
/// from the canon registry only. Anything else — including any hypothetical
/// proprietary chain product — is dropped by [`source_is_approved`].
pub const APPROVED_SOURCE_IDS: &[&str] = &[
    "kjv-pd",
    "openbible-xrefs",
    "naves-topical",
    "torrey-topical",
];

/// Legacy `cross_references` rows predate the `source_id` column; their
/// dataset is documented and registered in the canon manifest. This is the
/// named, tested default used for those rows.
pub const DEFAULT_XREF_SOURCE_ID: &str = "openbible-xrefs";

// ── DTOs ─────────────────────────────────────────────────────────────────────

/// Top-level result. `canonAvailable == false` means the canon vault was
/// absent/empty or predates the locked schema: `chains` and
/// `archiveConnections` are empty and the UI shows a calm unavailable state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStudyResult {
    pub engine_version: String,
    pub seed_reference: String,
    pub canon_available: bool,
    pub chains: Vec<Chain>,
    pub archive_connections: Vec<ArchiveConnection>,
    /// Audit: the exact bounded parameters applied. These ARE the determinism
    /// contract, surfaced so a reader never has to guess the bounds.
    pub parameters: ChainStudyParameters,
}

/// The frozen bounding parameters of this engine version, for audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStudyParameters {
    pub max_search_depth: u32,
    pub max_neighbors_per_node: usize,
    pub max_chain_references: usize,
    pub max_chains: usize,
    pub max_archive_connections: usize,
}

impl ChainStudyParameters {
    /// The engine's frozen defaults — the constants above, verbatim.
    pub fn engine_defaults() -> Self {
        ChainStudyParameters {
            max_search_depth: MAX_SEARCH_DEPTH,
            max_neighbors_per_node: MAX_NEIGHBORS_PER_NODE,
            max_chain_references: MAX_CHAIN_REFERENCES,
            max_chains: MAX_CHAINS,
            max_archive_connections: MAX_ARCHIVE_CONNECTIONS,
        }
    }
}

/// One bounded, readable chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chain {
    pub id: String,
    /// Sourced topic name when (and only when) a source topic record supports
    /// the chain. Never an inferred label.
    pub name: Option<String>,
    pub seed_reference: String,
    pub references: Vec<ChainReference>,
    /// Sum of member weights (milli / 1000). Deterministic.
    pub score: f64,
    /// Why the chain is connected: one entry per linked reference plus one per
    /// sourced supporting topic.
    pub evidence: Vec<ChainEvidence>,
    /// Sourced topics supporting this chain (names come only from `topics`).
    pub source_topics: Vec<String>,
}

/// A verse in a chain, with its distance from the seed and provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainReference {
    /// Human-readable reference, e.g. `"Romans 8:28"`.
    pub reference: String,
    /// BFS graph distance from the seed (1 = direct neighbor).
    pub distance: u32,
    /// Chain weight contribution in milli/1000 (see scoring constants).
    pub weight: f64,
    pub provenance: Provenance,
}

/// A traceable connection explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainEvidence {
    /// `"cross-reference" | "rule-edge" | "sourced-topic"`.
    pub kind: String,
    pub label: String,
    /// The traceable token: the linking reference, the `rule_id`, or the
    /// `topic_id` — whichever makes the edge auditable.
    pub value: String,
    pub weight: f64,
    pub provenance: Provenance,
}

/// A sermon from the pastor's own archive that touches chain verses. Pure
/// overlay: building these never changes chain membership or scores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveConnection {
    /// Existing `sermon_index.id` — navigation uses the archive's own IDs.
    pub sermon_id: String,
    pub title: String,
    pub primary_passage: String,
    /// The chain references this sermon actually touches (canonical order).
    pub matching_references: Vec<String>,
    /// Always `your-archive`.
    pub provenance: Provenance,
}

// ── Admission guards ─────────────────────────────────────────────────────────

/// `true` when `source_id` is in the frozen approved allow-list.
///
/// This is the constitutional choke point for chain content: research-packet,
/// AI/engine, and any proprietary (e.g. Thompson) source can never produce a
/// chain edge because it is not on the list. Callers must ALSO verify the
/// source is registered in canon.db `sources` (see [`source_provenance`]) so
/// license/attribution strings come from the registry, never from ad hoc text.
#[inline]
pub fn source_is_approved(source_id: &str) -> bool {
    APPROVED_SOURCE_IDS.contains(&source_id)
}

/// Build a [`Provenance`] for a canon source from the `sources` registry.
///
/// Returns `None` (edge dropped) when the source is unregistered or not
/// approved. `source_version`/`license`/`attribution` always come from the
/// registry row so surfaced attribution matches the license exactly.
fn source_provenance(conn: &Connection, source_id: &str) -> Option<Provenance> {
    if !source_is_approved(source_id) {
        return None;
    }
    let row: Option<(String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT name, license_code, attribution, version FROM sources WHERE id = ?1",
            params![source_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    let (name, license_code, attribution, source_version) = row?;
    let mut p = Provenance::biblical_study(source_id, name, license_code, attribution);
    p.source_version = source_version;
    Some(p)
}

/// The admission guard for chain evidence — mirrors
/// `intelligence::admit_evidence`. Only static biblical-study provenance may
/// become chain evidence; research-packet, AI, and archive classes are dropped
/// here (archive content appears only in the explicitly-scoped overlay).
fn admit_edge_evidence(evidence: ChainEvidence) -> Option<ChainEvidence> {
    if evidence.provenance.class == ProvenanceClass::BiblicalStudy {
        Some(evidence)
    } else {
        None
    }
}

// ── Canon readiness ──────────────────────────────────────────────────────────

/// `true` when canon.db has the locked chain-study schema with usable data.
///
/// Requires the V1 extension (the `source_id` columns and topic tables) — a
/// pre-extension canon vault is reported as unavailable rather than erroring.
pub fn canon_ready(conn: &Connection) -> bool {
    let tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'
             AND name IN ('bible_verses','cross_references','sources','topics','topic_verses')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if tables < 5 {
        return false;
    }
    // The engine reads `cross_references.source_id` (V1 extension column).
    let has_source_col: i64 = {
        let mut stmt = match conn.prepare("PRAGMA table_info(cross_references)") {
            Ok(s) => s,
            Err(_) => return false,
        };
        let cols: Vec<String> = match stmt.query_map([], |r| r.get::<_, String>(1)) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => return false,
        };
        cols.iter().filter(|c| c.as_str() == "source_id").count() as i64
    };
    if has_source_col < 1 {
        return false;
    }
    // Usable data: at least one verse and one cross-reference.
    let verses: i64 = conn
        .query_row("SELECT COUNT(*) FROM bible_verses", [], |r| r.get(0))
        .unwrap_or(0);
    let xrefs: i64 = conn
        .query_row("SELECT COUNT(*) FROM cross_references", [], |r| r.get(0))
        .unwrap_or(0);
    verses > 0 && xrefs > 0
}

// ── Engine ───────────────────────────────────────────────────────────────────

/// Run Chain Study for `seed`. `pastor` is optional: `None` (or a fresh
/// archive) simply yields an empty "From Your Archive" overlay; it can never
/// alter the chain itself.
pub fn chain_study(
    canon: &Connection,
    pastor: Option<&Connection>,
    seed: &str,
) -> Result<ChainStudyResult> {
    let seed_trimmed = seed.trim().to_string();
    let unavailable = |seed_reference: String| ChainStudyResult {
        engine_version: ENGINE_VERSION.to_string(),
        seed_reference,
        canon_available: false,
        chains: Vec::new(),
        archive_connections: Vec::new(),
        parameters: ChainStudyParameters::engine_defaults(),
    };

    if !canon_ready(canon) {
        return Ok(unavailable(seed_trimmed));
    }

    // 1. Deterministic seed resolution.
    let passage = match reference::parse_passage(&seed_trimmed) {
        Ok(p) => p,
        // Unresolvable seed: an honest, calm empty result (not an error).
        Err(_) => return Ok(unavailable(seed_trimmed)),
    };
    let start = passage.start;
    let seed_id: Option<i64> = canon
        .query_row(
            "SELECT id FROM bible_verses WHERE book_num=?1 AND chapter=?2 AND verse=?3",
            params![start.book_num, start.chapter, start.verse],
            |r| r.get(0),
        )
        .ok();
    let Some(seed_id) = seed_id else {
        return Ok(unavailable(seed_trimmed));
    };

    // 2./3. Bounded BFS neighborhood with explicit scoring.
    let gathered = gather_neighbors(canon, seed_id)?;
    if gathered.is_empty() {
        return Ok(ChainStudyResult {
            engine_version: ENGINE_VERSION.to_string(),
            seed_reference: start.canonical(),
            canon_available: true,
            chains: Vec::new(),
            archive_connections: Vec::new(),
            parameters: ChainStudyParameters::engine_defaults(),
        });
    }

    // 4. Sourced topics supporting this chain.
    let source_topics = supporting_topics(canon, seed_id, &gathered)?;

    // Rank: (score_milli DESC, canonical ASC). Total order ⇒ deterministic.
    let mut ranked: Vec<&Neighbor> = gathered.values().collect();
    ranked.sort_by(|a, b| {
        b.best_score_milli
            .cmp(&a.best_score_milli)
            .then_with(|| a.canonical_key().cmp(&b.canonical_key()))
    });
    let chosen: Vec<&Neighbor> = ranked.into_iter().take(MAX_CHAIN_REFERENCES).collect();

    // 5. Bounded readable chain.
    let mut references: Vec<ChainReference> = Vec::new();
    let mut evidence: Vec<ChainEvidence> = Vec::new();
    let mut members: Vec<(i64, i64, i64, String)> = Vec::new();
    let mut score_milli_total: i64 = 0;
    for neighbor in &chosen {
        score_milli_total += neighbor.best_score_milli;
        references.push(ChainReference {
            reference: neighbor.human_reference.clone(),
            distance: neighbor.distance,
            weight: neighbor.best_score_milli as f64 / 1000.0,
            provenance: neighbor.best_provenance.clone(),
        });
        evidence.push(neighbor.best_evidence.clone());
        members.push((
            neighbor.book_num,
            neighbor.chapter,
            neighbor.verse,
            neighbor.human_reference.clone(),
        ));
    }
    for topic in &source_topics {
        evidence.push(ChainEvidence {
            kind: "sourced-topic".to_string(),
            label: topic.name.clone(),
            value: topic.topic_id.clone(),
            weight: W_SOURCED_TOPIC_MILLI as f64 / 1000.0,
            provenance: topic.provenance.clone(),
        });
    }

    let chain = Chain {
        id: format!("chain-{}", start.canonical()),
        name: source_topics.first().map(|t| t.name.clone()),
        seed_reference: start.canonical(),
        score: score_milli_total as f64 / 1000.0,
        references,
        evidence,
        source_topics: source_topics.iter().map(|t| t.name.clone()).collect(),
    };

    // 6. "From Your Archive" overlay (pure; never re-ranks the chain).
    let archive_connections = match pastor {
        Some(conn) => archive_overlay(conn, &members)?,
        None => Vec::new(),
    };

    Ok(ChainStudyResult {
        engine_version: ENGINE_VERSION.to_string(),
        seed_reference: start.canonical(),
        canon_available: true,
        chains: vec![chain],
        archive_connections,
        parameters: ChainStudyParameters::engine_defaults(),
    })
}

// ── Internal model ───────────────────────────────────────────────────────────

/// One sourced supporting topic (never an inferred label).
#[derive(Debug, Clone, PartialEq)]
struct SourcedTopic {
    topic_id: String,
    name: String,
    provenance: Provenance,
}

/// One discovered neighbor verse with its best (highest-scoring, then
/// canonically-first) admitted edge.
#[derive(Debug, Clone)]
struct Neighbor {
    book_num: i64,
    book_name: String,
    chapter: i64,
    verse: i64,
    distance: u32,
    best_score_milli: i64,
    human_reference: String,
    best_provenance: Provenance,
    best_evidence: ChainEvidence,
}

impl Neighbor {
    fn canonical_key(&self) -> (i64, i64, i64) {
        (self.book_num, self.chapter, self.verse)
    }
}

/// One admitted edge discovered during the bounded walk.
struct DiscoveredEdge {
    other_verse_id: i64,
    score_milli: i64,
    evidence: ChainEvidence,
}

/// Resolve a canon verse id → (book_num, book_name, chapter, verse).
fn verse_identity(conn: &Connection, verse_id: i64) -> Option<(i64, String, i64, i64)> {
    conn.query_row(
        "SELECT v.book_num, b.name, v.chapter, v.verse
         FROM bible_verses v JOIN bible_books b ON b.book_num = v.book_num
         WHERE v.id = ?1",
        params![verse_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .ok()
}

/// Human reference "Romans 8:28".
fn human_ref(book_name: &str, chapter: i64, verse: i64) -> String {
    format!("{} {}:{}", book_name, chapter, verse)
}

/// xref rank → milli: `min(rank,100) * W_XREF_RANK_MILLI / 100`.
fn xref_rank_milli(rank: i64) -> i64 {
    rank.clamp(0, 100) * W_XREF_RANK_MILLI / 100
}

/// chain_edges weight (REAL) → capped milli contribution.
fn edge_weight_milli(weight: f64) -> i64 {
    let capped = weight.clamp(0.0, 1.0);
    (capped * 1000.0) as i64 * W_RULE_EDGE_MILLI / 1000
}

/// Bounded BFS from the seed over cross-references and rule edges.
///
/// Bounds: [`MAX_SEARCH_DEPTH`] hops, [`MAX_NEIGHBORS_PER_NODE`] per verse per
/// family. Every discovered edge passes the admission guards (registered +
/// approved source) before it can influence the result — no unexplained edge.
/// Frontier order is deterministic (`BTreeMap` iteration + sorted edge lists),
/// so first-evidence-wins is stable.
fn gather_neighbors(canon: &Connection, seed_id: i64) -> Result<BTreeMap<i64, Neighbor>> {
    let mut frontier: Vec<i64> = vec![seed_id];
    let mut seen: BTreeSet<i64> = BTreeSet::from([seed_id]);
    let mut out: BTreeMap<i64, Neighbor> = BTreeMap::new();

    for depth in 1..=MAX_SEARCH_DEPTH {
        let mut next_frontier: Vec<i64> = Vec::new();
        for current in &frontier {
            for edge in edges_from(canon, *current, seed_id)? {
                let Some(evidence) = admit_edge_evidence(edge.evidence) else {
                    continue;
                };
                if let Some(existing) = out.get_mut(&edge.other_verse_id) {
                    // Discovered at a smaller distance; keep the best edge.
                    if edge.score_milli > existing.best_score_milli {
                        existing.best_score_milli = edge.score_milli;
                        existing.best_provenance = evidence.provenance.clone();
                        existing.best_evidence = evidence;
                    }
                    continue;
                }
                if seen.contains(&edge.other_verse_id) {
                    continue; // Seed, or already queued this round.
                }
                let Some((book_num, book_name, chapter, verse)) =
                    verse_identity(canon, edge.other_verse_id)
                else {
                    continue;
                };
                seen.insert(edge.other_verse_id);
                next_frontier.push(edge.other_verse_id);
                out.insert(
                    edge.other_verse_id,
                    Neighbor {
                        book_num,
                        book_name: book_name.clone(),
                        chapter,
                        verse,
                        distance: depth,
                        best_score_milli: edge.score_milli,
                        best_provenance: evidence.provenance.clone(),
                        best_evidence: evidence,
                        human_reference: human_ref(&book_name, chapter, verse),
                    },
                );
            }
        }
        if out.len() >= MAX_CHAIN_REFERENCES {
            break;
        }
        frontier = next_frontier;
    }
    Ok(out)
}

/// Admitted edges touching `verse_id` (both directions), deterministically
/// ordered, bounded per family. Cross-reference and rule-edge families are
/// each capped at [`MAX_NEIGHBORS_PER_NODE`].
fn edges_from(canon: &Connection, verse_id: i64, seed_id: i64) -> Result<Vec<DiscoveredEdge>> {
    let mut out: Vec<DiscoveredEdge> = Vec::new();

    // Family 1: cross_references (rank-ranked, both directions).
    let mut stmt = canon.prepare(
        "SELECT x.from_verse_id, x.to_verse_id, x.rank, x.source_id
         FROM cross_references x
         WHERE x.from_verse_id = ?1 OR x.to_verse_id = ?1
         ORDER BY x.rank DESC, x.from_verse_id ASC, x.to_verse_id ASC",
    )?;
    let mut xref_rows: Vec<(i64, i64, i64, Option<String>)> = Vec::new();
    let rows = stmt.query_map(params![verse_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        xref_rows.push(row?);
    }

    let mut admitted_xref = 0usize;
    for (from_id, to_id, rank, source_id) in xref_rows {
        if admitted_xref >= MAX_NEIGHBORS_PER_NODE {
            break;
        }
        let other = if from_id == verse_id { to_id } else { from_id };
        if other == seed_id {
            continue;
        }
        // Legacy rows predate source_id; their dataset is the named, registered
        // default. Unapproved/unregistered sources are dropped (no unexplained
        // edge).
        let effective_source =
            source_id.unwrap_or_else(|| DEFAULT_XREF_SOURCE_ID.to_string());
        let Some(provenance) = source_provenance(canon, &effective_source) else {
            continue;
        };
        let linking = verse_identity(canon, from_id)
            .map(|(_, name, c, v)| human_ref(&name, c, v))
            .unwrap_or_default();
        let Some(evidence) = admit_edge_evidence(ChainEvidence {
            kind: "cross-reference".to_string(),
            label: format!("Cross-reference (rank {})", rank.clamp(0, 100)),
            value: linking,
            weight: xref_rank_milli(rank) as f64 / 1000.0,
            provenance,
        }) else {
            continue;
        };
        out.push(DiscoveredEdge {
            other_verse_id: other,
            score_milli: xref_rank_milli(rank),
            evidence,
        });
        admitted_xref += 1;
    }

    // Family 2: chain_edges (explicit rule-derived edges).
    let mut stmt = canon.prepare(
        "SELECT c.from_verse, c.to_verse, c.kind, c.weight, c.source_id, c.rule_id
         FROM chain_edges c
         WHERE c.from_verse = ?1 OR c.to_verse = ?1
         ORDER BY c.weight DESC, c.from_verse ASC, c.to_verse ASC, c.kind ASC",
    )?;
    let mut rule_rows: Vec<(i64, i64, String, f64, String, Option<String>)> = Vec::new();
    let rows = stmt.query_map(params![verse_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, f64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    for row in rows {
        rule_rows.push(row?);
    }

    let mut admitted_rule = 0usize;
    for (from_verse, to_verse, kind, weight, source_id, rule_id) in rule_rows {
        if admitted_rule >= MAX_NEIGHBORS_PER_NODE {
            break;
        }
        let other = if from_verse == verse_id { to_verse } else { from_verse };
        if other == seed_id {
            continue;
        }
        let Some(provenance) = source_provenance(canon, &source_id) else {
            continue; // e.g. a proprietary/unapproved source → dropped.
        };
        // Traceability: explicit versioned rule_id when present, else the
        // registered source id — either way the edge is auditable.
        let value = rule_id.unwrap_or_else(|| format!("source:{}", source_id));
        let Some(evidence) = admit_edge_evidence(ChainEvidence {
            kind: "rule-edge".to_string(),
            label: format!("Rule edge ({})", kind),
            value,
            weight: edge_weight_milli(weight) as f64 / 1000.0,
            provenance,
        }) else {
            continue;
        };
        out.push(DiscoveredEdge {
            other_verse_id: other,
            score_milli: edge_weight_milli(weight),
            evidence,
        });
        admitted_rule += 1;
    }

    // Stable order inside a frontier round: score DESC, then other-verse id.
    out.sort_by(|a, b| {
        b.score_milli
            .cmp(&a.score_milli)
            .then_with(|| a.other_verse_id.cmp(&b.other_verse_id))
    });
    Ok(out)
}

/// Sourced supporting topics. A topic is associated ONLY when a `topics` row
/// exists AND its membership rows come from an approved source AND the topic
/// contains the seed verse or at least [`MIN_TOPIC_SUPPORT`] chain members.
/// Names come only from the `topics` table — never inferred.
fn supporting_topics(
    canon: &Connection,
    seed_id: i64,
    gathered: &BTreeMap<i64, Neighbor>,
) -> Result<Vec<SourcedTopic>> {
    let verse_ids: Vec<i64> = std::iter::once(seed_id)
        .chain(gathered.keys().copied())
        .collect();
    let mut topics: Vec<SourcedTopic> = Vec::new();
    // The verse-id set is bounded (≤ 1 + frontier-capped members), so the
    // prepared IN-list is bounded too. Bounds/limit are internal constants.
    let placeholders = vec!["?"; verse_ids.len()].join(",");
    let sql = format!(
        "SELECT t.id, t.name, tv.source_id, COUNT(DISTINCT tv.verse_id) AS members,
                MAX(CASE WHEN tv.verse_id = {seed_id} THEN 1 ELSE 0 END) AS has_seed
         FROM topic_verses tv
         JOIN topics t ON t.id = tv.topic_id
         WHERE tv.verse_id IN ({placeholders})
         GROUP BY t.id, t.name, tv.source_id
         HAVING has_seed = 1 OR members >= {min_support}
         ORDER BY members DESC, t.name ASC, t.id ASC
         LIMIT {limit}",
        seed_id = seed_id,
        placeholders = placeholders,
        min_support = MIN_TOPIC_SUPPORT,
        limit = MAX_CHAIN_TOPICS * 4, // admit-then-trim: guards may drop some
    );
    let mut stmt = canon.prepare(&sql)?;
    let params_list: Vec<&dyn rusqlite::ToSql> =
        verse_ids.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(params_list.as_slice(), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (topic_id, name, source_id, _members) = row?;
        let Some(provenance) = source_provenance(canon, &source_id) else {
            continue; // Unapproved/unsourced topic → never labeled.
        };
        topics.push(SourcedTopic {
            topic_id,
            name,
            provenance,
        });
        if topics.len() >= MAX_CHAIN_TOPICS {
            break;
        }
    }
    Ok(topics)
}

/// "From Your Archive" overlay: sermons from the pastor's own archive that
/// touch chain verses. Pure overlay — chain construction has already finished
/// and this never re-ranks or re-selects members. `members` is in canonical
/// order; output is ordered by the archive's own sermon IDs (stable) and
/// capped at [`MAX_ARCHIVE_CONNECTIONS`].
fn archive_overlay(
    pastor: &Connection,
    members: &[(i64, i64, i64, String)],
) -> Result<Vec<ArchiveConnection>> {
    // sermon_id → (title, primary_passage, matching chain references).
    let mut collected: BTreeMap<String, (String, String, BTreeSet<String>)> = BTreeMap::new();
    for (book, chapter, verse, human) in members {
        let mut stmt = pastor.prepare(
            "SELECT s.id, s.title, s.primary_passage
             FROM scripture_sermon_links l
             JOIN sermon_index s ON s.id = l.sermon_id
             WHERE l.book_num=?1 AND l.chapter=?2 AND l.verse=?3",
        )?;
        let rows = stmt.query_map(params![book, chapter, verse], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (sermon_id, title, primary_passage) = row?;
            let entry = collected
                .entry(sermon_id)
                .or_insert_with(|| (title, primary_passage, BTreeSet::new()));
            entry.2.insert(human.clone());
        }
    }
    let mut out: Vec<ArchiveConnection> = collected
        .into_iter()
        .map(|(sermon_id, (title, primary_passage, refs))| ArchiveConnection {
            provenance: Provenance::your_archive(sermon_id.clone()),
            matching_references: refs.into_iter().collect(),
            sermon_id,
            title,
            primary_passage,
        })
        .collect();
    out.sort_by(|a, b| a.sermon_id.cmp(&b.sermon_id));
    out.truncate(MAX_ARCHIVE_CONNECTIONS);
    Ok(out)
}

#[cfg(test)]
mod tests;
