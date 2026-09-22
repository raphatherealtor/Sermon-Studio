//! # Track O — Research Packet store: real import/extraction path tests
//!
//! PDFs are generated in-test with `lopdf` (pure Rust, no network, no
//! subprocess): text PDFs, a two-page page-aware PDF, an image-only-style PDF
//! (empty text layer), and corrupt variants. The constitutional sentinel
//! (`RESEARCH_PACKET_SENTINEL_9F3A7`) is carried through the REAL
//! attach → extract → store path and proven never to reach the pastor-authored
//! FTS index, Sermon Intelligence, or canonical Markdown.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use sermon_core::provenance::{is_lifetime_corpus, Provenance, ProvenanceClass};
use sermon_core::research_packet::{self, ExtractionStatus};
use sermon_core::research_store::{self, AttachmentMetadata, ImportRequest, StoreError};
use std::path::{Path, PathBuf};

const SENTINEL: &str = "RESEARCH_PACKET_SENTINEL_9F3A7";
const SERMON_ID: &str = "11111111-1111-4111-8111-111111111111";
const SERMON_ID_2: &str = "22222222-2222-4222-8222-222222222222";

// ── Test PDF builders (lopdf) ────────────────────────────────────────────────

fn text_operations(text: &str) -> Vec<Operation> {
    vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 24.into()]),
        Operation::new("Td", vec![72.into(), 720.into()]),
        Operation::new("Tj", vec![Object::string_literal(text)]),
        Operation::new("ET", vec![]),
    ]
}

