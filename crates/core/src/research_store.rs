//! # Research Packet store (V1)
//!
//! The filesystem realization of the research-packet scaffold in
//! [`crate::research_packet`]: importing a local PDF into one sermon's packet,
//! content-addressed storage, page-aware text extraction, atomic manifest
//! updates, and provenance-safe removal.
//!
//! ## Guarantees
//!
//! - **One sermon per packet.** The manifest is keyed by sermon UUID and every
//!   read validates ownership; sermon ids must be valid UUIDs.
//! - **Content-addressed blobs.** Stored as `<sha256>.pdf` inside the packet
//!   directory; the original user file is copied (never moved) and never used
//!   verbatim on disk.
//! - **Atomic manifest.** Every mutation rewrites `manifest.json` via
//!   [`crate::atomic_save::save_atomic`] (temp file + rename), so a crash
//!   mid-write cannot produce a torn manifest.
//! - **Extraction is text-layer only, no OCR.** Scanned/image-only PDFs yield
//!   [`ExtractionStatus::NoText`]; we never invent text. Extraction runs
//!   in-process (pure-Rust `pdf-extract`; there is no embedded-JS execution and
//!   no shell involvement), bounded by [`MAX_ATTACHMENT_BYTES`]. A pathological
//!   PDF can still take bounded-but-not-timeout-bounded CPU time in-process;
//!   that limitation is accepted for V1 (documented in the Wave 5 report).
//! - **No network.** Import reads a local file the user selected; extraction is
//!   local computation.
//! - **Isolation.** Nothing here touches the pastor-authored index, the FTS
//!   tables, or Sermon Intelligence; packet paths are additionally excluded
//!   from indexing by the indexer guard.

use crate::atomic_save;
use crate::research_packet::{
    confine_path, content_address, extracted_path, is_valid_uuid, packet_dir, safe_stored_filename,
    ExtractionStatus, ExtractedPage, ImportProvenance, PacketError, ResearchAttachment,
    ResearchPacketManifest, ATTACHMENTS_DIR,
};
use crate::provenance::ProvenanceClass;
use std::path::{Path, PathBuf};

/// Reasonable size bound for a single research attachment (50 MiB).
pub const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;

/// PDF magic bytes — type validation is by content, not extension.
const PDF_MAGIC: &[u8] = b"%PDF-";

/// Errors raised by the packet store layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// Wrapped scaffold-level error (validation, confinement, ownership).
    Packet(PacketError),
    /// Filesystem error with context.
    Io(String),
    /// Attachment exceeds the size bound.
    TooLarge { size: u64, max: u64 },
    /// Content does not look like a PDF.
    NotPdf,
    /// The requested attachment does not exist in this packet.
    AttachmentNotFound(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Packet(e) => write!(f, "{e}"),
            StoreError::Io(ctx) => write!(f, "filesystem error: {ctx}"),
            StoreError::TooLarge { size, max } => {
                write!(f, "attachment is {size} bytes; the limit is {max}")
            }
            StoreError::NotPdf => write!(f, "file content is not a PDF"),
            StoreError::AttachmentNotFound(id) => write!(f, "attachment not found: {id}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<PacketError> for StoreError {
    fn from(e: PacketError) -> Self {
        StoreError::Packet(e)
    }
}

pub type StoreResult<T> = std::result::Result<T, StoreError>;

fn io_err(ctx: impl std::fmt::Display) -> StoreError {
    StoreError::Io(ctx.to_string())
}

/// Metadata the user supplies (or edits later) for an attachment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttachmentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub user_notes: Option<String>,
}

/// Import request: the local file the user selected plus optional metadata.
#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Absolute path of the user-selected local PDF (copied, never moved).
    pub source_path: PathBuf,
    pub metadata: AttachmentMetadata,
}

// ── Manifest I/O ─────────────────────────────────────────────────────────────

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("manifest.json")
}

