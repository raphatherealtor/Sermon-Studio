//! Integration tests for the export subsystem, driven from a fixture file
//! (mirrors how the integration track will feed real sermons).

use sermon_core::export::{
    export_sermon, ExportFormat, ExportRequest, ExportOutcome, PulpitMode,
};
use sermon_core::sermon::Sermon;
use std::path::Path;

const FIXTURE_PATH: &str = "tests/fixtures/export_canonical.md";

fn load_fixture() -> (Sermon, String) {
    let raw = std::fs::read_to_string(FIXTURE_PATH).unwrap();
    (Sermon::parse(&raw).unwrap(), raw)
}

fn export_to(sermon: &Sermon, raw: &str, request: ExportRequest, path: &Path) -> ExportOutcome {
    export_sermon(sermon, raw, request, path)
}

#[test]
fn all_canonical_export_targets_render_from_fixture() {
    let (sermon, raw) = load_fixture();
    let tmp = tempfile::tempdir().unwrap();

    let cases = [
        (ExportRequest::pulpit(PulpitMode::Manuscript), "manuscript.pdf"),
        (ExportRequest::pulpit(PulpitMode::Outline), "outline.pdf"),
        (ExportRequest::pulpit(PulpitMode::Combined), "combined.pdf"),
        (ExportRequest::bulletin(), "bulletin.pdf"),
    ];

    for (request, name) in cases {
        let out = tmp.path().join(name);
        let outcome = export_to(&sermon, &raw, request, &out);
        assert!(outcome.success, "{name} failed: {:?}", outcome.error);
        let bytes = std::fs::read(&out).unwrap();
        assert!(!bytes.is_empty(), "{name} is empty");
        assert!(bytes.starts_with(b"%PDF-"), "{name} lacks %PDF- signature");
        // Sanity: embedded serif font is present, so prose is real text.
        assert!(
            bytes.windows(10).any(|w| w == b"Libertinus"),
            "{name} does not embed Libertinus"
        );
    }
}

#[test]
fn rendering_is_byte_deterministic_for_identical_inputs() {
    let (sermon, raw) = load_fixture();
    let tmp = tempfile::tempdir().unwrap();
    let a = export_to(&sermon, &raw, ExportRequest::pulpit(PulpitMode::Combined), &tmp.path().join("a.pdf"));
    let b = export_to(&sermon, &raw, ExportRequest::pulpit(PulpitMode::Combined), &tmp.path().join("b.pdf"));
    assert!(a.success && b.success);
    let bytes_a = std::fs::read(tmp.path().join("a.pdf")).unwrap();
    let bytes_b = std::fs::read(tmp.path().join("b.pdf")).unwrap();
    assert_eq!(
        bytes_a, bytes_b,
        "same inputs must produce byte-identical PDFs"
    );
    assert_eq!(
        a.snapshot.as_ref().unwrap().content_hash(),
        b.snapshot.as_ref().unwrap().content_hash()
    );
}

#[test]
fn fixture_export_reports_everything_track_f_needs() {
    let (sermon, raw) = load_fixture();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("report.pdf");
    let outcome = export_to(&sermon, &raw, ExportRequest::pulpit(PulpitMode::Manuscript), &out);
    assert!(outcome.success);
    assert!(outcome.error.is_none());
    let result = outcome.result.as_ref().unwrap();
    let snapshot = outcome.snapshot.as_ref().unwrap();
    assert_eq!(result.export_id, snapshot.export_id());
    assert_eq!(result.content_hash, snapshot.content_hash());
    assert_eq!(result.output_path, out.display().to_string());
    assert!(result.file_size > 0);
    assert_eq!(result.format, ExportFormat::PulpitManuscript);
    assert_eq!(snapshot.sermon_id(), "vine-sermon");
    assert_eq!(snapshot.schema_version(), sermon_core::export::EXPORT_SNAPSHOT_SCHEMA_VERSION);
}