fn pdf_with_pages(page_texts: &[Option<&str>]) -> Vec<u8> {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let mut kids = Vec::new();
    for text in page_texts {
        let ops = match text {
            Some(t) => text_operations(t),
            // No text operators at all: an image-only/scanned-style page.
            None => Vec::new(),
        };
        let content = Content { operations: ops };
        let content_id = doc.add_object(
            Stream::new(dictionary! {}, content.encode().expect("encode content")),
        );
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        kids.push(page_id.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => page_texts.len() as u32,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save pdf to memory");
    out
}

// ── Temp vault helpers (no dev-tempfile semantics needed beyond a dir) ───────

struct TempVault(PathBuf);
impl TempVault {
    fn new() -> Self {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!(
            "sermon-tracko-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&p).unwrap();
        TempVault(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_source(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&p, bytes).unwrap();
    p
}

fn import(vault: &Path, sermon_id: &str, src: &Path) -> research_packet::ResearchAttachment {
    research_store::import_attachment(
        vault,
        sermon_id,
        &ImportRequest {
            source_path: src.to_path_buf(),
            metadata: AttachmentMetadata::default(),
        },
    )
    .unwrap()
}

fn packet_dir(vault: &Path, sermon_id: &str) -> PathBuf {
    research_packet::packet_dir(vault, sermon_id)
}

// ── 1–2. Attach a text PDF; SHA-256 addressing ───────────────────────────────

#[test]
fn attach_text_pdf_extracts_page_aware_text() {
    let vault = TempVault::new();
    let src = write_source(
        vault.path(),
        "notes.pdf",
        &pdf_with_pages(&[Some("Grace abounds in the first page.")]),
    );
    let att = import(vault.path(), SERMON_ID, &src);

    assert_eq!(att.extraction, ExtractionStatus::Ok);
    assert_eq!(att.page_count, Some(1));
    assert_eq!(att.provenance_class, ProvenanceClass::ResearchPacket);
    assert_eq!(att.original_filename, "notes.pdf");
    assert_eq!(att.mime_type, "application/pdf");

    let pages = research_store::get_extracted_pages(vault.path(), SERMON_ID, &att.id).unwrap();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].text.contains("Grace abounds"));
}

#[test]
fn stored_blob_is_content_addressed_by_sha256() {
    let vault = TempVault::new();
    let bytes = pdf_with_pages(&[Some("Content addressed storage test.")]);
    let src = write_source(vault.path(), "deep/whatever.PDF", &bytes);
    let att = import(vault.path(), SERMON_ID, &src);

    let expected = research_packet::sha256_hex(&bytes);
    assert_eq!(att.checksum, expected);
    assert_eq!(att.stored_filename, format!("{expected}.pdf"));
    // The blob lives inside the packet dir under its content address.
    let blob = packet_dir(vault.path(), SERMON_ID).join(&att.stored_filename);
    assert!(blob.is_file());
    assert_eq!(std::fs::read(&blob).unwrap(), bytes);
    // The original file is untouched (copy, never move).
    assert!(src.is_file());
}

// ── 3. Duplicate handling ────────────────────────────────────────────────────

#[test]
fn same_sha256_is_idempotent_no_duplicate_blob_or_entry() {
    let vault = TempVault::new();
    let bytes = pdf_with_pages(&[Some("Duplicate handling.")]);
    let src1 = write_source(vault.path(), "a.pdf", &bytes);
    let src2 = write_source(vault.path(), "b.pdf", &bytes);

    let first = import(vault.path(), SERMON_ID, &src1);
    let second = import(vault.path(), SERMON_ID, &src2);

    assert_eq!(first.id, second.id, "same content → same attachment");
    let all = research_store::list_attachments(vault.path(), SERMON_ID).unwrap();
    assert_eq!(all.len(), 1);
    // One blob on disk.
    let dir = packet_dir(vault.path(), SERMON_ID);
    let blobs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".pdf"))
        .collect();
    assert_eq!(blobs.len(), 1);
}

// ── 4. Ownership validation ──────────────────────────────────────────────────

#[test]
fn cross_sermon_access_is_rejected() {
    let vault = TempVault::new();
    let src = write_source(vault.path(), "x.pdf", &pdf_with_pages(&[Some("Owned.")]));
    let att = import(vault.path(), SERMON_ID, &src);

    // Cross-sermon access must not resolve the attachment (existence is not
    // leaked across sermons — a foreign sermon sees "not found").
    assert!(matches!(
        research_store::get_attachment(vault.path(), SERMON_ID_2, &att.id),
        Err(StoreError::AttachmentNotFound(_))
    ));
    assert!(matches!(
        research_store::remove_attachment(vault.path(), SERMON_ID_2, &att.id),
        Err(StoreError::AttachmentNotFound(_))
    ));
    assert!(matches!(
        research_store::get_extracted_pages(vault.path(), SERMON_ID_2, &att.id),
        Err(StoreError::AttachmentNotFound(_))
    ));
}

#[test]
fn non_uuid_sermon_ids_are_rejected() {
    let vault = TempVault::new();
    let src = write_source(vault.path(), "x.pdf", &pdf_with_pages(&[Some("x")]));
    assert!(matches!(
        research_store::import_attachment(
            vault.path(),
            "sermon-001",
            &ImportRequest {
                source_path: src.to_path_buf(),
                metadata: AttachmentMetadata::default(),
            },
        ),
        Err(StoreError::Packet(
            research_packet::PacketError::InvalidSermonId(_)
        ))
    ));
}

// ── 5–6. Traversal prevention / Windows-style paths ─────────────────────────

#[test]
fn traversal_names_cannot_escape_the_packet_dir() {
    let vault = TempVault::new();
    // A hostile *original filename* must not appear on disk; storage is
    // content-addressed and confined.
    let bytes = pdf_with_pages(&[Some("Traversal safety.")]);
    let hostile_dir = vault.path().join("hostile");
    let src = write_source(&hostile_dir, "..%2f..%2fevil.pdf", &bytes);
    let att = import(vault.path(), SERMON_ID, &src);

    let dir = packet_dir(vault.path(), SERMON_ID);
    let blob = research_store::stored_file_path(vault.path(), SERMON_ID, &att.id).unwrap();
    assert!(blob.starts_with(&dir), "blob confined to packet dir");
    assert!(!vault.path().join("evil.pdf").exists());

    // Component-aware predicates: backslash paths behave like forward slash.
    let win = Path::new("C:\\vault\\.sermon-studio\\attachments\\s1\\abc.pdf");
    assert!(!research_packet::is_indexable_as_pastor_content(win));
    // Direct confine escape attempts still fail.
    assert!(matches!(
        research_packet::confine_path(&dir, Path::new("../../../../etc/passwd")),
        Err(research_packet::PacketError::PathEscape(_))
    ));
}

// ── 7. Page-aware extraction ─────────────────────────────────────────────────

#[test]
fn two_page_pdf_yields_two_pages_with_distinct_text() {
    let vault = TempVault::new();
    let src = write_source(
        vault.path(),
        "two.pdf",
        &pdf_with_pages(&[Some("FIRST PAGE UNIQUE MARKER"), Some("SECOND PAGE UNIQUE MARKER")]),
    );
    let att = import(vault.path(), SERMON_ID, &src);
    assert_eq!(att.page_count, Some(2));

    let pages = research_store::get_extracted_pages(vault.path(), SERMON_ID, &att.id).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].page, 1);
    assert_eq!(pages[1].page, 2);
    assert!(pages[0].text.contains("FIRST PAGE UNIQUE MARKER"));
    assert!(pages[1].text.contains("SECOND PAGE UNIQUE MARKER"));
}

