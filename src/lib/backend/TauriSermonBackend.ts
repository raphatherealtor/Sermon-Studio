// Isolated Tauri IPC adapter.
// Dynamically imports @tauri-apps/api only when instantiated.
// NEVER import this directly in React components — use BackendContext.
// No business logic here — thin IPC calls only.
//
// Error contract:
// - BackendUnavailableError: not running inside the Tauri runtime (browser or
//   static preview). The app must fall back to MockSermonBackend.
// - BackendCommandError: the native command failed; `code` is 'unsupported'
//   for typed unsupported errors and 'command-failed' otherwise.
// - Command names and payload casing match the Rust side exactly
//   (snake_case commands, camelCase payload fields).

import type { SermonBackend } from './SermonBackend';
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
  IntelligenceResult, ChainStudyResult } from './types';
import { isTauriRuntime } from './runtime';

// Dynamic import isolates Tauri dependency from the browser bundle.
// This file must never be imported by React components directly.

export type BackendErrorCode = 'unavailable' | 'unsupported' | 'command-failed';

export class BackendUnavailableError extends Error {
  readonly code: BackendErrorCode = 'unavailable';
  readonly command: string;
  constructor(command: string, cause: unknown) {
    super(
      `Native backend unavailable for "${command}": the Tauri runtime is not ` +
        'present (browser/static preview). Use MockSermonBackend instead.'
    );
    this.name = 'BackendUnavailableError';
    this.command = command;
    if (cause !== undefined) {
      (this as { cause?: unknown }).cause = cause;
    }
  }
}

export class BackendCommandError extends Error {
  readonly code: BackendErrorCode;
  readonly command: string;
  constructor(command: string, message: string) {
    super(`[${command}] ${message}`);
    this.name = 'BackendCommandError';
    this.command = command;
    if (message.startsWith('unsupported:')) {
      this.code = 'unsupported';
    } else {
      this.code = 'command-failed';
    }
  }
}

export function isBackendUnavailableError(e: unknown): e is BackendUnavailableError {
  return e instanceof BackendUnavailableError;
}

async function tauriInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  // Not inside the Tauri webview (browser/static preview, or server/build
  // evaluation): the native backend is unavailable. Checked explicitly so the
  // error semantics stay correct now that @tauri-apps/api is a real dependency
  // (the module import alone can no longer tell us we are outside Tauri).
  if (!isTauriRuntime()) {
    throw new BackendUnavailableError(command, new Error('not running inside the Tauri webview'));
  }
  let invoke: <R>(cmd: string, args?: Record<string, unknown>) => Promise<R>;
  try {
    ({ invoke } = await import('@tauri-apps/api/core'));
  } catch (e) {
    throw new BackendUnavailableError(command, e);
  }
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    const message =
      typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e);
    throw new BackendCommandError(command, message);
  }
}

