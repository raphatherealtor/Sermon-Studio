//! Data-integrity checks use isolated temporary vaults; no user sermon is read.

use sermon_core::{atomic_save, indexer, reconcile, retrieval, Sermon};
use std::fs;

fn sermon(id: &str, title: &str, body: &str) -> String {
    format!("---\nid: {id}\ntitle: {title}\nprimary_passage: John 6:35\n---\n\n{body}\n")
}

fn indexed_count(db: &std::path::Path) -> i64 {
    indexer::open_pastor_db(db)
        .unwrap()
        .query_row("SELECT count(*) FROM sermon_index", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn unusual_frontmatter_never_rewrites_files_or_blocks_a_good_sermon() {
    let cases = [
        ("missing-id", "---\ntitle: Missing ID\n---\n\nA body.", false),
        ("malformed-id", "---\nid: not-a-uuid\ntitle: Opaque ID\n---\n\nA body.", false),
        ("missing-title", "---\nid: untitled\n---\n\nA body.", false),
        ("missing-passage", "---\nid: no-passage\ntitle: No Passage\n---\n\nA body.", false),
        ("malformed-date", "---\nid: bad-date\ntitle: Bad Date\ndate_preached: not-a-date\n---\n\nA body.", false),
        ("unknown-field", "---\nid: unknown-field\ntitle: Unknown Field\ncustom_field: keep-me\n---\n\nA body.", false),
        ("duplicate-key", "---\nid: duplicate\nid: second\ntitle: Duplicate Key\n---\n\nA body.", true),
        ("empty-frontmatter", "---\n---\n\nA body.", false),
        ("no-frontmatter", "A body without frontmatter.", false),
        ("broken-yaml", "---\nid: broken\ntitle: \"unterminated\n---\n\nA body.", true),
    ];
    for (name, raw, should_exclude) in cases {
        let tmp = tempfile::tempdir().unwrap();
        let vault = tmp.path().join("Sermons");
        fs::create_dir(&vault).unwrap();
        let db = tmp.path().join("pastor.db");
        fs::write(vault.join("good.md"), sermon("good", "Good Sermon", "safekeyword")).unwrap();
        fs::write(vault.join("adversarial.md"), raw).unwrap();
        let report = reconcile::reconcile(&vault, &db).unwrap();
        assert_eq!(fs::read_to_string(vault.join("adversarial.md")).unwrap(), raw, "{name}");
        assert_eq!(indexed_count(&db), if should_exclude { 1 } else { 2 }, "{name}");
        assert_eq!(report.read_errors.is_empty(), !should_exclude, "{name}");
        let conn = indexer::open_pastor_db(&db).unwrap();
        assert_eq!(retrieval::search_sermons(&conn, "safekeyword", 10).unwrap().len(), 1, "{name}");
    }
}

#[test]
fn external_rename_delete_recreate_and_collision_leave_no_ghost_results() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Sermons");
    fs::create_dir(&vault).unwrap();
    let db = tmp.path().join("pastor.db");
    let original = sermon("stable-id", "First Title", "firstkeyword");
    fs::write(vault.join("a.md"), &original).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();

    fs::create_dir(vault.join("later")).unwrap();
    fs::rename(vault.join("a.md"), vault.join("later/b.md")).unwrap();
    let report = reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(report.renamed, 1);
    assert_eq!(indexed_count(&db), 1);

    fs::write(vault.join("z.md"), sermon("stable-id", "Collision", "loserkeyword")).unwrap();
    let report = reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(report.id_collisions.len(), 1);
    assert_eq!(indexed_count(&db), 1);
    let conn = indexer::open_pastor_db(&db).unwrap();
    assert!(retrieval::search_sermons(&conn, "loserkeyword", 10).unwrap().is_empty());
    drop(conn);

    fs::remove_file(vault.join("z.md")).unwrap();
    fs::write(vault.join("later/b.md"), sermon("stable-id", "Changed Title", "newkeywordxx")).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    let conn = indexer::open_pastor_db(&db).unwrap();
    assert_eq!(retrieval::search_sermons(&conn, "newkeywordxx", 10).unwrap().len(), 1);
    assert_eq!(retrieval::search_sermons(&conn, "Changed", 10).unwrap().len(), 1);
    assert_eq!(retrieval::search_sermons(&conn, "John", 10).unwrap().len(), 1);
    assert!(retrieval::search_sermons(&conn, "firstkeyword", 10).unwrap().is_empty());
    drop(conn);

    fs::remove_file(vault.join("later/b.md")).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(indexed_count(&db), 0);
    fs::write(vault.join("later/b.md"), original).unwrap();
    let report = reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(report.recovered, 1);
    assert_eq!(indexed_count(&db), 1);
}

#[test]
fn unknown_directives_unicode_and_long_body_survive_round_trip_and_indexing() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Sermons");
    fs::create_dir(&vault).unwrap();
    let db = tmp.path().join("pastor.db");
    let long = "abundantword ".repeat(20_000);
    let raw = format!("{}\n:::movement\nKnown.\n:::\n\n:::illustration\nIllustration.\n:::\n\n:::application\nApply.\n:::\n\n:::exegetical-notes\nPrivate.\n:::\n\n:::future-block key=\"value\"\nKeep 😄 “curly” — Unicode.\n:::movement nested-looking\n:::\n\nNormal prose :::unknown stays prose.\n{long}\n", sermon("directives", "Directives", "Body."));
    let parsed = Sermon::parse(&raw).unwrap();
    let markdown = parsed.to_markdown();
    assert!(markdown.contains(":::future-block key=\"value\""));
    assert!(markdown.contains("Keep 😄 “curly” — Unicode."));
    assert!(markdown.contains("Normal prose :::unknown stays prose."));
    assert!(markdown.contains(&long));
    atomic_save::save_sermon_atomic(&vault, "directives.md", &raw).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(fs::read_to_string(vault.join("directives.md")).unwrap(), raw);
    assert_eq!(indexed_count(&db), 1);

    let edge = format!("{}\n:::future-empty\n:::\n\n:::future-empty\n\tduplicate\n:::\n\n:::future-open\nNo closing fence 😄 — keep these bytes.\n", sermon("edge", "Edge", "Prose :::inline stays prose."));
    fs::write(vault.join("edge.md"), &edge).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(fs::read_to_string(vault.join("edge.md")).unwrap(), edge);
    assert_eq!(indexed_count(&db), 2);
}