// ── 8. Image-only / no-text degradation ─────────────────────────────────────

#[test]
fn image_only_pdf_reports_no_text_and_never_invents_content() {
    let vault = TempVault::new();
    let src = write_source(vault.path(), "scan.pdf", &pdf_with_pages(&[None]));
    let att = import(vault.path(), SERMON_ID, &src);

    assert_eq!(att.extraction, ExtractionStatus::NoText);
    assert_eq!(att.page_count, None);
    // No extracted text exists — and none was fabricated.
    let pages = research_store::get_extracted_pages(vault.path(), SERMON_ID, &att.id).unwrap();
    assert!(pages.is_empty());
}

// ── 9. Malformed PDF fails safely ────────────────────────────────────────────

#[test]
fn malformed_pdf_fails_safely_with_failed_status() {
    let vault = TempVault::new();
    // Right magic, corrupt body: passes the content sniff, breaks extraction.
    let corrupt = b"%PDF-1.4\nthis is not a real pdf body at all\n%%EOF";
    let src = write_source(vault.path(), "corrupt.pdf", corrupt);
    let att = import(vault.path(), SERMON_ID, &src);
    assert_eq!(att.extraction, ExtractionStatus::Failed);
    // The attachment and blob still exist; nothing panicked, nothing leaked.
    assert!(research_store::stored_file_path(vault.path(), SERMON_ID, &att.id).is_ok());

    // Wrong content type is refused outright.
    let not_pdf = write_source(vault.path(), "fake.pdf", b"plain text, no magic");
    assert!(matches!(
        research_store::import_attachment(
            vault.path(),
            SERMON_ID,
            &ImportRequest {
                source_path: not_pdf,
                metadata: AttachmentMetadata::default(),
            },
        ),
        Err(StoreError::NotPdf)
    ));
}

// ── 10. Oversized file rejected ──────────────────────────────────────────────

#[test]
fn oversized_attachment_is_rejected_before_storage() {
    let vault = TempVault::new();
    let bytes = pdf_with_pages(&[Some("This file is definitely larger than sixteen bytes.")]);
    let src = write_source(vault.path(), "big.pdf", &bytes);

    let result = research_store::import_attachment_bounded(
        vault.path(),
        SERMON_ID,
        &ImportRequest {
            source_path: src,
            metadata: AttachmentMetadata::default(),
        },
        16,
        true,
    );
    assert!(matches!(result, Err(StoreError::TooLarge { .. })));
    // Nothing was written.
    assert!(!packet_dir(vault.path(), SERMON_ID).exists());
}

// ── 11. Manifest atomicity ───────────────────────────────────────────────────

#[test]
fn manifest_is_written_atomically_with_no_temp_artifacts() {
    let vault = TempVault::new();
    let src = write_source(vault.path(), "m.pdf", &pdf_with_pages(&[Some("Atomic manifest.")]));
    import(vault.path(), SERMON_ID, &src);

    let dir = packet_dir(vault.path(), SERMON_ID);
    let manifest_raw = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    let manifest: research_packet::ResearchPacketManifest =
        serde_json::from_str(&manifest_raw).unwrap();
    manifest.validate().unwrap();
    assert_eq!(manifest.attachments.len(), 1);

    // No temp/leftover artifacts from atomic writes anywhere in the packet.
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let name = entry.file_name();
        assert!(
            !sermon_core::atomic_save::is_temp_artifact(&name.to_string_lossy()),
            "temp artifact left behind: {name:?}"
        );
    }
}

