//! # Research Packet data model (V1)
//!
//! A **Research Packet** is the set of external documents a pastor attaches to a
//! single sermon (PDFs, articles, images). Packets live on the filesystem under:
//!
//! ```text
//! <vault>/.sermon-studio/attachments/<sermon-uuid>/
//!     manifest.json
//!     <sha256>.<ext>                 # content-addressed original
//!     extracted/<sha256>.pages.json  # per-page extracted text (no OCR)
//! ```
//!
//! ## Constitutional isolation
//!
//! Packet content is **never** lifetime corpus and is **never** eligible as
//! Sermon Intelligence evidence. This module provides the predicates that
//! enforce that ([`is_intelligence_eligible`], [`is_indexable_as_pastor_content`])
//! and the ownership/path guards that keep packets confined to their sermon.
//!
//! ## No OCR
//!
//! Extraction is text-layer only. Scanned/image-only PDFs yield
//! [`ExtractionStatus::NoText`]; OCR is explicitly deferred.

use crate::provenance::{Provenance, ProvenanceClass};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

/// Current packet manifest version.
pub const PACKET_VERSION: &str = "1";

/// Vault-relative directory that holds all research packets.
pub const ATTACHMENTS_DIR: &str = ".sermon-studio/attachments";

/// Errors raised by the research-packet layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    /// The manifest's `packet_version` is not supported.
    UnsupportedVersion(String),
    /// The manifest's `sermon_id` is not a valid UUID.
    InvalidSermonId(String),
    /// The manifest does not belong to the sermon it was loaded for.
    OwnershipMismatch { expected: String, found: String },
    /// An attachment id is duplicated within the manifest.
    DuplicateAttachmentId(String),
    /// An attachment's stored filename does not match its checksum.
    ChecksumFilenameMismatch { id: String, stored: String, checksum: String },
    /// An attachment's provenance class is not `research-packet`.
    WrongProvenanceClass { id: String, class: ProvenanceClass },
    /// A path escaped its confinement root (traversal attempt).
    PathEscape(String),
}

impl std::fmt::Display for PacketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PacketError::UnsupportedVersion(v) => write!(f, "unsupported packet version: {v}"),
            PacketError::InvalidSermonId(v) => write!(f, "invalid sermon id: {v}"),
            PacketError::OwnershipMismatch { expected, found } => {
                write!(f, "packet ownership mismatch: expected {expected}, found {found}")
            }
            PacketError::DuplicateAttachmentId(id) => write!(f, "duplicate attachment id: {id}"),
            PacketError::ChecksumFilenameMismatch { id, stored, checksum } => write!(
                f,
                "attachment {id}: stored filename {stored} does not match checksum {checksum}"
            ),
            PacketError::WrongProvenanceClass { id, class } => {
                write!(f, "attachment {id}: provenance class must be research-packet, got {class:?}")
            }
            PacketError::PathEscape(p) => write!(f, "path escapes confinement root: {p}"),
        }
    }
}

impl std::error::Error for PacketError {}

/// Result alias for the packet layer.
pub type PacketResult<T> = std::result::Result<T, PacketError>;

/// Extraction status of an attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractionStatus {
    /// Text was extracted successfully.
    Ok,
    /// The document has no text layer (e.g. a scan). OCR is deferred.
    NoText,
    /// Extraction was attempted and failed.
    Failed,
    /// Extraction has not been attempted yet.
    NotAttempted,
}

/// A single extracted page of text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedPage {
    /// 1-based page number.
    pub page: u32,
    /// Extracted text for the page.
    pub text: String,
}

/// Where an attachment was imported from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProvenance {
    /// The origin (e.g. a file path or a URL the user pasted).
    pub imported_from: String,
    /// RFC 3339 timestamp of import.
    pub imported_at: String,
}

/// A single attachment within a research packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchAttachment {
    /// Stable attachment id (unique within the packet).
    pub id: String,
    /// The filename as the user saw it.
    pub original_filename: String,
    /// The content-addressed stored filename: `<sha256>.<ext>`.
    pub stored_filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// RFC 3339 timestamp of addition.
    pub date_added: String,
    /// MIME type (best-effort).
    pub mime_type: String,
    /// Size in bytes.
    pub byte_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    /// SHA-256 hex of the original bytes.
    pub checksum: String,
    /// Extraction status.
    pub extraction: ExtractionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_notes: Option<String>,
    /// Import provenance.
    pub import_provenance: ImportProvenance,
    /// Always `research-packet`. Present so the invariant is explicit on the wire.
    pub provenance_class: ProvenanceClass,
}