/// Load the packet manifest for a sermon, creating an empty one if absent.
/// Validates version, shape, and ownership.
pub fn load_manifest(vault: &Path, sermon_id: &str) -> StoreResult<ResearchPacketManifest> {
    require_uuid(sermon_id)?;
    let dir = packet_dir(vault, sermon_id);
    let path = manifest_path(&dir);
    if !path.exists() {
        return Ok(ResearchPacketManifest::new(sermon_id));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| io_err(format!("read {}: {e}", path.display())))?;
    let manifest: ResearchPacketManifest = serde_json::from_str(&raw)
        .map_err(|e| io_err(format!("parse {}: {e}", path.display())))?;
    manifest.validate()?;
    manifest.validate_ownership(sermon_id)?;
    Ok(manifest)
}

/// Write the manifest atomically (temp file + rename) inside the packet dir.
pub fn save_manifest(vault: &Path, sermon_id: &str, manifest: &ResearchPacketManifest) -> StoreResult<()> {
    require_uuid(sermon_id)?;
    manifest.validate().map_err(StoreError::Packet)?;
    manifest.validate_ownership(sermon_id)?;
    let dir = packet_dir(vault, sermon_id);
    std::fs::create_dir_all(&dir).map_err(|e| io_err(format!("create {}: {e}", dir.display())))?;
    let path = manifest_path(&dir);
    let confined = confine_path(&dir, &path)?;
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| io_err(format!("serialize manifest: {e}")))?;
    atomic_save::save_atomic(&confined, json.as_bytes())
        .map_err(|e| io_err(format!("atomic write {}: {e}", confined.display())))
}

// ── Import / extraction ──────────────────────────────────────────────────────

/// Import a user-selected local PDF into a sermon's research packet.
///
/// Idempotent per content: if this sermon already has an attachment with the
/// same SHA-256, the existing attachment is returned and nothing is duplicated
/// (no second blob, no second manifest entry).
///
/// Extraction is attempted when `extract` is true (the normal path); failures
/// degrade the extraction status instead of failing the import.
pub fn import_attachment(
    vault: &Path,
    sermon_id: &str,
    request: &ImportRequest,
) -> StoreResult<ResearchAttachment> {
    import_attachment_bounded(vault, sermon_id, request, MAX_ATTACHMENT_BYTES, true)
}

