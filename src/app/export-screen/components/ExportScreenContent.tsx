'use client';
import React, { useState, useEffect, useCallback } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { FileText, AlignJustify, CheckCircle, XCircle, Loader2, Download, BookOpen, ChevronRight, FolderOpen, Camera, Clock, RefreshCw, AlertTriangle, Info } from 'lucide-react';
import type {
  ExportFormat, PulpitManuscriptMode, ExportOptions, ExportResult, ExportSnapshot, SermonSummary,
} from '@/lib/backend/types';

// Fix 2: Canonical export choices reduced to pulpit_manuscript and church_bulletin only.
const FORMAT_OPTIONS: {
  id: ExportFormat;
  label: string;
  description: string;
  icon: React.ElementType;
}[] = [
  { id: 'pulpit_manuscript', label: 'Pulpit Manuscript', description: 'Full manuscript, outline, or combined — for pulpit use', icon: FileText },
  { id: 'church_bulletin', label: 'Church Bulletin', description: 'Congregation bulletin insert with scripture and outline', icon: AlignJustify },
];

const MANUSCRIPT_MODES: { id: PulpitManuscriptMode; label: string; description: string }[] = [
  { id: 'manuscript', label: 'Full Manuscript', description: 'Complete sermon text for reading' },
  { id: 'outline', label: 'Preaching Outline', description: 'Condensed outline for the pulpit' },
  { id: 'combined', label: 'Combined', description: 'Outline with key manuscript sections' },
];

const TYPOGRAPHY_PRESETS = [
  { value: 'default', label: 'Default Expository' },
  { value: 'outline-heavy', label: 'Outline-Heavy' },
  { value: 'manuscript', label: 'Full Manuscript' },
  { value: 'notes', label: 'Preaching Notes' },
  { value: 'handout', label: 'Congregation Handout' },
];