// ── 12. Removal behavior ─────────────────────────────────────────────────────

#[test]
fn removal_cleans_blob_and_extracted_data() {
    let vault = TempVault::new();
    let src = write_source(vault.path(), "r.pdf", &pdf_with_pages(&[Some("Removable.")]));
    let att = import(vault.path(), SERMON_ID, &src);
    let dir = packet_dir(vault.path(), SERMON_ID);
    let blob = dir.join(&att.stored_filename);
    let pages = dir.join("extracted").join(format!("{}.pages.json", att.checksum));
    assert!(blob.exists() && pages.exists());

    research_store::remove_attachment(vault.path(), SERMON_ID, &att.id).unwrap();

    assert!(research_store::list_attachments(vault.path(), SERMON_ID)
        .unwrap()
        .is_empty());
    assert!(!blob.exists(), "blob removed when unreferenced");
    assert!(!pages.exists(), "extracted data removed");
    // Manifest still valid (empty packet).
    let m = research_store::load_manifest(vault.path(), SERMON_ID).unwrap();
    m.validate().unwrap();
}

#[test]
fn shared_blob_survives_until_last_reference_is_removed() {
    let vault = TempVault::new();
    let bytes = pdf_with_pages(&[Some("Shared blob.")]);
    let src = write_source(vault.path(), "shared.pdf", &bytes);
    let att = import(vault.path(), SERMON_ID, &src);

    // Craft a second metadata reference to the same blob (a legal manifest
    // state: unique ids, checksum-matching stored name).
    let mut manifest = research_store::load_manifest(vault.path(), SERMON_ID).unwrap();
    let mut twin = att.clone();
    twin.id = uuid::Uuid::new_v4().to_string();
    twin.original_filename = "shared-copy.pdf".to_string();
    manifest.attachments.push(twin.clone());
    research_store::save_manifest(vault.path(), SERMON_ID, &manifest).unwrap();

    research_store::remove_attachment(vault.path(), SERMON_ID, &att.id).unwrap();
    let blob = packet_dir(vault.path(), SERMON_ID).join(&att.stored_filename);
    assert!(blob.exists(), "blob kept while another attachment references it");
    // Extracted data was per-checksum and is gone with the first removal.
    assert!(!packet_dir(vault.path(), SERMON_ID)
        .join("extracted")
        .join(format!("{}.pages.json", att.checksum))
        .exists());

    research_store::remove_attachment(vault.path(), SERMON_ID, &twin.id).unwrap();
    assert!(!blob.exists(), "blob removed with the last reference");
}

// ── 13. Packet provenance labels ─────────────────────────────────────────────

#[test]
fn attachments_carry_research_packet_provenance_labels() {
    let vault = TempVault::new();
    let src = write_source(vault.path(), "p.pdf", &pdf_with_pages(&[Some("Provenance.")]));
    let att = import(vault.path(), SERMON_ID, &src);

    assert_eq!(att.provenance_class, ProvenanceClass::ResearchPacket);
    let prov = att.provenance(SERMON_ID, Some(1));
    assert_eq!(prov.class, ProvenanceClass::ResearchPacket);
    assert!(!is_lifetime_corpus(&prov));
    assert!(!research_packet::is_intelligence_eligible(&prov));
}

// ── 14–17. Constitutional isolation through the REAL import path ────────────

/// Build a pastor.db with one authored sermon (no sentinel) — minimal version
/// of the constitutional fixture, enough for FTS assertions.
fn authored_db(db_path: &Path, authored_md: &str) -> rusqlite::Connection {
    let mut conn = sermon_core::indexer::open_pastor_db(db_path).unwrap();
    let doc = sermon_core::sermon::SermonDoc::parse(authored_md).unwrap();
    let mut stats = sermon_core::indexer::IndexStats::default();
    sermon_core::indexer::index_single(&mut conn, "Sermons/grace.md", &doc, &mut stats).unwrap();
    conn
}

