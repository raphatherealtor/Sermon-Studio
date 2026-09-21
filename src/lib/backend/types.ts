// All domain types shared across backend adapter and UI
// TypeScript defines types and transport contracts only.
// Rust owns: parsing, AST validation, linting logic, SQLite, filesystem, Typst, PDF.

// ── Sermon core ──────────────────────────────────────────────────────────────

export interface SermonSummary {
  id: string;
  title: string;
  scripture: string;
  series: string | null;
  status: SermonStatus;
  wordCount: number;
  createdAt: string;
  updatedAt: string;
  preachedOn: string | null;
  tags: string[];
  isPinned?: boolean;
  hasConflict?: boolean;
  isIndexing?: boolean;
  sourcePath?: string;
  fsState?: FilesystemState;
}

export type SermonStatus = 'draft' | 'in-progress' | 'reviewed' | 'preached' | 'archived';

export type FilesystemState =
  | 'clean' |'local-dirty' |'disk-changed' |'both-changed' |'missing' |'renamed-externally' |'duplicate-detected' |'recovery-available';

export interface SermonDocument {
  id: string;
  title: string;
  subtitle?: string;
  scripture: string;
  series: string | null;
  seriesIndex?: number;
  status: SermonStatus;
  body: string; // canonical Markdown (prose + ::: directive fences) — the TipTap markdown transport renders/edits it; frontmatter is backend-owned
  outline: OutlineNode[];
  tags: string[];
  createdAt: string;
  updatedAt: string;
  preachedOn: string | null;
  version: number;
  directives: DirectiveEntry[];
  sourcePath?: string;
  fsState?: FilesystemState;
  exportHistory?: ExportHistoryEntry[];
}

export interface OutlineNode {
  id: string;
  level: number;
  text: string;
  children: OutlineNode[];
}

export interface DirectiveEntry {
  key: string;
  value: string;
}

// ── Directive transport codec types ──────────────────────────────────────────
// Frontend treats unknown directives as opaque transport data.
// Known directives may receive specialized rendering.
// Unknown directives MUST survive load → editor state → save without semantic loss.

export type DirectiveKind = 'known' | 'unknown';

export interface ParsedDirective {
  kind: DirectiveKind;
  name: string;
  attributes: Record<string, string>;
  body: string;
  rawSource: string; // preserved verbatim for unknown directives
}

export interface CodecRoundTripResult {
  pass: boolean;
  input: string;
  parsed: ParsedDirective[];
  serialized: string;
  diff?: string;
}

// ── CRUD requests ─────────────────────────────────────────────────────────────

export interface CreateSermonRequest {
  title?: string;
  scripture?: string;
  series?: string;
  template?: string;
}

export interface RenameSermonRequest {
  id: string;
  newTitle: string;
}

export interface DuplicateSermonRequest {
  id: string;
  newTitle?: string;
}

// ── Save / conflict ───────────────────────────────────────────────────────────

export interface SaveResult {
  success: boolean;
  savedAt: string;
  version: number;
  conflict?: ConflictInfo;
}

export interface ConflictInfo {
  localTitle: string;
  localModifiedAt: string;
  diskModifiedAt: string;
  diskVersion: number;
  diskWordCount: number;
  sourcePath: string;
  explanation: string;
}

export interface ConflictResolution {
  sermonId: string;
  strategy: 'keep-local' | 'use-disk' | 'merge' | 'save-local-as';
  mergedBody?: string;
  saveAsPath?: string;
}

export interface DiffPreparationResult {
  localLines: string[];
  diskLines: string[];
  hunks: DiffHunk[];
}

export interface DiffHunk {
  localStart: number;
  localCount: number;
  diskStart: number;
  diskCount: number;
  lines: DiffLine[];
}

export interface DiffLine {
  kind: 'context' | 'added' | 'removed';
  text: string;
}

export interface MergePreparationResult {
  base: string;
  local: string;
  disk: string;
  conflicts: MergeConflict[];
}