export class TauriSermonBackend implements SermonBackend {
  async listSermons(): Promise<SermonSummary[]> {
    return tauriInvoke('list_sermons');
  }
  async searchSermons(query: string, filters?: SearchFilters): Promise<SearchResult[]> {
    return tauriInvoke('search_sermons', { query, filters: filters ?? null });
  }
  async createSermon(request?: CreateSermonRequest): Promise<SermonDocument> {
    return tauriInvoke('create_sermon', { request: request ?? {} });
  }
  async loadSermon(id: string): Promise<SermonDocument> {
    return tauriInvoke('load_sermon', { id });
  }
  async saveSermon(doc: SermonDocument): Promise<SaveResult> {
    return tauriInvoke('save_sermon', { doc });
  }
  async renameSermon(request: RenameSermonRequest): Promise<SermonSummary> {
    return tauriInvoke('rename_sermon', { request });
  }
  async duplicateSermon(request: DuplicateSermonRequest): Promise<SermonDocument> {
    return tauriInvoke('duplicate_sermon', { request });
  }
  async archiveSermon(id: string): Promise<void> {
    return tauriInvoke('archive_sermon', { id });
  }
  async deleteSermon(id: string): Promise<void> {
    return tauriInvoke('delete_sermon', { id });
  }
  async pinSermon(id: string, pinned: boolean): Promise<void> {
    return tauriInvoke('pin_sermon', { id, pinned });
  }
  async parseReferences(text: string): Promise<ReferenceMatch[]> {
    return tauriInvoke('parse_references', { text });
  }
  async lintSermon(doc: SermonDocument): Promise<LintFinding[]> {
    return tauriInvoke('lint_sermon', { doc });
  }
  async getPassage(reference: string): Promise<PassageResult> {
    return tauriInvoke('get_passage', { reference });
  }
  async getStrongs(id: string): Promise<StrongsEntry> {
    return tauriInvoke('get_strongs', { id });
  }
  async getCrossReferences(reference: string): Promise<CrossReference[]> {
    return tauriInvoke('get_cross_references', { reference });
  }
  async getPreachedOn(reference: string): Promise<PreachedResult[]> {
    return tauriInvoke('get_preached_on', { reference });
  }
  async getChainStudy(reference: string): Promise<ChainStudyResult> {
    return tauriInvoke('get_chain_study', { reference });
  }
  async syncIndex(): Promise<IndexOperationResult> {
    return tauriInvoke('sync_index');
  }
  async rebuildIndex(): Promise<IndexOperationResult> {
    return tauriInvoke('rebuild_index');
  }
  async rescanLibrary(): Promise<IndexOperationResult> {
    return tauriInvoke('rescan_library');
  }
  async repairIndex(): Promise<IndexOperationResult> {
    return tauriInvoke('repair_index');
  }
  async cancelIndexOperation(): Promise<void> {
    return tauriInvoke('cancel_index_operation');
  }
  async getIndexStatus(): Promise<IndexStatus> {
    return tauriInvoke('get_index_status');
  }
  async getArchiveStats(): Promise<ArchiveStats> {
    return tauriInvoke('get_archive_stats');
  }
  async getIllustrationFatigue(): Promise<IllustrationFatigueResult[]> {
    return tauriInvoke('get_illustration_fatigue');
  }
  async getRelatedSermons(sermonId: string, limit = 10): Promise<IntelligenceResult> {
    return tauriInvoke('get_related_sermons', { sermonId, limit });
  }
  async getPassageHistory(reference: string): Promise<IntelligenceResult> {
    return tauriInvoke('get_passage_history', { reference });
  }
  async getSermonInsights(sermonId: string, limit = 10): Promise<IntelligenceResult> {
    return tauriInvoke('get_sermon_insights', { sermonId, limit });
  }
  async setLibrarianEnabled(enabled: boolean): Promise<void> {
    return tauriInvoke('set_librarian_enabled', { enabled });
  }
  async createExportSnapshot(request: CreateExportSnapshotRequest): Promise<ExportSnapshot> {
    return tauriInvoke('create_export_snapshot', { request });
  }
  async executeExportJob(request: ExportRequest): Promise<ExportResult> {
    return tauriInvoke('execute_export_job', { request });
  }
  async exportSermon(request: ExportRequest): Promise<ExportResult> {
    return tauriInvoke('export_sermon', { request });
  }
  async revealExportedFile(request: RevealFileRequest): Promise<void> {
    return tauriInvoke('reveal_exported_file', { request });
  }
  async resolveConflict(request: ConflictResolution): Promise<SaveResult> {
    return tauriInvoke('resolve_conflict', { request });
  }
  async prepareDiff(sermonId: string): Promise<DiffPreparationResult> {
    return tauriInvoke('prepare_diff', { sermonId });
  }
  async prepareMerge(sermonId: string): Promise<MergePreparationResult> {
    return tauriInvoke('prepare_merge', { sermonId });
  }
  async getFilesystemStatus(sermonId: string): Promise<FilesystemReconciliationStatus> {
    return tauriInvoke('get_filesystem_status', { sermonId });
  }
  async recoverSermon(sermonId: string): Promise<RecoveryResult> {
    return tauriInvoke('recover_sermon', { sermonId });
  }
  async reconnectSermon(request: ReconnectRequest): Promise<SermonSummary> {
    return tauriInvoke('reconnect_sermon', { request });
  }
  async loadSettings(): Promise<AppSettings> {
    return tauriInvoke('load_settings');
  }
  async saveSettings(settings: AppSettings): Promise<void> {
    return tauriInvoke('save_settings', { settings });
  }
  async testDirectiveCodec(input: string): Promise<CodecRoundTripResult> {
    return tauriInvoke('test_directive_codec', { input });
  }
}