fn fts_hits(conn: &rusqlite::Connection, term: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM sermons_fts WHERE sermons_fts MATCH ?1",
        [term],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn sentinel_through_real_import_never_reaches_fts_intelligence_or_markdown() {
    let vault = TempVault::new();
    let sermons = vault.path().join("Sermons");
    std::fs::create_dir_all(&sermons).unwrap();

    // Authored sermon (canonical Markdown, no sentinel).
    let authored = "---\nid: grace\ntitle: \"Grace\"\nprimary_passage: \"Rom.8.28\"\nbig_idea: \"Grace grounds assurance\"\nstructure_type: expository\n---\n\nGrace upon grace.\n";
    let md_path = sermons.join("grace.md");
    std::fs::write(&md_path, authored).unwrap();
    let md_before = std::fs::read(&md_path).unwrap();

    // Attach a REAL research PDF whose text layer carries the sentinel.
    let src = write_source(
        vault.path(),
        "sentinel.pdf",
        &pdf_with_pages(&[Some(
            "This external article contains the marker RESEARCH_PACKET_SENTINEL_9F3A7 which must never leak.",
        )]),
    );
    let att = import(vault.path(), SERMON_ID, &src);

    // The sentinel IS in the packet extracted text (not vacuous).
    let pages = research_store::get_extracted_pages(vault.path(), SERMON_ID, &att.id).unwrap();
    let extracted = pages.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");
    assert!(extracted.contains(SENTINEL), "sentinel must be in extracted packet text");

    // Sneakier: a markdown file physically inside the packet directory must
    // also stay out of the index.
    let dir = packet_dir(vault.path(), SERMON_ID);
    std::fs::write(dir.join("sneaky.md"), format!("sentinel notes {SENTINEL}\n")).unwrap();

    // Real indexer run over the whole vault.
    let db_path = vault.path().join("pastor.db");
    let conn = authored_db(&db_path, authored);
    sermon_core::indexer::sync(vault.path(), &db_path).unwrap();

    // FTS: sentinel absent, authored control token present.
    assert_eq!(fts_hits(&conn, "sentinel"), 0, "sentinel must not reach pastor-authored FTS");
    assert!(fts_hits(&conn, "grace") >= 1, "control token must be indexed");

    // Packet files never became sermon rows.
    let packet_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sermon_index WHERE file_path LIKE '%.sermon-studio%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(packet_rows, 0);

    // Intelligence admission guard drops the real packet provenance.
    let prov: Provenance = att.provenance(SERMON_ID, Some(1));
    assert!(!sermon_core::research_packet::is_intelligence_eligible(&prov));

    // Canonical Markdown unchanged by attach + remove.
    research_store::remove_attachment(vault.path(), SERMON_ID, &att.id).unwrap();
    let md_after = std::fs::read(&md_path).unwrap();
    assert_eq!(md_before, md_after, "canonical Markdown must be untouched");
}

// ── 18. No runtime network ───────────────────────────────────────────────────

#[test]
fn import_uses_no_network_and_copies_local_bytes_only() {
    // The store layer exposes no network client; import reads the selected
    // local path and writes under the vault. Prove the source is copied, not
    // moved, and the vault contains only packet artifacts.
    let vault = TempVault::new();
    let external = std::env::temp_dir().join(format!("sermon-tracko-ext-{}.pdf", std::process::id()));
    std::fs::write(&external, pdf_with_pages(&[Some("Local only.")])).unwrap();
    let att = import(vault.path(), SERMON_ID, &external);
    assert!(external.exists(), "original preserved (copy, never move)");
    let _ = std::fs::remove_file(&external);

    // Everything written lives under .sermon-studio/attachments/<uuid>/.
    let root = research_store::attachments_root(vault.path());
    let blob = research_store::stored_file_path(vault.path(), SERMON_ID, &att.id).unwrap();
    assert!(blob.starts_with(&root));
}