impl ResearchAttachment {
    /// The provenance record for this attachment (optionally at a page).
    pub fn provenance(&self, sermon_id: &str, page: Option<u32>) -> Provenance {
        Provenance::research_packet(
            sermon_id,
            self.id.clone(),
            format!("Research packet: {}", self.original_filename),
            page,
        )
    }
}

/// The packet manifest, stored at `manifest.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchPacketManifest {
    /// Manifest version.
    pub packet_version: String,
    /// Owning sermon UUID.
    pub sermon_id: String,
    /// Attachments in the packet.
    #[serde(default)]
    pub attachments: Vec<ResearchAttachment>,
}

impl ResearchPacketManifest {
    /// A new, empty manifest for a sermon.
    pub fn new(sermon_id: impl Into<String>) -> Self {
        ResearchPacketManifest {
            packet_version: PACKET_VERSION.to_string(),
            sermon_id: sermon_id.into(),
            attachments: Vec::new(),
        }
    }

    /// Validate the manifest's internal consistency.
    pub fn validate(&self) -> PacketResult<()> {
        if self.packet_version != PACKET_VERSION {
            return Err(PacketError::UnsupportedVersion(self.packet_version.clone()));
        }
        if !is_valid_uuid(&self.sermon_id) {
            return Err(PacketError::InvalidSermonId(self.sermon_id.clone()));
        }
        let mut seen = std::collections::HashSet::new();
        for a in &self.attachments {
            if !seen.insert(a.id.clone()) {
                return Err(PacketError::DuplicateAttachmentId(a.id.clone()));
            }
            if a.provenance_class != ProvenanceClass::ResearchPacket {
                return Err(PacketError::WrongProvenanceClass {
                    id: a.id.clone(),
                    class: a.provenance_class,
                });
            }
            // stored_filename must be "<checksum>.<ext>".
            let expected_prefix = format!("{}.", a.checksum);
            if !a.stored_filename.starts_with(&expected_prefix) {
                return Err(PacketError::ChecksumFilenameMismatch {
                    id: a.id.clone(),
                    stored: a.stored_filename.clone(),
                    checksum: a.checksum.clone(),
                });
            }
        }
        Ok(())
    }