export interface MergeConflict {
  id: string;
  localLines: string[];
  diskLines: string[];
  resolved?: 'local' | 'disk' | 'custom';
  customText?: string;
}

// ── Filesystem reconciliation ─────────────────────────────────────────────────

export interface FilesystemReconciliationStatus {
  sermonId: string;
  state: FilesystemState;
  sourcePath: string;
  localModifiedAt?: string;
  diskModifiedAt?: string;
  renamedTo?: string;
  duplicatePaths?: string[];
  recoveryPath?: string;
}

export interface RecoveryResult {
  success: boolean;
  recoveredPath: string;
  message: string;
}

export interface ReconnectRequest {
  sermonId: string;
  newPath: string;
}

// ── Search ────────────────────────────────────────────────────────────────────

export interface SearchResult {
  id: string;
  title: string;
  scripture: string;
  snippet: string;
  score: number;
  matchedFields?: string[];
}

export interface SearchFilters {
  status?: SermonStatus[];
  series?: string;
  tags?: string[];
  dateFrom?: string;
  dateTo?: string;
  scriptureBook?: string;
}

// ── References ────────────────────────────────────────────────────────────────
// TypeScript defines the transport contract; Rust owns citation parsing.
// The transport preserves Track A's three resolution states explicitly;
// nothing is collapsed to a generic successful reference.

export type ReferenceResolution = 'definite' | 'ambiguous' | 'invalid';

export interface ReferenceMatch {
  raw: string;
  book: string;
  chapter: number; // 0 when the reference is context-only (e.g. "v.6")
  verse: number | null;
  endVerse: number | null;
  offset: number;
  length: number;
  osisId?: string; // canonical dotted form — present only for definite matches
  resolution: ReferenceResolution;
  reason?: string; // human-readable reason for ambiguous/invalid matches
}

// ── Linting ───────────────────────────────────────────────────────────────────
// Frontend displays backend-provided findings. No lint rules computed in TypeScript.

export interface LintFinding {
  id: string;
  severity: LintSeverity;
  ruleId: string;
  code: string; // legacy alias for ruleId
  message: string;
  location?: string;
  movementId?: string;
  blockId?: string;
  sourceRange?: SourceRange;
  suggestedAction?: string;
  dismissed?: boolean;
}

export type LintSeverity = 'error' | 'warning' | 'info';

export interface SourceRange {
  startLine: number;
  startCol: number;
  endLine: number;
  endCol: number;
}

export interface LintSummary {
  errorCount: number;
  warningCount: number;
  infoCount: number;
  total: number;
  ruleBreakdown: Record<string, number>;
}

// ── Study rail ────────────────────────────────────────────────────────────────

export interface PassageResult {
  reference: string;
  text: string;
  translation: string;
  verses: VerseEntry[];
  osisRef?: string;
}

export interface VerseEntry {
  verse: number;
  text: string;
}

export interface StrongsEntry {
  id: string;
  lemma: string;
  transliteration: string;
  definition: string;
  gloss: string;
  partOfSpeech: string;
  occurrences: number;
  usageExamples?: StrongsUsageExample[];
  relatedIds?: string[];
}

export interface StrongsUsageExample {
  reference: string;
  text: string;
}

export interface CrossReference {
  reference: string;
  snippet: string;
  relevance: number;
  category?: string;
}

export interface PreachedResult {
  sermonId: string;
  sermonTitle: string;
  preachedOn: string;
  series: string | null;
  wordCount?: number;
}

// ── Index / archive ───────────────────────────────────────────────────────────

export interface IndexOperationResult {
  success: boolean;
  message: string;
  documentsIndexed?: number;
  durationMs?: number;
  errors?: string[];
}

export interface IndexStatus {
  indexedFileCount: number;
  indexVersion: string;
  lastReconciliationTime: string | null;
  lastFullScanTime: string | null;
  status: 'idle' | 'indexing' | 'reconciling' | 'error';
  pendingFiles?: number;
  errorMessage?: string;
}