/// Test/config seam: explicit size bound and an extraction toggle.
pub fn import_attachment_bounded(
    vault: &Path,
    sermon_id: &str,
    request: &ImportRequest,
    max_bytes: u64,
    extract: bool,
) -> StoreResult<ResearchAttachment> {
    require_uuid(sermon_id)?;

    // 1–3. Read the user-selected file, enforce the size bound, sniff content.
    let bytes = std::fs::read(&request.source_path)
        .map_err(|e| io_err(format!("read {}: {e}", request.source_path.display())))?;
    let size = bytes.len() as u64;
    if size > max_bytes {
        return Err(StoreError::TooLarge { size, max: max_bytes });
    }
    if !bytes.starts_with(PDF_MAGIC) {
        return Err(StoreError::NotPdf);
    }

    // 4–5. Content-addressed stored name; copy (never move) into the packet dir.
    let checksum = content_address(&bytes);
    let stored_filename = safe_stored_filename(&checksum, &request.source_path.to_string_lossy().as_ref());
    let dir = packet_dir(vault, sermon_id);
    std::fs::create_dir_all(&dir).map_err(|e| io_err(format!("create {}: {e}", dir.display())))?;

    // 6. Manifest up front: validates ownership and lets us dedup.
    let mut manifest = load_manifest(vault, sermon_id)?;
    if let Some(existing) = manifest.attachments.iter().find(|a| a.checksum == checksum) {
        return Ok(existing.clone());
    }

    // 7. Write the blob (atomic), confined to the packet directory.
    let blob_path = confine_path(&dir, Path::new(&stored_filename))?;
    atomic_save::save_atomic(&blob_path, &bytes)
        .map_err(|e| io_err(format!("store blob {}: {e}", blob_path.display())))?;

    // 8–10. Page-aware text extraction (text layer only, no OCR).
    let mut extraction = ExtractionStatus::NotAttempted;
    let mut page_count: Option<u32> = None;
    if extract {
        match extract_pages(&bytes) {
            Ok(pages) => {
                let meaningful: Vec<&ExtractedPage> = pages
                    .iter()
                    .filter(|p| !p.text.trim().is_empty())
                    .collect();
                if meaningful.is_empty() {
                    // Scanned/image-only: say so; never invent text.
                    extraction = ExtractionStatus::NoText;
                } else {
                    let json = serde_json::to_string_pretty(&pages)
                        .map_err(|e| io_err(format!("serialize pages: {e}")))?;
                    let pages_path = confine_path(&dir, &extracted_path(Path::new(""), &checksum))?;
                    if let Some(parent) = pages_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| io_err(format!("create {}: {e}", parent.display())))?;
                    }
                    atomic_save::save_atomic(&pages_path, json.as_bytes())
                        .map_err(|e| io_err(format!("store pages {}: {e}", pages_path.display())))?;
                    extraction = ExtractionStatus::Ok;
                    page_count = Some(pages.len() as u32);
                }
            }
            Err(_) => {
                extraction = ExtractionStatus::Failed;
            }
        }
    }

    let attachment = ResearchAttachment {
        id: uuid::Uuid::new_v4().to_string(),
        original_filename: request
            .source_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "attachment.pdf".to_string()),
        stored_filename,
        title: request.metadata.title.clone(),
        author: request.metadata.author.clone(),
        source: request.metadata.source.clone(),
        date_added: chrono::Utc::now().to_rfc3339(),
        mime_type: "application/pdf".to_string(),
        byte_size: size,
        page_count,
        checksum,
        extraction,
        user_notes: request.metadata.user_notes.clone(),
        import_provenance: ImportProvenance {
            imported_from: request.source_path.to_string_lossy().into_owned(),
            imported_at: chrono::Utc::now().to_rfc3339(),
        },
        provenance_class: ProvenanceClass::ResearchPacket,
    };
    manifest.attachments.push(attachment.clone());
    save_manifest(vault, sermon_id, &manifest)?;
    Ok(attachment)
}

/// Text-layer extraction, page-aware. Never OCR; empty pages stay empty.
fn extract_pages(bytes: &[u8]) -> Result<Vec<ExtractedPage>, ()> {
    let texts = pdf_extract::extract_text_from_mem_by_pages(bytes).map_err(|_| ())?;
    Ok(texts
        .into_iter()
        .enumerate()
        .map(|(i, text)| ExtractedPage {
            page: (i + 1) as u32,
            text,
        })
        .collect())
}

// ── Queries ──────────────────────────────────────────────────────────────────

/// All attachments in a sermon's packet.
pub fn list_attachments(vault: &Path, sermon_id: &str) -> StoreResult<Vec<ResearchAttachment>> {
    Ok(load_manifest(vault, sermon_id)?.attachments)
}

/// One attachment, ownership-validated.
pub fn get_attachment(
    vault: &Path,
    sermon_id: &str,
    attachment_id: &str,
) -> StoreResult<ResearchAttachment> {
    let manifest = load_manifest(vault, sermon_id)?;
    manifest
        .attachments
        .into_iter()
        .find(|a| a.id == attachment_id)
        .ok_or_else(|| StoreError::AttachmentNotFound(attachment_id.to_string()))
}