export default function ExportScreenContent() {
  const backend = useBackend();

  const [sermons, setSermons] = useState<SermonSummary[]>([]);
  const [loadingSermons, setLoadingSermons] = useState(true);
  const [selectedSermonId, setSelectedSermonId] = useState('sermon-001');
  const [selectedFormat, setSelectedFormat] = useState<ExportFormat>('pulpit_manuscript');
  const [manuscriptMode, setManuscriptMode] = useState<PulpitManuscriptMode>('manuscript');

  // Options
  const [pageSize, setPageSize] = useState<'letter' | 'a4' | 'a5'>('letter');
  const [fontSize, setFontSize] = useState(11);
  const [typographyPreset, setTypographyPreset] = useState('default');
  const [includeTitlePage, setIncludeTitlePage] = useState(true);
  const [includeScriptureRefs, setIncludeScriptureRefs] = useState(true);
  const [includeNotes, setIncludeNotes] = useState(true);
  const [includeIllustrations, setIncludeIllustrations] = useState(true);
  const [outputFilename, setOutputFilename] = useState('');
  const [outputPath, setOutputPath] = useState('/home/preacher/sermons/exports');

  // Snapshot
  const [snapshot, setSnapshot] = useState<ExportSnapshot | null>(null);
  const [creatingSnapshot, setCreatingSnapshot] = useState(false);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);

  // Export
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  useEffect(() => {
    backend.listSermons().then((list) => {
      setSermons(list.filter((s) => s.status !== 'archived'));
      setLoadingSermons(false);
    });
  }, [backend]);

  const selectedSermon = sermons.find((s) => s.id === selectedSermonId);

  const createSnapshot = useCallback(async () => {
    if (!selectedSermonId) return;
    setCreatingSnapshot(true);
    setSnapshotError(null);
    setSnapshot(null);
    setExportResult(null);
    try {
      const snap = await backend.createExportSnapshot({ sermonId: selectedSermonId });
      setSnapshot(snap);
    } catch (e: unknown) {
      setSnapshotError(e instanceof Error ? e.message : 'Failed to create snapshot');
    } finally {
      setCreatingSnapshot(false);
    }
  }, [backend, selectedSermonId]);

  const handleExport = async () => {
    if (!snapshot) return;
    setExporting(true);
    setExportError(null);
    setExportResult(null);
    try {
      const options: ExportOptions = {
        pageSize,
        fontSize,
        typographyPreset,
        includeTitlePage,
        includeScriptureReferences: includeScriptureRefs,
        includeNotes,
        includeIllustrations,
        outputFilename: outputFilename || `${selectedSermon?.title?.toLowerCase().replace(/\s+/g, '-') || 'sermon'}`,
        outputPath,
      };
      const result = await backend.executeExportJob({
        sermonId: selectedSermonId,
        format: selectedFormat,
        manuscriptMode: selectedFormat === 'pulpit_manuscript' ? manuscriptMode : undefined,
        options,
        snapshotId: snapshot.snapshotId,
      });
      setExportResult(result);
    } catch (e: unknown) {
      setExportError(e instanceof Error ? e.message : 'Export failed');
    } finally {
      setExporting(false);
    }
  };

  // Fix 3: Use generic "Reveal in File Manager" label
  const handleReveal = async () => {
    if (!exportResult?.outputPath) return;
    await backend.revealExportedFile({ path: exportResult.outputPath });
  };

  const formatLabel = FORMAT_OPTIONS.find((f) => f.id === selectedFormat)?.label || selectedFormat;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-6 py-4 border-b border-border bg-panel flex-shrink-0">
        <Download size={16} className="text-accent" />
        <h1 className="text-lg font-600 text-fg">Export Sermon</h1>
        <ChevronRight size={14} className="text-fg-dim" />
        <span className="text-sm text-fg-dim">{selectedSermon?.title || 'Select a sermon'}</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-screen-xl mx-auto px-6 py-6">
          <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">

            {/* Left: Configuration */}
            <div className="xl:col-span-2 space-y-5">

              {/* Sermon selector */}
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <BookOpen size={13} className="text-accent" /> Select Sermon
                </h2>
                {loadingSermons ? (
                  <div className="flex items-center gap-2 py-4 text-fg-dim">
                    <Loader2 size={14} className="animate-spin-slow" />
                    <span className="text-xs">Loading sermons…</span>
                  </div>
                ) : (
                  <div className="space-y-1.5 max-h-48 overflow-y-auto">
                    {sermons.map((sermon) => (
                      <label
                        key={`export-sermon-${sermon.id}`}
                        className={[
                          'flex items-center gap-3 p-2.5 rounded border cursor-pointer transition-all duration-100',
                          selectedSermonId === sermon.id
                            ? 'border-accent bg-accent/8' : 'border-border hover:border-border/80 hover:bg-elevated',
                        ].join(' ')}
                      >
                        <input
                          type="radio"
                          name="sermonId"
                          value={sermon.id}
                          checked={selectedSermonId === sermon.id}
                          onChange={() => { setSelectedSermonId(sermon.id); setSnapshot(null); setExportResult(null); }}
                          className="hidden"
                        />
                        <div className={`w-3 h-3 rounded-full border-2 flex-shrink-0 ${selectedSermonId === sermon.id ? 'border-accent bg-accent' : 'border-border'}`} />
                        <div className="flex-1 min-w-0">
                          <p className="text-xs font-500 text-fg truncate">{sermon.title}</p>
                          <p className="text-2xs font-mono-data text-accent">{sermon.scripture}</p>
                        </div>
                        <div className="flex items-center gap-2 flex-shrink-0">
                          <span className="text-2xs font-mono-data text-fg-dim">{sermon.wordCount.toLocaleString()}w</span>
                          <span className={`status-badge badge-${sermon.status.replace('-', '')}`}>{sermon.status}</span>
                        </div>
                      </label>
                    ))}
                  </div>
                )}
              </div>

              {/* Format selector */}
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <FileText size={13} className="text-accent" /> Output Format
                </h2>
                <div className="grid grid-cols-2 gap-2">
                  {FORMAT_OPTIONS.map((fmt) => {
                    const FmtIcon = fmt.icon;
                    const isSelected = selectedFormat === fmt.id;
                    return (
                      <label
                        key={`fmt-${fmt.id}`}
                        className={[
                          'flex items-start gap-2.5 p-2.5 rounded border cursor-pointer transition-all duration-100',
                          isSelected ? 'border-accent bg-accent/8' : 'border-border hover:border-border/80 hover:bg-elevated',
                        ].join(' ')}
                      >
                        <input type="radio" name="format" value={fmt.id} checked={isSelected} onChange={() => setSelectedFormat(fmt.id)} className="hidden" />
                        <FmtIcon size={14} className={`mt-0.5 flex-shrink-0 ${isSelected ? 'text-accent' : 'text-fg-dim'}`} />
                        <div>
                          <p className={`text-xs font-600 ${isSelected ? 'text-accent' : 'text-fg'}`}>{fmt.label}</p>
                          <p className="text-2xs text-fg-dim leading-tight mt-0.5">{fmt.description}</p>
                        </div>
                      </label>
                    );
                  })}
                </div>

                {/* Pulpit manuscript mode selector */}
                {selectedFormat === 'pulpit_manuscript' && (
                  <div className="mt-4">
                    <p className="text-xs text-fg-dim mb-2">Manuscript Mode</p>
                    <div className="grid grid-cols-3 gap-2">
                      {MANUSCRIPT_MODES.map((mode) => {
                        const isSelected = manuscriptMode === mode.id;
                        return (
                          <label
                            key={`mode-${mode.id}`}
                            className={[
                              'flex flex-col gap-1 p-2.5 rounded border cursor-pointer transition-all duration-100',
                              isSelected ? 'border-accent bg-accent/8' : 'border-border hover:border-border/80 hover:bg-elevated',
                            ].join(' ')}
                          >
                            <input type="radio" name="manuscriptMode" value={mode.id} checked={isSelected} onChange={() => setManuscriptMode(mode.id)} className="hidden" />
                            <p className={`text-xs font-600 ${isSelected ? 'text-accent' : 'text-fg'}`}>{mode.label}</p>
                            <p className="text-2xs text-fg-dim leading-tight">{mode.description}</p>
                          </label>
                        );
                      })}
                    </div>
                  </div>
                )}
              </div>

              {/* Options */}
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-4">Export Options</h2>
                <div className="space-y-4">
                  <div className="grid grid-cols-3 gap-3">
                    <div>
                      <label className="block text-xs text-fg-dim mb-1">Page Size</label>
                      <select value={pageSize} onChange={(e) => setPageSize(e.target.value as 'letter' | 'a4' | 'a5')} className="input-field text-xs">
                        <option value="letter">US Letter</option>
                        <option value="a4">A4</option>
                        <option value="a5">A5</option>
                      </select>
                    </div>
                    <div>
                      <label className="block text-xs text-fg-dim mb-1">Font Size (pt)</label>
                      <input type="number" min={8} max={16} value={fontSize} onChange={(e) => setFontSize(Number(e.target.value))} className="input-field text-xs" />
                    </div>
                    <div>
                      <label className="block text-xs text-fg-dim mb-1">Typography</label>
                      <select value={typographyPreset} onChange={(e) => setTypographyPreset(e.target.value)} className="input-field text-xs">
                        {TYPOGRAPHY_PRESETS.map((p) => (
                          <option key={p.value} value={p.value}>{p.label}</option>
                        ))}
                      </select>
                    </div>
                  </div>

                  {/* Include/exclude toggles */}
                  <div className="grid grid-cols-2 gap-2">
                    {[
                      { label: 'Title Page', value: includeTitlePage, set: setIncludeTitlePage },
                      { label: 'Scripture References', value: includeScriptureRefs, set: setIncludeScriptureRefs },
                      { label: 'Notes', value: includeNotes, set: setIncludeNotes },
                      { label: 'Illustrations', value: includeIllustrations, set: setIncludeIllustrations },
                    ].map(({ label, value, set }) => (
                      <label key={label} className="flex items-center gap-2 cursor-pointer">
                        <button
                          role="switch"
                          aria-checked={value}
                          onClick={() => set(!value)}
                          className={`toggle-track ${value ? 'active' : ''}`}
                        >
                          <div className="toggle-thumb" />
                        </button>
                        <span className="text-xs text-fg">{label}</span>
                      </label>
                    ))}
                  </div>

                  {/* Output */}
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs text-fg-dim mb-1">Output Filename</label>
                      <input
                        type="text"
                        value={outputFilename}
                        onChange={(e) => setOutputFilename(e.target.value)}
                        placeholder={`${selectedSermon?.title?.toLowerCase().replace(/\s+/g, '-') || 'sermon'}`}
                        className="input-field text-xs font-mono-data"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-fg-dim mb-1">Output Directory</label>
                      <input
                        type="text"
                        value={outputPath}
                        onChange={(e) => setOutputPath(e.target.value)}
                        className="input-field text-xs font-mono-data"
                      />
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Right: Snapshot + Export */}
            <div className="space-y-5">

              {/* Export snapshot */}
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <Camera size={13} className="text-accent" /> Export Snapshot
                </h2>
                <p className="text-xs text-fg-dim mb-3 leading-relaxed">
                  Create an immutable snapshot of the current sermon state before exporting. This ensures the exported file matches a specific revision.
                </p>

                {!snapshot ? (
                  <button
                    onClick={createSnapshot}
                    disabled={creatingSnapshot || !selectedSermonId}
                    className="btn-secondary w-full text-xs"
                  >
                    {creatingSnapshot ? (
                      <><Loader2 size={11} className="animate-spin-slow" /> Creating snapshot…</>
                    ) : (
                      <><Camera size={11} /> Create Snapshot</>
                    )}
                  </button>
                ) : (
                  <div className="space-y-2">
                    <div className="bg-elevated rounded p-3 border border-border space-y-1.5">
                      <div className="flex items-center justify-between">
                        <span className="text-2xs font-mono-data text-fg-dim">Snapshot ID</span>
                        <span className="text-2xs font-mono-data text-accent">{snapshot.snapshotId.slice(0, 20)}…</span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-2xs font-mono-data text-fg-dim">Created</span>
                        <span className="text-2xs font-mono-data text-fg">{new Date(snapshot.createdAt).toLocaleTimeString()}</span>
                      </div>
                      {snapshot.revisionHash && (
                        <div className="flex items-center justify-between">
                          <span className="text-2xs font-mono-data text-fg-dim">Revision</span>
                          <span className="text-2xs font-mono-data text-fg-dim">{snapshot.revisionHash.slice(0, 16)}…</span>
                        </div>
                      )}
                      <div className="flex items-center justify-between">
                        <span className="text-2xs font-mono-data text-fg-dim">Words</span>
                        <span className="text-2xs font-mono-data text-fg">{snapshot.wordCount.toLocaleString()}</span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-2xs font-mono-data text-fg-dim">Status</span>
                        <span className="text-2xs font-mono-data text-ok-green flex items-center gap-1">
                          <CheckCircle size={9} /> {snapshot.status}
                        </span>
                      </div>
                    </div>
                    <button onClick={() => { setSnapshot(null); setExportResult(null); }} className="btn-ghost w-full text-xs">
                      <RefreshCw size={11} /> New Snapshot
                    </button>
                  </div>
                )}

                {snapshotError && (
                  <p className="text-xs text-alert-red mt-2 flex items-center gap-1">
                    <AlertTriangle size={11} /> {snapshotError}
                  </p>
                )}
              </div>

              {/* Export action */}
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <Download size={13} className="text-accent" /> Export
                </h2>

                {!snapshot && (
                  <div className="flex items-start gap-2 p-3 rounded bg-warn/8 border border-warn/20 mb-3">
                    <Info size={12} className="text-warn-amber flex-shrink-0 mt-0.5" />
                    <p className="text-xs text-fg-dim">Create a snapshot first to lock the sermon revision before exporting.</p>
                  </div>
                )}

                <button
                  onClick={handleExport}
                  disabled={exporting || !snapshot}
                  className="btn-primary w-full text-sm"
                >
                  {exporting ? (
                    <><Loader2 size={13} className="animate-spin-slow" /> Exporting…</>
                  ) : (
                    <><Download size={13} /> Export as {formatLabel}</>
                  )}
                </button>

                {exportError && (
                  <div className="mt-3 flex items-start gap-2 p-3 rounded bg-alert-red/10 border border-alert-red/30">
                    <XCircle size={13} className="text-alert-red flex-shrink-0 mt-0.5" />
                    <p className="text-xs text-alert-red">{exportError}</p>
                  </div>
                )}

                {exportResult && (
                  <div className="mt-3 space-y-2">
                    <div className={`flex items-start gap-2 p-3 rounded border ${exportResult.success ? 'bg-ok-green/8 border-ok-green/30' : 'bg-alert-red/8 border-alert-red/30'}`}>
                      {exportResult.success ? (
                        <CheckCircle size={13} className="text-ok-green flex-shrink-0 mt-0.5" />
                      ) : (
                        <XCircle size={13} className="text-alert-red flex-shrink-0 mt-0.5" />
                      )}
                      <div>
                        <p className={`text-xs font-600 ${exportResult.success ? 'text-ok-green' : 'text-alert-red'}`}>
                          {exportResult.success ? 'Export successful' : 'Export failed'}
                        </p>
                        <p className="text-xs text-fg-dim mt-0.5">{exportResult.message}</p>
                      </div>
                    </div>

                    {exportResult.outputPath && (
                      <div className="bg-elevated rounded p-2.5 border border-border space-y-1.5">
                        <p className="text-2xs font-mono-data text-fg-dim">Output path</p>
                        <p className="text-2xs font-mono-data text-fg break-all">{exportResult.outputPath}</p>
                        {exportResult.fileSizeBytes && (
                          <p className="text-2xs font-mono-data text-fg-dim">
                            {(exportResult.fileSizeBytes / 1024).toFixed(1)} KB
                          </p>
                        )}
                        {/* Fix 3: Platform-neutral "Reveal in File Manager" */}
                        <button onClick={handleReveal} className="btn-ghost text-xs w-full mt-1">
                          <FolderOpen size={11} /> Reveal in File Manager
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>

              {/* Export history */}
              {selectedSermon && (
                <div className="card-panel">
                  <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                    <Clock size={13} className="text-fg-dim" /> Export History
                  </h2>
                  <p className="text-xs text-fg-dim">
                    Export history is tracked per sermon in the backend. Previous exports will appear here after the Rust backend is connected.
                  </p>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}