#[test]
fn failed_save_preserves_file_and_temporary_artifacts_never_index() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Sermons");
    fs::create_dir(&vault).unwrap();
    let db = tmp.path().join("pastor.db");
    let original = sermon("saved", "Saved", "originalkeyword");
    fs::write(vault.join("saved.md"), &original).unwrap();
    let impossible = vault.join("saved.md").join("nested.md");
    assert!(atomic_save::save_atomic(&impossible, b"replacement").is_err());
    assert_eq!(fs::read_to_string(vault.join("saved.md")).unwrap(), original);
    fs::write(vault.join(".saved.md.sstmp-qa.md"), sermon("temp", "Temp", "tempkeyword")).unwrap();
    fs::create_dir_all(vault.join(".sermon-studio/attachments")).unwrap();
    fs::write(vault.join(".sermon-studio/attachments/packet.md"), "packetkeyword").unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(indexed_count(&db), 1);
    let conn = indexer::open_pastor_db(&db).unwrap();
    assert!(retrieval::search_sermons(&conn, "tempkeyword", 10).unwrap().is_empty());
    assert!(retrieval::search_sermons(&conn, "packetkeyword", 10).unwrap().is_empty());
    drop(conn);
    let empty = sermon("empty", "Empty Manuscript", "");
    atomic_save::save_sermon_atomic(&vault, "empty.md", &empty).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(fs::read_to_string(vault.join("empty.md")).unwrap(), empty);
    assert_eq!(fs::read_to_string(vault.join("saved.md")).unwrap(), original);
    assert_eq!(indexed_count(&db), 2);
}

#[test]
fn externally_changed_identity_does_not_leave_an_old_fts_row() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Sermons");
    fs::create_dir(&vault).unwrap();
    let db = tmp.path().join("pastor.db");
    fs::write(vault.join("a.md"), sermon("old-id", "Old", "oldkeyword")).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    fs::write(vault.join("a.md"), sermon("new-id", "New", "newkeyword")).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    let conn = indexer::open_pastor_db(&db).unwrap();
    assert_eq!(indexed_count(&db), 1);
    let fts_count: i64 = conn.query_row("SELECT count(*) FROM sermons_fts", [], |r| r.get(0)).unwrap();
    assert_eq!(fts_count, 1, "old sermon identity must not leave an orphan FTS row");
}

#[test]
fn externally_malformed_file_is_excluded_without_erasing_its_bytes_or_good_results() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Sermons");
    fs::create_dir(&vault).unwrap();
    let db = tmp.path().join("pastor.db");
    fs::write(vault.join("good.md"), sermon("good", "Good", "goodkeyword")).unwrap();
    fs::write(vault.join("bad.md"), sermon("bad", "Previously Valid", "stalekeyword")).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    let broken = "---\nid: bad\ntitle: \"unterminated\n---\n\nnew raw content\n";
    fs::write(vault.join("bad.md"), broken).unwrap();
    let report = reconcile::reconcile(&vault, &db).unwrap();
    assert_eq!(report.read_errors.len(), 1);
    assert_eq!(fs::read_to_string(vault.join("bad.md")).unwrap(), broken);
    let conn = indexer::open_pastor_db(&db).unwrap();
    assert_eq!(retrieval::search_sermons(&conn, "goodkeyword", 10).unwrap().len(), 1);
    assert!(retrieval::search_sermons(&conn, "stalekeyword", 10).unwrap().is_empty());
    assert_eq!(indexed_count(&db), 1);
}

#[test]
fn watcher_path_reconcile_excludes_a_file_that_becomes_malformed() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Sermons");
    fs::create_dir(&vault).unwrap();
    let db = tmp.path().join("pastor.db");
    fs::write(vault.join("a.md"), sermon("a", "A", "stalekeyword")).unwrap();
    reconcile::reconcile(&vault, &db).unwrap();
    let broken = "---\nid: a\ntitle: \"unterminated\n---\n\nuntouched\n";
    fs::write(vault.join("a.md"), broken).unwrap();
    let report = reconcile::reconcile_paths(&vault, &db, &["a.md".to_string()]).unwrap();
    assert_eq!(report.read_errors.len(), 1);
    assert_eq!(fs::read_to_string(vault.join("a.md")).unwrap(), broken);
    assert_eq!(indexed_count(&db), 0);
}
