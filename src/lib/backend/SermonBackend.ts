// The single injected interface all React components use.
// Components must NOT import MockSermonBackend or TauriSermonBackend directly.
// All backend behavior flows through this interface.

import type {
  SermonSummary,
  SermonDocument,
  CreateSermonRequest,
  RenameSermonRequest,
  DuplicateSermonRequest,
  SaveResult,
  SearchResult,
  SearchFilters,
  ReferenceMatch,
  LintFinding,
  PassageResult,
  StrongsEntry,
  CrossReference,
  PreachedResult,
  IndexOperationResult,
  IndexStatus,
  ArchiveStats,
  IllustrationFatigueResult,
  ExportRequest,
  ExportResult,
  ExportSnapshot,
  CreateExportSnapshotRequest,
  ConflictResolution,
  DiffPreparationResult,
  MergePreparationResult,
  FilesystemReconciliationStatus,
  RecoveryResult,
  ReconnectRequest,
  RevealFileRequest,
  AppSettings,
  CodecRoundTripResult,
} from './types';

export interface SermonBackend {
  // ── Sermon list / archive ──────────────────────────────────────────────────
  listSermons(): Promise<SermonSummary[]>;
  searchSermons(query: string, filters?: SearchFilters): Promise<SearchResult[]>;
  createSermon(request?: CreateSermonRequest): Promise<SermonDocument>;
  loadSermon(id: string): Promise<SermonDocument>;
  saveSermon(doc: SermonDocument): Promise<SaveResult>;
  renameSermon(request: RenameSermonRequest): Promise<SermonSummary>;
  duplicateSermon(request: DuplicateSermonRequest): Promise<SermonDocument>;
  archiveSermon(id: string): Promise<void>;
  deleteSermon(id: string): Promise<void>;
  pinSermon(id: string, pinned: boolean): Promise<void>;

  // ── Compiler / analysis (Rust-owned; frontend displays results) ────────────
  parseReferences(text: string): Promise<ReferenceMatch[]>;
  lintSermon(doc: SermonDocument): Promise<LintFinding[]>;

  // ── Study rail ────────────────────────────────────────────────────────────
  getPassage(reference: string): Promise<PassageResult>;
  getStrongs(id: string): Promise<StrongsEntry>;
  getCrossReferences(reference: string): Promise<CrossReference[]>;
  getPreachedOn(reference: string): Promise<PreachedResult[]>;

  // ── Index / archive stats ─────────────────────────────────────────────────
  syncIndex(): Promise<IndexOperationResult>;
  rebuildIndex(): Promise<IndexOperationResult>;
  rescanLibrary(): Promise<IndexOperationResult>;
  repairIndex(): Promise<IndexOperationResult>;
  cancelIndexOperation(): Promise<void>;
  getIndexStatus(): Promise<IndexStatus>;
  getArchiveStats(): Promise<ArchiveStats>;
  getIllustrationFatigue(): Promise<IllustrationFatigueResult[]>;

  // ── Librarian ─────────────────────────────────────────────────────────────
  setLibrarianEnabled(enabled: boolean): Promise<void>;

  // ── Export ────────────────────────────────────────────────────────────────
  createExportSnapshot(request: CreateExportSnapshotRequest): Promise<ExportSnapshot>;
  executeExportJob(request: ExportRequest): Promise<ExportResult>;
  exportSermon(request: ExportRequest): Promise<ExportResult>; // convenience alias
  revealExportedFile(request: RevealFileRequest): Promise<void>;

  // ── Conflict / filesystem reconciliation ──────────────────────────────────
  resolveConflict(request: ConflictResolution): Promise<SaveResult>;
  prepareDiff(sermonId: string): Promise<DiffPreparationResult>;
  prepareMerge(sermonId: string): Promise<MergePreparationResult>;
  getFilesystemStatus(sermonId: string): Promise<FilesystemReconciliationStatus>;
  recoverSermon(sermonId: string): Promise<RecoveryResult>;
  reconnectSermon(request: ReconnectRequest): Promise<SermonSummary>;

  // ── Settings ──────────────────────────────────────────────────────────────
  loadSettings(): Promise<AppSettings>;
  saveSettings(settings: AppSettings): Promise<void>;

  // ── Developer / codec test ────────────────────────────────────────────────
  testDirectiveCodec(input: string): Promise<CodecRoundTripResult>;
}