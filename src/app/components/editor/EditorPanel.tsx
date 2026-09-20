'use client';
import React, { useEffect, useCallback, useRef, useState } from 'react';
import dynamic from 'next/dynamic';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import { Save, AlertTriangle, AlertCircle, Info, CheckCircle, Loader2, ChevronDown, ChevronUp, BookOpen, GitMerge, HardDrive, FileText, Clock, X, Eye, EyeOff,  } from 'lucide-react';
import type { DiffPreparationResult } from '@/lib/backend/types';

const TipTapEditor = dynamic(() => import('./TipTapEditor'), { ssr: false });

const SEVERITY_ICON: Record<string, React.ElementType> = {
  error: AlertCircle,
  warning: AlertTriangle,
  info: Info,
};
const SEVERITY_CLASS: Record<string, string> = {
  error: 'linter-error',
  warning: 'linter-warn',
  info: 'linter-info',
};
const SEVERITY_TEXT: Record<string, string> = {
  error: 'text-alert-red',
  warning: 'text-warn-amber',
  info: 'text-ref-blue',
};

// ── Inline conflict banner ────────────────────────────────────────────────────

interface ConflictBannerProps {
  sermonId: string;
  onResolved: () => void;
}

function ConflictBanner({ sermonId, onResolved }: ConflictBannerProps) {
  const backend = useBackend();
  const { conflictInfo, setConflict, setShowMergeDrawer, showMergeDrawer } = useEditorStore();
  const [resolving, setResolving] = useState(false);
  const [diff, setDiff] = useState<DiffPreparationResult | null>(null);
  const [loadingDiff, setLoadingDiff] = useState(false);

  const loadDiff = useCallback(async () => {
    setLoadingDiff(true);
    try {
      const d = await backend.prepareDiff(sermonId);
      setDiff(d);
    } finally {
      setLoadingDiff(false);
    }
  }, [backend, sermonId]);

  const handleKeepLocal = async () => {
    setResolving(true);
    try {
      await backend.resolveConflict({ sermonId, strategy: 'keep-local' });
      setConflict(null);
      onResolved();
    } finally {
      setResolving(false);
    }
  };

  const handleUseDisk = async () => {
    setResolving(true);
    try {
      await backend.resolveConflict({ sermonId, strategy: 'use-disk' });
      setConflict(null);
      onResolved();
    } finally {
      setResolving(false);
    }
  };

  const handleMerge = async () => {
    if (!showMergeDrawer) {
      setShowMergeDrawer(true);
      await loadDiff();
    } else {
      setShowMergeDrawer(false);
    }
  };

  const handleSaveAs = async () => {
    const path = prompt('Save local version as:', `/home/preacher/sermons/conflict-copy-${sermonId}.md`);
    if (!path) return;
    setResolving(true);
    try {
      await backend.resolveConflict({ sermonId, strategy: 'save-local-as', saveAsPath: path });
      setConflict(null);
      onResolved();
    } finally {
      setResolving(false);
    }
  };

  if (!conflictInfo) return null;

  return (
    <div className="conflict-banner flex-shrink-0">
      <div className="px-4 py-2.5">
        <div className="flex items-start gap-3">
          <GitMerge size={14} className="text-warn-amber flex-shrink-0 mt-0.5" />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-xs font-600 text-warn-amber">Save Conflict Detected</span>
              <span className="text-2xs font-mono-data text-fg-dim">— reconciliation required</span>
            </div>
            <p className="text-xs text-fg-dim mb-2">{conflictInfo.explanation || 'This sermon has been modified both locally and on disk since the last save.'}</p>
            <div className="flex items-center gap-4 text-2xs font-mono-data text-fg-dim mb-2.5">
              <span className="flex items-center gap-1">
                <Clock size={9} />
                Local: {new Date(conflictInfo.localModifiedAt).toLocaleTimeString()}
              </span>
              <span className="flex items-center gap-1">
                <HardDrive size={9} />
                Disk: {new Date(conflictInfo.diskModifiedAt).toLocaleTimeString()}
              </span>
              <span className="flex items-center gap-1">
                <FileText size={9} />
                Disk: {conflictInfo.diskWordCount.toLocaleString()}w
              </span>
              <span className="flex items-center gap-1 text-fg-dim/60">
                {conflictInfo.sourcePath}
              </span>
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <button onClick={handleKeepLocal} disabled={resolving} className="btn-secondary text-2xs py-1 px-2.5">
                {resolving ? <Loader2 size={10} className="animate-spin-slow" /> : null}
                Keep Local
              </button>
              <button onClick={handleUseDisk} disabled={resolving} className="btn-secondary text-2xs py-1 px-2.5">
                Use Disk Version
              </button>
              <button onClick={handleMerge} disabled={loadingDiff} className="btn-warn text-2xs py-1 px-2.5">
                {loadingDiff ? <Loader2 size={10} className="animate-spin-slow" /> : <GitMerge size={10} />}
                {showMergeDrawer ? 'Close Merge' : 'Merge'}
              </button>
              <button onClick={handleSaveAs} disabled={resolving} className="btn-ghost text-2xs py-1 px-2.5">
                Save Local As…
              </button>
            </div>
          </div>
          <button onClick={() => setConflict(null)} className="text-fg-dim hover:text-fg flex-shrink-0">
            <X size={13} />
          </button>
        </div>
      </div>

      {/* Inline merge/diff drawer */}
      {showMergeDrawer && (
        <div className="merge-drawer border-t border-border/50">
          <div className="px-4 py-2 flex items-center gap-2 border-b border-border/30">
            <span className="text-2xs font-mono-data text-fg-dim uppercase tracking-wider">Diff View</span>
            {loadingDiff && <Loader2 size={10} className="animate-spin-slow text-fg-dim" />}
          </div>
          {diff && (
            <div className="p-3 font-mono-data text-2xs space-y-1">
              {diff.hunks.map((hunk, hi) => (
                <div key={`hunk-${hi}`} className="space-y-0.5">
                  <div className="text-fg-dim/50 py-0.5">@@ -{hunk.diskStart},{hunk.diskCount} +{hunk.localStart},{hunk.localCount} @@</div>
                  {hunk.lines.map((line, li) => (
                    <div
                      key={`line-${hi}-${li}`}
                      className={`px-2 py-0.5 rounded-sm ${
                        line.kind === 'added' ? 'diff-added' :
                        line.kind === 'removed' ? 'diff-removed' :
                        'diff-context'
                      }`}
                    >
                      <span className="mr-2 select-none opacity-50">
                        {line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}
                      </span>
                      {line.text}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── EditorPanel ───────────────────────────────────────────────────────────────

export default function EditorPanel() {
  const backend = useBackend();
  const {
    activeDocument,
    isDirty,
    isSaving,
    lastSaved,
    saveError,
    lintFindings,
    isLinting,
    conflictInfo,
    wordCount,
    estimatedMinutes,
    setSaving,
    setLastSaved,
    setSaveError,
    markClean,
    setConflict,
    updateBody,
    updateTitle,
    updateScripture,
    dismissLintFinding,
  } = useEditorStore();

  const [lintOpen, setLintOpen] = useState(true);
  const [lintSeverityFilter, setLintSeverityFilter] = useState<'all' | 'error' | 'warning' | 'info'>('all');
  const [showMetadata, setShowMetadata] = useState(true);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const saveDocument = useCallback(async () => {
    if (!activeDocument) return;
    setSaving(true);
    setSaveError(null);
    try {
      const result = await backend.saveSermon(activeDocument);
      if (result.conflict) {
        setConflict({
          localTitle: activeDocument.title,
          localModifiedAt: activeDocument.updatedAt,
          diskModifiedAt: result.conflict.diskUpdatedAt,
          diskVersion: result.conflict.diskVersion,
          diskWordCount: result.conflict.diskWordCount,
          sourcePath: activeDocument.sourcePath || '',
          explanation: 'The file on disk has been modified since your last save. Choose how to reconcile.',
        });
      } else {
        setLastSaved(result.savedAt);
        markClean();
      }
    } catch (e: unknown) {
      setSaveError(e instanceof Error ? e.message : 'Save failed');
    } finally {
      setSaving(false);
    }
  }, [activeDocument, backend, setSaving, setSaveError, setLastSaved, markClean, setConflict]);

  useEffect(() => {
    if (!isDirty) return;
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(saveDocument, 3000);
    return () => { if (saveTimerRef.current) clearTimeout(saveTimerRef.current); };
  }, [isDirty, saveDocument]);

  const formatSavedTime = (ts: string | null) => {
    if (!ts) return null;
    const d = new Date(ts);
    return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
  };

  const visibleFindings = lintFindings.filter(
    (f) => !f.dismissed && (lintSeverityFilter === 'all' || f.severity === lintSeverityFilter)
  );
  const errorCount = lintFindings.filter((f) => !f.dismissed && f.severity === 'error').length;
  const warnCount = lintFindings.filter((f) => !f.dismissed && f.severity === 'warning').length;
  const infoCount = lintFindings.filter((f) => !f.dismissed && f.severity === 'info').length;

  if (!activeDocument) {
    return (
      <div className="flex-1 flex items-center justify-center bg-background">
        <div className="text-center space-y-3">
          <BookOpen size={36} className="text-fg-dim mx-auto" />
          <p className="text-fg-dim text-sm">Select a sermon from the archive</p>
          <p className="text-2xs font-mono-data text-fg-dim/60">or press Cmd+N to create a new one</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 overflow-hidden bg-background">
      {/* Conflict banner — inline, non-modal, docked at top */}
      {conflictInfo && (
        <ConflictBanner
          sermonId={activeDocument.id}
          onResolved={() => setConflict(null)}
        />
      )}

      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-panel flex-shrink-0 gap-3">
        <div className="flex flex-col min-w-0 flex-1">
          {showMetadata ? (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={activeDocument.title}
                onChange={(e) => updateTitle(e.target.value)}
                className="text-sm font-600 text-fg bg-transparent border-none outline-none flex-1 min-w-0 focus:bg-elevated focus:px-2 focus:rounded transition-all"
                placeholder="Sermon title…"
              />
            </div>
          ) : (
            <span className="text-sm font-600 text-fg truncate">{activeDocument.title}</span>
          )}
          <div className="flex items-center gap-2 mt-0.5">
            <input
              type="text"
              value={activeDocument.scripture}
              onChange={(e) => updateScripture(e.target.value)}
              className="text-2xs font-mono-data text-accent bg-transparent border-none outline-none focus:bg-elevated focus:px-1.5 focus:rounded transition-all"
              placeholder="Scripture reference…"
            />
            {activeDocument.series && (
              <span className="text-2xs text-fg-dim truncate">· {activeDocument.series}</span>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2 flex-shrink-0">
          {/* Directives */}
          <div className="hidden lg:flex items-center gap-1">
            {activeDocument.directives.slice(0, 2).map((d) => (
              <span key={`dir-${d.key}`} className="text-2xs font-mono-data bg-elevated text-fg-dim px-1.5 py-0.5 rounded border border-border">
                {d.key}={d.value}
              </span>
            ))}
          </div>

          {/* Word count + speaking time */}
          <div className="flex items-center gap-1.5 text-2xs font-mono-data text-fg-dim">
            <span>{wordCount.toLocaleString()}w</span>
            <span className="text-border">·</span>
            <span className="flex items-center gap-0.5">
              <Clock size={9} />
              {estimatedMinutes}m
            </span>
          </div>

          {/* Save state */}
          {lastSaved && !isDirty && !isSaving && (
            <span className="text-2xs font-mono-data text-ok-green flex items-center gap-1">
              <CheckCircle size={10} />
              {formatSavedTime(lastSaved)}
            </span>
          )}
          {isDirty && !isSaving && (
            <span className="text-2xs font-mono-data text-warn-amber">● Unsaved</span>
          )}
          {saveError && (
            <span className="text-2xs font-mono-data text-alert-red truncate max-w-32" title={saveError}>
              {saveError}
            </span>
          )}

          {/* Source path */}
          {activeDocument.sourcePath && (
            <span className="hidden xl:block text-2xs font-mono-data text-fg-dim/50 truncate max-w-40" title={activeDocument.sourcePath}>
              {activeDocument.sourcePath.split('/').pop()}
            </span>
          )}

          <button
            onClick={() => setShowMetadata((p) => !p)}
            className="btn-ghost py-1 px-1.5"
            title="Toggle metadata"
          >
            {showMetadata ? <EyeOff size={12} /> : <Eye size={12} />}
          </button>

          <button
            onClick={saveDocument}
            disabled={isSaving || !isDirty}
            className="btn-primary py-1.5 px-3 text-xs"
          >
            {isSaving ? (
              <><Loader2 size={11} className="animate-spin-slow" /><span>Saving…</span></>
            ) : (
              <><Save size={11} /><span>Save</span></>
            )}
          </button>
        </div>
      </div>

      {/* Editor area */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto px-8 py-8">
          <TipTapEditor content={activeDocument.body} onChange={updateBody} />
        </div>
      </div>

      {/* Linter panel */}
      <div className="border-t border-border bg-panel flex-shrink-0">
        <div className="flex items-center gap-2 px-4 py-1.5">
          <button
            onClick={() => setLintOpen((p) => !p)}
            className="flex items-center gap-2 flex-1 hover:bg-elevated rounded px-1 py-0.5 transition-colors"
          >
            {isLinting ? (
              <Loader2 size={10} className="animate-spin-slow text-fg-dim" />
            ) : errorCount > 0 ? (
              <AlertCircle size={10} className="text-alert-red" />
            ) : warnCount > 0 ? (
              <AlertTriangle size={10} className="text-warn-amber" />
            ) : (
              <CheckCircle size={10} className="text-ok-green" />
            )}
            <span className="text-2xs font-mono-data text-fg-dim">
              {isLinting ? 'Linting…' : `${lintFindings.filter((f) => !f.dismissed).length} finding${lintFindings.filter((f) => !f.dismissed).length !== 1 ? 's' : ''}`}
            </span>
            {errorCount > 0 && <span className="text-2xs font-mono-data text-alert-red">{errorCount}E</span>}
            {warnCount > 0 && <span className="text-2xs font-mono-data text-warn-amber">{warnCount}W</span>}
            {infoCount > 0 && <span className="text-2xs font-mono-data text-ref-blue">{infoCount}I</span>}
            <span className="ml-auto text-fg-dim">{lintOpen ? <ChevronDown size={10} /> : <ChevronUp size={10} />}</span>
          </button>

          {/* Severity filter */}
          {lintOpen && (
            <div className="flex items-center gap-1">
              {(['all', 'error', 'warning', 'info'] as const).map((s) => (
                <button
                  key={`lint-filter-${s}`}
                  onClick={() => setLintSeverityFilter(s)}
                  className={`text-2xs font-mono-data px-1.5 py-0.5 rounded transition-colors ${
                    lintSeverityFilter === s ? 'bg-elevated text-fg' : 'text-fg-dim hover:text-fg'
                  }`}
                >
                  {s === 'all' ? 'All' : s.charAt(0).toUpperCase() + s.slice(1)}
                </button>
              ))}
            </div>
          )}
        </div>

        {lintOpen && visibleFindings.length > 0 && (
          <div className="border-t border-border/50 max-h-40 overflow-y-auto">
            {visibleFindings.map((finding) => {
              const SeverityIcon = SEVERITY_ICON[finding.severity];
              return (
                <div
                  key={`lint-${finding.id}`}
                  className={`flex items-start gap-2.5 px-4 py-2 border-b border-border/30 ${SEVERITY_CLASS[finding.severity]} cursor-pointer hover:bg-elevated/50 transition-colors`}
                  title={finding.suggestedAction}
                >
                  <SeverityIcon size={10} className={`mt-0.5 flex-shrink-0 ${SEVERITY_TEXT[finding.severity]}`} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className={`text-2xs font-mono-data font-600 ${SEVERITY_TEXT[finding.severity]}`}>
                        {finding.ruleId || finding.code}
                      </span>
                      {finding.location && (
                        <span className="text-2xs font-mono-data text-fg-dim">@ {finding.location}</span>
                      )}
                    </div>
                    <p className="text-xs text-fg/80 leading-snug mt-0.5">{finding.message}</p>
                    {finding.suggestedAction && (
                      <p className="text-2xs text-fg-dim mt-0.5 italic">{finding.suggestedAction}</p>
                    )}
                  </div>
                  <button
                    onClick={() => dismissLintFinding(finding.id)}
                    className="text-fg-dim hover:text-fg flex-shrink-0 mt-0.5"
                    title="Dismiss"
                  >
                    <X size={10} />
                  </button>
                </div>
              );
            })}
          </div>
        )}

        {lintOpen && visibleFindings.length === 0 && !isLinting && (
          <div className="border-t border-border/50 px-4 py-2">
            <p className="text-2xs font-mono-data text-ok-green flex items-center gap-1.5">
              <CheckCircle size={10} />
              {lintFindings.filter((f) => !f.dismissed).length === 0
                ? 'No findings — sermon looks good'
                : `All ${lintSeverityFilter} findings dismissed`}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