/// Page-aware extracted text for an attachment (empty when not extracted).
pub fn get_extracted_pages(
    vault: &Path,
    sermon_id: &str,
    attachment_id: &str,
) -> StoreResult<Vec<ExtractedPage>> {
    let att = get_attachment(vault, sermon_id, attachment_id)?;
    let dir = packet_dir(vault, sermon_id);
    let path = confine_path(
        &dir,
        Path::new("extracted").join(format!("{}.pages.json", att.checksum)).as_path(),
    )?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| io_err(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&raw).map_err(|e| io_err(format!("parse {}: {e}", path.display())))
}

/// Update user-editable metadata (title/author/source/notes). Atomic manifest.
pub fn update_metadata(
    vault: &Path,
    sermon_id: &str,
    attachment_id: &str,
    patch: &AttachmentMetadata,
) -> StoreResult<ResearchAttachment> {
    let mut manifest = load_manifest(vault, sermon_id)?;
    let att = manifest
        .attachments
        .iter_mut()
        .find(|a| a.id == attachment_id)
        .ok_or_else(|| StoreError::AttachmentNotFound(attachment_id.to_string()))?;
    att.title = patch.title.clone();
    att.author = patch.author.clone();
    att.source = patch.source.clone();
    att.user_notes = patch.user_notes.clone();
    let updated = att.clone();
    save_manifest(vault, sermon_id, &manifest)?;
    Ok(updated)
}

/// Absolute, confinement-validated path of the stored original PDF.
/// Used by the open command; the path is never constructed from user input.
pub fn stored_file_path(
    vault: &Path,
    sermon_id: &str,
    attachment_id: &str,
) -> StoreResult<PathBuf> {
    let att = get_attachment(vault, sermon_id, attachment_id)?;
    let dir = packet_dir(vault, sermon_id);
    let path = confine_path(&dir, Path::new(&att.stored_filename))?;
    if !path.is_file() {
        return Err(StoreError::Io(format!("stored file missing: {}", path.display())));
    }
    Ok(path)
}

/// Remove an attachment: manifest update (atomic), extracted data deletion,
/// and blob deletion only when no other attachment in this packet references
/// the same checksum. Sermon content and canonical Markdown are untouched.
pub fn remove_attachment(
    vault: &Path,
    sermon_id: &str,
    attachment_id: &str,
) -> StoreResult<()> {
    let mut manifest = load_manifest(vault, sermon_id)?;
    let idx = manifest
        .attachments
        .iter()
        .position(|a| a.id == attachment_id)
        .ok_or_else(|| StoreError::AttachmentNotFound(attachment_id.to_string()))?;
    let removed = manifest.attachments.remove(idx);

    // Blob shared by another attachment in this packet? Keep it.
    let blob_still_referenced = manifest.attachments.iter().any(|a| a.checksum == removed.checksum);

    save_manifest(vault, sermon_id, &manifest)?;

    let dir = packet_dir(vault, sermon_id);
    let pages_path = confine_path(
        &dir,
        Path::new("extracted").join(format!("{}.pages.json", removed.checksum)).as_path(),
    )?;
    if pages_path.exists() {
        std::fs::remove_file(&pages_path)
            .map_err(|e| io_err(format!("remove {}: {e}", pages_path.display())))?;
    }
    if !blob_still_referenced {
        let blob = confine_path(&dir, Path::new(&removed.stored_filename))?;
        if blob.exists() {
            std::fs::remove_file(&blob)
                .map_err(|e| io_err(format!("remove {}: {e}", blob.display())))?;
        }
    }
    Ok(())
}

// ── Guards ───────────────────────────────────────────────────────────────────

fn require_uuid(sermon_id: &str) -> StoreResult<()> {
    if !is_valid_uuid(sermon_id) {
        return Err(StoreError::Packet(PacketError::InvalidSermonId(
            sermon_id.to_string(),
        )));
    }
    Ok(())
}

/// Keep the import module honest about where packets live (used by tests and
/// the Tauri layer to build display paths without duplicating the pattern).
pub fn attachments_root(vault: &Path) -> PathBuf {
    vault.join(ATTACHMENTS_DIR)
}
