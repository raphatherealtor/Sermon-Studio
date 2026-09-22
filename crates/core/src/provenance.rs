//! # Provenance contract (V1 — frozen)
//!
//! Every piece of content that Sermon Studio can surface carries a
//! [`Provenance`] describing *where it came from*. This module is the single
//! source of truth for that classification and is intentionally tiny, stable,
//! and dependency-light so that it can be frozen before any downstream
//! subsystem (canon pipeline, Chain Study, Research Packets, Sermon
//! Intelligence, Armarius) is built on top of it.
//!
//! ## The hard invariant
//!
//! [`is_lifetime_corpus`] returns `true` **only** for
//! [`ProvenanceClass::YourArchive`]. This is the constitutional rule of the
//! product: the pastor's own authored archive is the only thing that is
//! *lifetime corpus*. Everything else — purchased/PD study data, imported
//! research packets, machine-generated intelligence, and any future Armarius
//! content — is explicitly **not** lifetime corpus and must never be treated
//! as if it were the pastor's own writing.
//!
//! ## Serialization
//!
//! [`ProvenanceClass`] serializes to **kebab-case** strings
//! (`your-archive`, `biblical-study`, `research-packet`, `sermon-intelligence`,
//! `armarius`). This wire form is part of the frozen contract: the TypeScript
//! mirror in `src/lib/backend/contracts/provenance.ts` and any persisted JSON
//! must agree exactly. Do not rename variants without a version bump.

use serde::{Deserialize, Serialize};

/// The five provenance classes. Serialized as kebab-case.
///
/// Ordering is meaningful only for stable iteration in tests/UI; it carries no
/// semantic priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceClass {
    /// The pastor's own authored archive (Markdown sermons, notes, outlines).
    /// **The only lifetime corpus.**
    YourArchive,
    /// Static, read-only biblical study data shipped in `canon.db`
    /// (KJV text, Strong's, STEPBible tagging, xrefs, topical indexes).
    BiblicalStudy,
    /// A per-sermon imported research packet (PDFs, articles, images) stored
    /// under `.sermon-studio/attachments/<sermon-uuid>/`.
    ResearchPacket,
    /// Deterministic, evidence-backed output produced by the Sermon
    /// Intelligence engine (no LLM, no embeddings).
    SermonIntelligence,
    /// Reserved seam for a future external library/librarian integration.
    /// V1 ships the boundary only; no Armarius content is ingested.
    Armarius,
}

impl ProvenanceClass {
    /// `true` only for [`ProvenanceClass::YourArchive`].
    ///
    /// This is the method form of the [`is_lifetime_corpus`] free function and
    /// exists so call sites can read naturally (`p.class.is_lifetime_corpus()`).
    #[inline]
    pub fn is_lifetime_corpus(self) -> bool {
        matches!(self, ProvenanceClass::YourArchive)
    }

    /// The canonical kebab-case wire string. Kept in lockstep with the serde
    /// representation and asserted by tests.
    #[inline]
    pub fn as_wire(self) -> &'static str {
        match self {
            ProvenanceClass::YourArchive => "your-archive",
            ProvenanceClass::BiblicalStudy => "biblical-study",
            ProvenanceClass::ResearchPacket => "research-packet",
            ProvenanceClass::SermonIntelligence => "sermon-intelligence",
            ProvenanceClass::Armarius => "armarius",
        }
    }

    /// All classes, in a stable order. Useful for exhaustive tests/UI.
    pub const ALL: [ProvenanceClass; 5] = [
        ProvenanceClass::YourArchive,
        ProvenanceClass::BiblicalStudy,
        ProvenanceClass::ResearchPacket,
        ProvenanceClass::SermonIntelligence,
        ProvenanceClass::Armarius,
    ];
}

/// A provenance record attached to any surfaced content.
///
/// Fields are optional where they only apply to some classes:
/// * `source_id` / `source_version` / `license_code` / `attribution` — study
///   data and research packets.
/// * `sermon_id` — content that belongs to a specific sermon (archive content,
///   research packets, intelligence results).
/// * `attachment_id` / `page` — research-packet content.
/// * `engine_version` — Sermon Intelligence output.
///
/// `source_label` is always present: it is the human-readable name shown in the
/// UI ("Your archive", "STEPBible TAGNT", "Research packet: <file>").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The provenance class. Always present.
    pub class: ProvenanceClass,

    /// Stable machine id of the source (e.g. `stepbible-tagnt`,
    /// `openbible-xrefs`, `strongs-pd`). `None` for `YourArchive`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,

    /// Human-readable source label. Always present.
    pub source_label: String,

    /// Version of the source dataset/engine, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,

    /// SPDX-style license code (e.g. `CC-BY-4.0`, `PD`, `MIT`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_code: Option<String>,

    /// Attribution string required by the source license, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,

    /// Owning sermon UUID, when the content belongs to a sermon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sermon_id: Option<String>,

    /// Research-packet attachment id, when the content is packet content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<String>,

    /// 1-based page number within a research-packet attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,

    /// Engine version for `SermonIntelligence` provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
}

impl Provenance {
    /// Provenance for the pastor's own archive content.
    pub fn your_archive(sermon_id: impl Into<String>) -> Self {
        Provenance {
            class: ProvenanceClass::YourArchive,
            source_id: None,
            source_label: "Your archive".to_string(),
            source_version: None,
            license_code: None,
            attribution: None,
            sermon_id: Some(sermon_id.into()),
            attachment_id: None,
            page: None,
            engine_version: None,
        }
    }