    /// Ensure this manifest belongs to `sermon_id`.
    pub fn validate_ownership(&self, sermon_id: &str) -> PacketResult<()> {
        if self.sermon_id != sermon_id {
            return Err(PacketError::OwnershipMismatch {
                expected: sermon_id.to_string(),
                found: self.sermon_id.clone(),
            });
        }
        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// SHA-256 hex of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Content address of `bytes` (SHA-256 hex). Alias of [`sha256_hex`] for intent.
pub fn content_address(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

/// Build a safe, content-addressed stored filename: `<sha256>.<ext>`.
///
/// The extension is taken from `original_filename`, lowercased and reduced to
/// `[a-z0-9]`; if none is present, `bin` is used. The original filename is never
/// used verbatim on disk, which removes path-traversal and collision risk.
pub fn safe_stored_filename(checksum: &str, original_filename: &str) -> String {
    let ext = Path::new(original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            e.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| "bin".to_string());
    format!("{checksum}.{ext}")
}

/// The packet directory for a sermon: `<root>/.sermon-studio/attachments/<uuid>`.
pub fn packet_dir(root: &Path, sermon_id: &str) -> PathBuf {
    root.join(ATTACHMENTS_DIR).join(sermon_id)
}

/// The extracted-text path for an attachment: `extracted/<sha256>.pages.json`.
pub fn extracted_path(packet_dir: &Path, checksum: &str) -> PathBuf {
    packet_dir.join("extracted").join(format!("{checksum}.pages.json"))
}

/// Lexically confine `candidate` under `root`, rejecting traversal.
///
/// This is a *lexical* check (it does not require the path to exist), which is
/// what we want for validating manifest-supplied names before touching disk.
pub fn confine_path(root: &Path, candidate: &Path) -> PacketResult<PathBuf> {
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };

    let mut out = PathBuf::new();
    for comp in joined.components() {
        match comp {
            Component::ParentDir => {
                if !out.pop() {
                    return Err(PacketError::PathEscape(joined.display().to_string()));
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }

    if !out.starts_with(root) {
        return Err(PacketError::PathEscape(joined.display().to_string()));
    }
    Ok(out)
}

/// Is `sermon_id` a syntactically valid UUID?
pub fn is_valid_uuid(sermon_id: &str) -> bool {
    uuid::Uuid::parse_str(sermon_id).is_ok()
}

/// **Constitutional predicate.** Research-packet content is never eligible as
/// Sermon Intelligence evidence. Only the pastor's archive and static biblical
/// study data may support an insight.
pub fn is_intelligence_eligible(provenance: &Provenance) -> bool {
    matches!(
        provenance.class,
        ProvenanceClass::YourArchive | ProvenanceClass::BiblicalStudy
    )
}

/// **Constitutional predicate.** A path that actually traverses the
/// `.sermon-studio/attachments/` directory hierarchy is packet content and must
/// never be indexed as pastor-authored content (and therefore never enters the
/// pastor-authored FTS index).
///
/// The test is **component-aware**: it returns `false` only when the path's
/// components contain the consecutive directory names `.sermon-studio` followed
/// by `attachments`. A filename that merely *contains* that text (for example
/// `.sermon-studio/attachments-notes.md`, or a file named
/// `notes-on-.sermon-studio`) is not the directory hierarchy and remains
/// indexable. Both `/` and `\` separators are honored so Windows-style paths
/// behave identically.
pub fn is_indexable_as_pastor_content(path: &Path) -> bool {
    !traverses_attachments_dir(path)
}

/// True only when `path` traverses `.sermon-studio` / `attachments` as real,
/// consecutive path components. Separators are normalized first so the check is
/// platform-independent; this is component splitting, never substring matching.
fn traverses_attachments_dir(path: &Path) -> bool {
    // The guarded hierarchy, as individual components (derived from the single
    // source of truth `ATTACHMENTS_DIR`).
    let guard: Vec<&str> = ATTACHMENTS_DIR.split('/').filter(|s| !s.is_empty()).collect();
    if guard.is_empty() {
        return false;
    }

    // Normalize separators so Windows-style paths split into components on any
    // platform. This is component splitting, not substring matching.
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut window: Vec<String> = Vec::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(name) => {
                window.push(name.to_string_lossy().into_owned());
                if window.len() > guard.len() {
                    window.remove(0);
                }
                if window.len() == guard.len()
                    && window.iter().map(String::as_str).eq(guard.iter().copied())
                {
                    return true;
                }
            }
            // RootDir / Prefix / ParentDir / CurDir break component adjacency.
            _ => window.clear(),
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_attachment(checksum: &str) -> ResearchAttachment {
        ResearchAttachment {
            id: "att-1".to_string(),
            original_filename: "notes.pdf".to_string(),
            stored_filename: format!("{checksum}.pdf"),
            title: None,
            author: None,
            source: None,
            date_added: "2025-01-01T00:00:00Z".to_string(),
            mime_type: "application/pdf".to_string(),
            byte_size: 1234,
            page_count: Some(2),
            checksum: checksum.to_string(),
            extraction: ExtractionStatus::Ok,
            user_notes: None,
            import_provenance: ImportProvenance {
                imported_from: "/home/pastor/notes.pdf".to_string(),
                imported_at: "2025-01-01T00:00:00Z".to_string(),
            },
            provenance_class: ProvenanceClass::ResearchPacket,
        }
    }

    #[test]
    fn content_address_is_stable_sha256() {
        // Known SHA-256 of "abc".
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(content_address(b"abc"), sha256_hex(b"abc"));
    }

    #[test]
    fn stored_filename_is_content_addressed_and_sanitized() {
        let c = sha256_hex(b"hello");
        assert_eq!(safe_stored_filename(&c, "My Notes.PDF"), format!("{c}.pdf"));
        assert_eq!(safe_stored_filename(&c, "no-extension"), format!("{c}.bin"));
        // Traversal in the original name cannot leak into the stored name.
        assert_eq!(safe_stored_filename(&c, "../../etc/passwd"), format!("{c}.bin"));
    }

    #[test]
    fn manifest_validation_accepts_a_well_formed_packet() {
        let c = sha256_hex(b"hello");
        let mut m = ResearchPacketManifest::new("11111111-1111-4111-8111-111111111111");
        m.attachments.push(sample_attachment(&c));
        assert!(m.validate().is_ok());
        assert!(m.validate_ownership("11111111-1111-4111-8111-111111111111").is_ok());
    }

    #[test]
    fn manifest_rejects_bad_version_uuid_and_checksum_mismatch() {
        let c = sha256_hex(b"hello");
        let mut m = ResearchPacketManifest::new("11111111-1111-4111-8111-111111111111");
        m.attachments.push(sample_attachment(&c));

        let mut bad_ver = m.clone();
        bad_ver.packet_version = "999".to_string();
        assert!(matches!(bad_ver.validate(), Err(PacketError::UnsupportedVersion(_))));

        let mut bad_uuid = m.clone();
        bad_uuid.sermon_id = "not-a-uuid".to_string();
        assert!(matches!(bad_uuid.validate(), Err(PacketError::InvalidSermonId(_))));

        let mut bad_sum = m.clone();
        bad_sum.attachments[0].stored_filename = "deadbeef.pdf".to_string();
        assert!(matches!(bad_sum.validate(), Err(PacketError::ChecksumFilenameMismatch { .. })));
    }

    #[test]
    fn ownership_mismatch_is_rejected() {
        let m = ResearchPacketManifest::new("11111111-1111-4111-8111-111111111111");
        assert!(matches!(
            m.validate_ownership("22222222-2222-4222-8222-222222222222"),
            Err(PacketError::OwnershipMismatch { .. })
        ));
    }

    #[test]
    fn path_confinement_blocks_traversal() {
        let root = Path::new("/vault/.sermon-studio/attachments/sermon-1");
        assert!(confine_path(root, Path::new("extracted/abc.pages.json")).is_ok());
        assert!(matches!(
            confine_path(root, Path::new("../../../../etc/passwd")),
            Err(PacketError::PathEscape(_))
        ));
        assert!(matches!(
            confine_path(root, Path::new("/etc/passwd")),
            Err(PacketError::PathEscape(_))
        ));
    }

    #[test]
    fn packet_content_is_not_intelligence_eligible() {
        let p = Provenance::research_packet("s-1", "att-1", "Research packet: x.pdf", Some(1));
        assert!(!is_intelligence_eligible(&p));
        assert!(is_intelligence_eligible(&Provenance::your_archive("s-1")));
        assert!(is_intelligence_eligible(&Provenance::biblical_study(
            "openbible-xrefs", "OpenBible", "CC-BY-4.0", "OpenBible.info"
        )));
    }

    #[test]
    fn packet_paths_are_not_indexable_as_pastor_content() {
        // (a) A real packet path traverses the attachments hierarchy.
        assert!(!is_indexable_as_pastor_content(Path::new(
            "/vault/.sermon-studio/attachments/s1/abc.pdf"
        )));
        // (b) A normal sermon path is indexable.
        assert!(is_indexable_as_pastor_content(Path::new("/vault/Sermons/romans-8.md")));
        // (c) A path whose *filename* merely contains the text
        //     ".sermon-studio/attachments" is NOT the directory hierarchy and
        //     remains indexable. (The old substring check wrongly rejected these.)
        assert!(is_indexable_as_pastor_content(Path::new(
            "/vault/Sermons/.sermon-studio/attachments-notes.md"
        )));
        assert!(is_indexable_as_pastor_content(Path::new(
            "/vault/Sermons/.sermon-studio/attachments.md"
        )));
        assert!(is_indexable_as_pastor_content(Path::new(
            "/vault/Sermons/notes-on-.sermon-studio/attachments.md"
        )));
        assert!(is_indexable_as_pastor_content(Path::new(
            "/vault/Sermons/.sermon-studio-attachments.md"
        )));
        // (d) Windows-style separators behave identically.
        assert!(!is_indexable_as_pastor_content(Path::new(
            "C:\\vault\\.sermon-studio\\attachments\\s1\\abc.pdf"
        )));
        assert!(is_indexable_as_pastor_content(Path::new(
            "C:\\vault\\Sermons\\romans-8.md"
        )));
        assert!(is_indexable_as_pastor_content(Path::new(
            "C:\\vault\\Sermons\\.sermon-studio\\attachments-notes.md"
        )));
        // (e) Nested packet paths (deeper than one level) are still packet content.
        assert!(!is_indexable_as_pastor_content(Path::new(
            "/vault/.sermon-studio/attachments/s1/extracted/abc.pages.json"
        )));
        assert!(!is_indexable_as_pastor_content(Path::new(
            "/vault/.sermon-studio/attachments/s1/nested/deeper/abc.pdf"
        )));
        // Relative packet paths are also rejected.
        assert!(!is_indexable_as_pastor_content(Path::new(
            ".sermon-studio/attachments/s1/abc.pdf"
        )));
    }
}