export interface ArchiveStats {
  totalSermons: number;
  totalSeries: number;
  totalWords: number;
  lastPreachedOn: string | null;
  oldestSermon: string | null;
  newestSermon: string | null;
  sermonsByStatus: Record<string, number>;
  sermonsByMonth: { month: string; count: number }[];
  sermonsByBook?: { book: string; count: number }[];
  averageWordCount?: number;
  longestSermon?: { id: string; title: string; wordCount: number };
  shortestSermon?: { id: string; title: string; wordCount: number };
  sermonLengthDistribution?: { bucket: string; count: number }[];
  applicationDensity?: number;
  unresolvedLintCount?: number;
  recentActivity?: RecentActivityEntry[];
}

export interface RecentActivityEntry {
  sermonId: string;
  sermonTitle: string;
  action: 'created' | 'edited' | 'preached' | 'exported' | 'archived';
  timestamp: string;
}

export interface IllustrationFatigueResult {
  illustration: string;
  useCount: number;
  lastUsedIn: string;
  lastUsedOn: string;
  severity: 'high' | 'medium' | 'low';
  sermonIds?: string[];
}

// ── Export ────────────────────────────────────────────────────────────────────
// Frontend builds the request and displays results.
// Rust owns: Typst rendering, PDF generation, filesystem write.
//
// Canonical export choices:
//   pulpit_manuscript — modes: manuscript | outline | combined
//   church_bulletin

export type ExportFormat = 'pulpit_manuscript' | 'church_bulletin';

export type PulpitManuscriptMode = 'manuscript' | 'outline' | 'combined';

export interface ExportRequest {
  sermonId: string;
  format: ExportFormat;
  /** Required when format === 'pulpit_manuscript' */
  manuscriptMode?: PulpitManuscriptMode;
  options: ExportOptions;
  snapshotId?: string; // if provided, export from immutable snapshot
}

export interface ExportOptions {
  pageSize?: 'letter' | 'a4' | 'a5';
  marginTop?: number;
  marginBottom?: number;
  marginLeft?: number;
  marginRight?: number;
  fontSize?: number;
  typographyPreset?: 'default' | 'outline-heavy' | 'manuscript' | 'notes' | 'handout';
  includeTitlePage?: boolean;
  includeScriptureReferences?: boolean;
  includeNotes?: boolean;
  includeIllustrations?: boolean;
  includeOutline?: boolean;
  includeScripture?: boolean;
  outputFilename?: string;
  outputPath?: string;
  template?: string;
  sermonMetadata?: Record<string, string>;
}

export interface ExportResult {
  success: boolean;
  outputPath?: string;
  message: string;
  format: string;
  snapshotId?: string;
  exportedAt?: string;
  fileSizeBytes?: number;
}

export interface ExportSnapshot {
  snapshotId: string;
  sermonId: string;
  sermonTitle: string;
  createdAt: string;
  revisionHash?: string;
  wordCount: number;
  status: 'pending' | 'ready' | 'exporting' | 'complete' | 'error';
  errorMessage?: string;
}

export interface ExportHistoryEntry {
  snapshotId: string;
  format: ExportFormat;
  exportedAt: string;
  outputPath?: string;
  success: boolean;
}

// ── Settings ──────────────────────────────────────────────────────────────────

export interface AppSettings {
  libraryPath: string;
  librarianEnabled: boolean;
  defaultTranslation: string;
  autosaveIntervalSeconds: number;
  editorFontSize: number;
  editorFont: string;
  spellcheck: boolean;
  focusMode: boolean;
  exportDefaults: ExportOptions;
  keyboardShortcuts: Record<string, string>;
  developerMode: boolean;
}

// ── Export snapshot request ───────────────────────────────────────────────────

export interface CreateExportSnapshotRequest {
  sermonId: string;
}

export interface RevealFileRequest {
  path: string;
}