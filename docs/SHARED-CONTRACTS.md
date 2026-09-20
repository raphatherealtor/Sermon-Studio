# Locked shared contract baseline

This checkpoint freezes the existing interfaces for the three backend tracks.
It does not invent a replacement AST or claim the Rocket adapter and original
Tauri commands are already compatible. Implement against the owned Rust source;
coordinate any shared API/schema change before merging it across tracks.

## Shared boundaries

- crates/core/src/reference.rs: VerseRef uses i64 book_num/chapter/verse;
  PassageRef has start/end VerseRef. Preserve parse_passage, normalize and
  canonical reference formatting while improving parser internals.
- crates/core/src/sermon.rs: Frontmatter and SermonDoc, YAML frontmatter plus
  CommonMark body/raw/file_hash, remain the existing persistence baseline.
  Track B must make any richer AST additive and preserve existing consumers.
- crates/core/src/indexer.rs: IndexStats and rebuild/sync/open_pastor_db remain
  the indexer boundary. Preserve the on-disk sermon source of truth.
- crates/core/src/schema.rs and schema.sql: shared database definitions;
  coordinate schema migrations with retrieval.rs and indexer.rs.
- crates/core/src/error.rs and lib.rs: shared error/export surface.
- src-tauri/src/commands.rs, config.rs and lib.rs: existing desktop IPC boundary.
- src/lib/backend/SermonBackend.ts and types.ts: current Rocket frontend API.
  TauriSermonBackend.ts is a stub; differing commands/payloads require explicit
  integration work. Do not silently treat the two APIs as interchangeable.
- src/editor/codec/directiveCodec.ts: transport codec only; Rust remains
  authoritative for parsing, AST validation, indexing and export behavior.

## Track ownership

| Branch | Primary implementation scope |
| --- | --- |
| backend/track-a-reference | crates/core/src/reference.rs and books.rs; reference parser tests |
| backend/track-b-ast | crates/core/src/sermon.rs; additive AST modules and tests |
| backend/track-c-indexer | crates/core/src/indexer.rs and retrieval.rs; indexer tests |

Shared public signatures, serialized fields, schema, Cargo dependencies, module
exports, CLI and Tauri integration require coordination across affected tracks.
Do not independently replace another track's implementation. Keep the original
source and current frontend intact while extending behavior in the assigned scope.

contracts.lock.json records Git blob IDs for the exact initial shared files.
This is a review baseline, not an assertion of runtime compatibility or an
approved design for new cross-track types. Changes to shared contracts must
include a reviewed baseline update before dependent tracks consume them.