    /// Provenance for static biblical study data shipped in `canon.db`.
    pub fn biblical_study(
        source_id: impl Into<String>,
        source_label: impl Into<String>,
        license_code: impl Into<String>,
        attribution: impl Into<String>,
    ) -> Self {
        Provenance {
            class: ProvenanceClass::BiblicalStudy,
            source_id: Some(source_id.into()),
            source_label: source_label.into(),
            source_version: None,
            license_code: Some(license_code.into()),
            attribution: Some(attribution.into()),
            sermon_id: None,
            attachment_id: None,
            page: None,
            engine_version: None,
        }
    }

    /// Provenance for a research-packet attachment page.
    pub fn research_packet(
        sermon_id: impl Into<String>,
        attachment_id: impl Into<String>,
        source_label: impl Into<String>,
        page: Option<u32>,
    ) -> Self {
        Provenance {
            class: ProvenanceClass::ResearchPacket,
            source_id: None,
            source_label: source_label.into(),
            source_version: None,
            license_code: None,
            attribution: None,
            sermon_id: Some(sermon_id.into()),
            attachment_id: Some(attachment_id.into()),
            page,
            engine_version: None,
        }
    }

    /// Provenance for Sermon Intelligence output.
    pub fn sermon_intelligence(engine_version: impl Into<String>, sermon_id: impl Into<String>) -> Self {
        Provenance {
            class: ProvenanceClass::SermonIntelligence,
            source_id: None,
            source_label: "Sermon Intelligence".to_string(),
            source_version: None,
            license_code: None,
            attribution: None,
            sermon_id: Some(sermon_id.into()),
            attachment_id: None,
            page: None,
            engine_version: Some(engine_version.into()),
        }
    }

    /// Convenience: is this provenance lifetime corpus?
    #[inline]
    pub fn is_lifetime_corpus(&self) -> bool {
        is_lifetime_corpus(self)
    }
}

/// **The hard invariant.** Returns `true` if and only if `provenance.class` is
/// [`ProvenanceClass::YourArchive`].
///
/// Every subsystem that decides "is this the pastor's own writing?" MUST route
/// through this function (or [`ProvenanceClass::is_lifetime_corpus`]) rather
/// than comparing classes ad hoc.
#[inline]
pub fn is_lifetime_corpus(provenance: &Provenance) -> bool {
    provenance.class.is_lifetime_corpus()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_your_archive_is_lifetime_corpus() {
        // Exhaustive over all classes: exactly one is lifetime corpus.
        let lifetime: Vec<ProvenanceClass> = ProvenanceClass::ALL
            .iter()
            .copied()
            .filter(|c| c.is_lifetime_corpus())
            .collect();
        assert_eq!(lifetime, vec![ProvenanceClass::YourArchive]);

        // And the free function agrees for representative DTOs.
        assert!(is_lifetime_corpus(&Provenance::your_archive("s-1")));
        assert!(!is_lifetime_corpus(&Provenance::biblical_study(
            "strongs-pd",
            "Strong's",
            "PD",
            "Public domain"
        )));
        assert!(!is_lifetime_corpus(&Provenance::research_packet(
            "s-1",
            "att-1",
            "Research packet: foo.pdf",
            Some(3)
        )));
        assert!(!is_lifetime_corpus(&Provenance::sermon_intelligence(
            "1.0.0", "s-1"
        )));
        assert!(!is_lifetime_corpus(&Provenance {
            class: ProvenanceClass::Armarius,
            source_id: Some("armarius".into()),
            source_label: "Armarius".into(),
            source_version: None,
            license_code: None,
            attribution: None,
            sermon_id: None,
            attachment_id: None,
            page: None,
            engine_version: None,
        }));
    }

    #[test]
    fn class_serializes_kebab_case_and_is_stable() {
        let cases = [
            (ProvenanceClass::YourArchive, "\"your-archive\""),
            (ProvenanceClass::BiblicalStudy, "\"biblical-study\""),
            (ProvenanceClass::ResearchPacket, "\"research-packet\""),
            (ProvenanceClass::SermonIntelligence, "\"sermon-intelligence\""),
            (ProvenanceClass::Armarius, "\"armarius\""),
        ];
        for (class, expected) in cases {
            let json = serde_json::to_string(&class).unwrap();
            assert_eq!(json, expected, "wire form drifted for {class:?}");
            // as_wire() must agree with serde.
            assert_eq!(format!("\"{}\"", class.as_wire()), expected);
            // Round-trips.
            let back: ProvenanceClass = serde_json::from_str(&json).unwrap();
            assert_eq!(back, class);
        }
    }

    #[test]
    fn provenance_round_trips_and_omits_none() {
        let p = Provenance::research_packet("sermon-uuid", "att-1", "Research packet: foo.pdf", Some(7));
        let json = serde_json::to_string(&p).unwrap();
        // None fields are omitted from the wire form.
        assert!(!json.contains("source_id"));
        assert!(!json.contains("license_code"));
        assert!(json.contains("\"class\":\"research-packet\""));
        assert!(json.contains("\"page\":7"));
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn your_archive_has_no_source_id() {
        let p = Provenance::your_archive("s-1");
        assert!(p.source_id.is_none());
        assert_eq!(p.class, ProvenanceClass::YourArchive);
        assert!(p.is_lifetime_corpus());
    }
}
