'use client';
import React, { useState, useEffect, useCallback } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { Settings, Database, FileOutput, Type, RefreshCw, RotateCcw, CheckCircle, XCircle, Loader2, ChevronRight, Info, Code2, Keyboard, FolderOpen, BookOpen, Wrench,  } from 'lucide-react';
import type { IndexOperationResult, IndexStatus, AppSettings } from '@/lib/backend/types';
import Icon from '@/components/ui/AppIcon';


type SettingsSection = 'library' | 'index' | 'appearance' | 'editor' | 'export' | 'shortcuts' | 'developer';

const SECTIONS: { id: SettingsSection; label: string; icon: React.ElementType }[] = [
  { id: 'library', label: 'Sermon Library', icon: BookOpen },
  { id: 'index', label: 'Index Status', icon: Database },
  { id: 'appearance', label: 'Appearance', icon: Settings },
  { id: 'editor', label: 'Editor', icon: Type },
  { id: 'export', label: 'Export Defaults', icon: FileOutput },
  { id: 'shortcuts', label: 'Keyboard Shortcuts', icon: Keyboard },
  { id: 'developer', label: 'Developer Tools', icon: Code2 },
];

function Toggle({ active, onToggle, label }: { active: boolean; onToggle: () => void; label: string }) {
  return (
    <button role="switch" aria-checked={active} aria-label={label} onClick={onToggle} className={`toggle-track ${active ? 'active' : ''}`}>
      <div className="toggle-thumb" />
    </button>
  );
}

function SettingRow({ label, description, children }: { label: string; description?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4 py-3 border-b border-border/50 last:border-0">
      <div className="flex-1 min-w-0">
        <p className="text-sm font-500 text-fg">{label}</p>
        {description && <p className="text-xs text-fg-dim mt-0.5 leading-relaxed">{description}</p>}
      </div>
      <div className="flex-shrink-0">{children}</div>
    </div>
  );
}

export default function SettingsContent() {
  const backend = useBackend();
  const [activeSection, setActiveSection] = useState<SettingsSection>('library');

  // Settings state
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [settingsLoading, setSettingsLoading] = useState(true);
  const [settingsSaved, setSettingsSaved] = useState(false);

  // Librarian
  const [librarianSaving, setLibrarianSaving] = useState(false);

  // Index
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);
  const [indexStatusLoading, setIndexStatusLoading] = useState(false);
  const [indexOp, setIndexOp] = useState<'idle' | 'syncing' | 'rebuilding' | 'rescanning' | 'repairing'>('idle');
  const [indexResult, setIndexResult] = useState<IndexOperationResult | null>(null);
  const [indexError, setIndexError] = useState<string | null>(null);

  useEffect(() => {
    backend.loadSettings().then((s) => {
      setSettings(s);
      setSettingsLoading(false);
    });
  }, [backend]);

  const loadIndexStatus = useCallback(async () => {
    setIndexStatusLoading(true);
    try {
      const status = await backend.getIndexStatus();
      setIndexStatus(status);
    } finally {
      setIndexStatusLoading(false);
    }
  }, [backend]);

  useEffect(() => {
    if (activeSection === 'index') loadIndexStatus();
  }, [activeSection, loadIndexStatus]);

  const saveSettings = async () => {
    if (!settings) return;
    await backend.saveSettings(settings);
    setSettingsSaved(true);
    setTimeout(() => setSettingsSaved(false), 2500);
  };

  const updateSettings = (patch: Partial<AppSettings>) => {
    setSettings((s) => s ? { ...s, ...patch } : s);
  };

  const handleLibrarianToggle = async () => {
    if (!settings) return;
    setLibrarianSaving(true);
    try {
      const newVal = !settings.librarianEnabled;
      await backend.setLibrarianEnabled(newVal);
      updateSettings({ librarianEnabled: newVal });
    } finally {
      setLibrarianSaving(false);
    }
  };

  const runIndexOp = async (op: 'sync' | 'rebuild' | 'rescan' | 'repair') => {
    const opMap = { sync: 'syncing', rebuild: 'rebuilding', rescan: 'rescanning', repair: 'repairing' } as const;
    setIndexOp(opMap[op]);
    setIndexResult(null);
    setIndexError(null);
    try {
      let result: IndexOperationResult;
      if (op === 'sync') result = await backend.syncIndex();
      else if (op === 'rebuild') result = await backend.rebuildIndex();
      else if (op === 'rescan') result = await backend.rescanLibrary();
      else result = await backend.repairIndex();
      setIndexResult(result);
      await loadIndexStatus();
    } catch (e: unknown) {
      setIndexError(e instanceof Error ? e.message : 'Operation failed');
    } finally {
      setIndexOp('idle');
    }
  };

  const cancelOp = async () => {
    await backend.cancelIndexOperation();
    setIndexOp('idle');
  };

  const DEFAULT_SHORTCUTS: Record<string, string> = {
    'Command Palette': '⌘P',
    'Focus Archive Search': '⌘K',
    'New Sermon': '⌘N',
    'Save': '⌘S',
    'Save As': '⌘⇧S',
    'Export': '⌘⇧E',
    'Run Linter': '⌘⇧L',
    'Search Archive': '⌘⇧A',
    'Toggle Archive Rail': '⌘[',
    'Toggle Study Rail': '⌘]',
    'Settings': '⌘,',
    'Undo': '⌘Z',
    'Redo': '⌘⇧Z',
    'Bold': '⌘B',
    'Italic': '⌘I',
    'Find': '⌘F',
  };

  if (settingsLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 size={20} className="animate-spin-slow text-fg-dim" />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-6 py-4 border-b border-border bg-panel flex-shrink-0">
        <Settings size={15} className="text-accent" />
        <h1 className="text-lg font-600 text-fg">Settings</h1>
        <ChevronRight size={13} className="text-fg-dim" />
        <span className="text-sm text-fg-dim">{SECTIONS.find((s) => s.id === activeSection)?.label}</span>
        {settingsSaved && (
          <span className="ml-auto text-xs text-ok-green flex items-center gap-1">
            <CheckCircle size={12} /> Saved
          </span>
        )}
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Settings nav */}
        <div className="w-52 flex-shrink-0 border-r border-border bg-panel py-3 px-2 flex flex-col gap-0.5">
          {SECTIONS.map((sec) => {
            const SecIcon = sec.icon;
            const isActive = activeSection === sec.id;
            return (
              <button
                key={`settings-nav-${sec.id}`}
                onClick={() => setActiveSection(sec.id)}
                className={[
                  'flex items-center gap-2.5 px-3 py-2 rounded text-left transition-all duration-100 w-full',
                  isActive ? 'bg-accent/10 text-accent' : 'text-fg-dim hover:bg-elevated hover:text-fg',
                ].join(' ')}
              >
                <SecIcon size={13} className="flex-shrink-0" />
                <span className="text-sm font-500">{sec.label}</span>
              </button>
            );
          })}
        </div>

        {/* Settings content */}
        <div className="flex-1 overflow-y-auto">
          <div className="max-w-2xl mx-auto px-8 py-8 space-y-6">

            {/* ── Sermon Library ── */}
            {activeSection === 'library' && settings && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Sermon Library</h2>
                  <p className="text-xs text-fg-dim">Configure the location and behavior of your sermon library.</p>
                </div>
                <div className="card-panel space-y-0">
                  <SettingRow label="Library Path" description="Absolute path to the root directory containing your sermon Markdown files.">
                    <input
                      type="text"
                      value={settings.libraryPath}
                      onChange={(e) => updateSettings({ libraryPath: e.target.value })}
                      className="input-field text-xs font-mono-data w-64"
                    />
                  </SettingRow>
                  <SettingRow label="Default Translation" description="Scripture translation used when no translation directive is set.">
                    <select value={settings.defaultTranslation} onChange={(e) => updateSettings({ defaultTranslation: e.target.value })} className="input-field text-xs w-32">
                      {['ESV', 'KJV', 'NKJV', 'NIV', 'NASB', 'CSB', 'NLT'].map((t) => (
                        <option key={t} value={t}>{t}</option>
                      ))}
                    </select>
                  </SettingRow>
                  <SettingRow label="Librarian" description="Background service that tracks illustration usage and surfaces fatigue warnings.">
                    <div className="flex items-center gap-2">
                      {librarianSaving && <Loader2 size={11} className="animate-spin-slow text-fg-dim" />}
                      <Toggle active={settings.librarianEnabled} onToggle={handleLibrarianToggle} label="Toggle Librarian" />
                    </div>
                  </SettingRow>
                </div>
                <button onClick={saveSettings} className="btn-primary">
                  <CheckCircle size={12} /> Save Library Settings
                </button>
              </div>
            )}

            {/* ── Index Status ── */}
            {activeSection === 'index' && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Index Status</h2>
                  <p className="text-xs text-fg-dim">The full-text search index is maintained by the native Rust backend. No SQLite logic runs in TypeScript.</p>
                </div>

                {/* Status panel */}
                <div className="card-panel">
                  <div className="flex items-center justify-between mb-3">
                    <h3 className="text-xs font-mono-data uppercase tracking-wider text-fg-dim">Current Index State</h3>
                    <button onClick={loadIndexStatus} disabled={indexStatusLoading} className="btn-ghost text-xs py-0.5 px-1.5">
                      {indexStatusLoading ? <Loader2 size={10} className="animate-spin-slow" /> : <RefreshCw size={10} />}
                    </button>
                  </div>
                  {indexStatus ? (
                    <div className="grid grid-cols-2 gap-3">
                      {[
                        { label: 'Indexed Files', value: indexStatus.indexedFileCount.toString(), color: 'text-accent' },
                        { label: 'Index Version', value: indexStatus.indexVersion, color: 'text-fg' },
                        { label: 'Status', value: indexStatus.status, color: indexStatus.status === 'idle' ? 'text-ok-green' : 'text-warn-amber' },
                        { label: 'Pending Files', value: (indexStatus.pendingFiles ?? 0).toString(), color: 'text-fg-dim' },
                      ].map((stat) => (
                        <div key={`idx-stat-${stat.label}`} className="bg-elevated rounded px-3 py-2 border border-border">
                          <p className="text-2xs text-fg-dim mb-0.5">{stat.label}</p>
                          <p className={`text-sm font-600 font-mono-data ${stat.color}`}>{stat.value}</p>
                        </div>
                      ))}
                      {indexStatus.lastReconciliationTime && (
                        <div className="col-span-2 bg-elevated rounded px-3 py-2 border border-border">
                          <p className="text-2xs text-fg-dim mb-0.5">Last Reconciliation</p>
                          <p className="text-xs font-mono-data text-fg">{indexStatus.lastReconciliationTime.slice(0, 19).replace('T', ' ')}</p>
                        </div>
                      )}
                      {indexStatus.lastFullScanTime && (
                        <div className="col-span-2 bg-elevated rounded px-3 py-2 border border-border">
                          <p className="text-2xs text-fg-dim mb-0.5">Last Full Scan</p>
                          <p className="text-xs font-mono-data text-fg">{indexStatus.lastFullScanTime.slice(0, 19).replace('T', ' ')}</p>
                        </div>
                      )}
                    </div>
                  ) : (
                    <div className="flex items-center gap-2 py-4 text-fg-dim">
                      <Loader2 size={13} className="animate-spin-slow" />
                      <span className="text-xs">Loading index status…</span>
                    </div>
                  )}
                </div>

                {/* Operations */}
                <div className="card-panel space-y-4">
                  <h3 className="text-xs font-mono-data uppercase tracking-wider text-fg-dim">Index Operations</h3>
                  {[
                    { op: 'sync' as const, label: 'Sync Index', desc: 'Update index with changed files since last sync. Fast.', icon: RefreshCw, color: 'text-accent' },
                    { op: 'rescan' as const, label: 'Rescan Library', desc: 'Scan library directory for new or moved files.', icon: FolderOpen, color: 'text-accent-2' },
                    { op: 'repair' as const, label: 'Repair / Reconcile', desc: 'Fix inconsistencies between index and filesystem.', icon: Wrench, color: 'text-warn-amber' },
                    { op: 'rebuild' as const, label: 'Rebuild Index', desc: 'Drop and recreate entire index from scratch. Slower.', icon: RotateCcw, color: 'text-alert-red' },
                  ].map(({ op, label, desc, icon: OpIcon, color }) => {
                    const Icon = OpIcon;
                    return (
                      <div key={`op-${op}`} className="flex items-start justify-between gap-4">
                        <div>
                          <p className="text-sm font-500 text-fg flex items-center gap-1.5">
                            <Icon size={12} className={color} /> {label}
                          </p>
                          <p className="text-xs text-fg-dim mt-0.5">{desc}</p>
                        </div>
                        <button
                          onClick={() => runIndexOp(op)}
                          disabled={indexOp !== 'idle'}
                          className="btn-ghost text-xs flex-shrink-0"
                        >
                          {indexOp !== 'idle' && indexOp === `${op}ing` ? (
                            <><Loader2 size={11} className="animate-spin-slow" /> Running…</>
                          ) : (
                            label.split(' ')[0]
                          )}
                        </button>
                      </div>
                    );
                  })}
                </div>

                {/* Progress */}
                {indexOp !== 'idle' && (
                  <div className="card-panel border-accent/30 bg-accent/5">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <Loader2 size={13} className="animate-spin-slow text-accent" />
                        <p className="text-sm font-500 text-fg capitalize">{indexOp}…</p>
                      </div>
                      <button onClick={cancelOp} className="btn-ghost text-xs text-alert-red">Cancel</button>
                    </div>
                    <div className="h-1 bg-elevated rounded-full overflow-hidden">
                      <div className="h-full bg-accent rounded-full w-1/2 transition-all duration-700 animate-pulse" />
                    </div>
                  </div>
                )}

                {/* Result */}
                {indexResult && indexOp === 'idle' && (
                  <div className={`card-panel ${indexResult.success ? 'border-ok-green/30 bg-ok-green/5' : 'border-alert-red/30 bg-alert-red/5'}`}>
                    <div className="flex items-center gap-2 mb-1">
                      {indexResult.success ? <CheckCircle size={13} className="text-ok-green" /> : <XCircle size={13} className="text-alert-red" />}
                      <p className={`text-sm font-600 ${indexResult.success ? 'text-ok-green' : 'text-alert-red'}`}>
                        {indexResult.success ? 'Complete' : 'Failed'}
                      </p>
                    </div>
                    <p className="text-xs text-fg/80">{indexResult.message}</p>
                    {indexResult.durationMs && (
                      <p className="text-2xs font-mono-data text-fg-dim mt-1">
                        {indexResult.durationMs}ms · {indexResult.documentsIndexed} documents
                      </p>
                    )}
                  </div>
                )}

                {indexError && (
                  <div className="card-panel border-alert-red/30 bg-alert-red/5">
                    <div className="flex items-center gap-2">
                      <XCircle size={13} className="text-alert-red" />
                      <p className="text-xs text-alert-red">{indexError}</p>
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* ── Appearance ── */}
            {activeSection === 'appearance' && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Appearance</h2>
                  <p className="text-xs text-fg-dim">Visual system settings. The color scheme is locked to the Sermon Studio design system.</p>
                </div>
                <div className="card-panel">
                  <div className="flex items-start gap-3 p-3 rounded bg-accent/5 border border-accent/20 mb-4">
                    <Info size={13} className="text-accent flex-shrink-0 mt-0.5" />
                    <p className="text-xs text-fg-dim leading-relaxed">
                      The visual system uses locked CSS custom properties (<code className="font-mono-data text-accent">--bg</code>, <code className="font-mono-data text-accent">--panel</code>, <code className="font-mono-data text-accent">--accent</code>, etc.).
                      The dark research environment is intentional and not configurable in this version.
                    </p>
                  </div>
                  <div className="grid grid-cols-3 gap-2">
                    {[
                      { name: '--bg', value: '#0e1116', label: 'Background' },
                      { name: '--panel', value: '#151a21', label: 'Panel' },
                      { name: '--elevated', value: '#1b222c', label: 'Elevated' },
                      { name: '--accent', value: '#6ea8fe', label: 'Accent' },
                      { name: '--accent-2', value: '#7ee0b8', label: 'Accent 2' },
                      { name: '--warn', value: '#f0b429', label: 'Warning' },
                    ].map((token) => (
                      <div key={token.name} className="flex items-center gap-2 p-2 rounded border border-border">
                        <div className="w-5 h-5 rounded flex-shrink-0 border border-border/50" style={{ backgroundColor: token.value }} />
                        <div>
                          <p className="text-2xs font-mono-data text-fg">{token.name}</p>
                          <p className="text-2xs font-mono-data text-fg-dim">{token.value}</p>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* ── Editor ── */}
            {activeSection === 'editor' && settings && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Editor</h2>
                  <p className="text-xs text-fg-dim">Customize the sermon writing environment.</p>
                </div>
                <div className="card-panel space-y-0">
                  <SettingRow label="Editor Font Size" description="Font size for the sermon body editor (px).">
                    <div className="flex items-center gap-2">
                      <input type="range" min={12} max={22} value={settings.editorFontSize} onChange={(e) => updateSettings({ editorFontSize: parseInt(e.target.value) })} className="w-24 accent-accent" />
                      <span className="text-xs font-mono-data text-fg w-8">{settings.editorFontSize}px</span>
                    </div>
                  </SettingRow>
                  <SettingRow label="Autosave Interval" description="Seconds after last keystroke before autosaving.">
                    <select value={settings.autosaveIntervalSeconds} onChange={(e) => updateSettings({ autosaveIntervalSeconds: parseInt(e.target.value) })} className="input-field text-xs w-32">
                      {[1, 3, 5, 10, 30].map((v) => <option key={v} value={v}>{v}s</option>)}
                    </select>
                  </SettingRow>
                  <SettingRow label="Spellcheck" description="Browser-native spellcheck in the editor.">
                    <Toggle active={settings.spellcheck} onToggle={() => updateSettings({ spellcheck: !settings.spellcheck })} label="Toggle spellcheck" />
                  </SettingRow>
                  <SettingRow label="Focus Mode" description="Hide archive and study rails for distraction-free writing.">
                    <Toggle active={settings.focusMode} onToggle={() => updateSettings({ focusMode: !settings.focusMode })} label="Toggle focus mode" />
                  </SettingRow>
                </div>
                <button onClick={saveSettings} className="btn-primary">
                  <CheckCircle size={12} /> Save Editor Settings
                </button>
              </div>
            )}

            {/* ── Export Defaults ── */}
            {activeSection === 'export' && settings && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Export Defaults</h2>
                  <p className="text-xs text-fg-dim">Default options applied to new export jobs. Can be overridden per-export.</p>
                </div>
                <div className="card-panel space-y-0">
                  <SettingRow label="Default Page Size">
                    <select value={settings.exportDefaults.pageSize || 'letter'} onChange={(e) => updateSettings({ exportDefaults: { ...settings.exportDefaults, pageSize: e.target.value as 'letter' | 'a4' } })} className="input-field text-xs w-32">
                      <option value="letter">US Letter</option>
                      <option value="a4">A4</option>
                      <option value="a5">A5</option>
                    </select>
                  </SettingRow>
                  <SettingRow label="Default Font Size (pt)">
                    <input type="number" min={8} max={16} value={settings.exportDefaults.fontSize || 11} onChange={(e) => updateSettings({ exportDefaults: { ...settings.exportDefaults, fontSize: parseInt(e.target.value) } })} className="input-field text-xs w-20" />
                  </SettingRow>
                  <SettingRow label="Include Title Page">
                    <Toggle active={settings.exportDefaults.includeTitlePage ?? true} onToggle={() => updateSettings({ exportDefaults: { ...settings.exportDefaults, includeTitlePage: !settings.exportDefaults.includeTitlePage } })} label="Toggle title page" />
                  </SettingRow>
                  <SettingRow label="Include Scripture References">
                    <Toggle active={settings.exportDefaults.includeScriptureReferences ?? true} onToggle={() => updateSettings({ exportDefaults: { ...settings.exportDefaults, includeScriptureReferences: !settings.exportDefaults.includeScriptureReferences } })} label="Toggle scripture refs" />
                  </SettingRow>
                  <SettingRow label="Include Notes">
                    <Toggle active={settings.exportDefaults.includeNotes ?? true} onToggle={() => updateSettings({ exportDefaults: { ...settings.exportDefaults, includeNotes: !settings.exportDefaults.includeNotes } })} label="Toggle notes" />
                  </SettingRow>
                  <SettingRow label="Include Illustrations">
                    <Toggle active={settings.exportDefaults.includeIllustrations ?? true} onToggle={() => updateSettings({ exportDefaults: { ...settings.exportDefaults, includeIllustrations: !settings.exportDefaults.includeIllustrations } })} label="Toggle illustrations" />
                  </SettingRow>
                </div>
                <button onClick={saveSettings} className="btn-primary">
                  <CheckCircle size={12} /> Save Export Defaults
                </button>
              </div>
            )}

            {/* ── Keyboard Shortcuts ── */}
            {activeSection === 'shortcuts' && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Keyboard Shortcuts</h2>
                  <p className="text-xs text-fg-dim">All keyboard shortcuts for Sermon Studio. Customization coming in a future release.</p>
                </div>
                <div className="card-panel">
                  <div className="space-y-0">
                    {Object.entries(DEFAULT_SHORTCUTS).map(([action, shortcut]) => (
                      <div key={`shortcut-${action}`} className="flex items-center justify-between py-2.5 border-b border-border/40 last:border-0">
                        <span className="text-sm text-fg">{action}</span>
                        <kbd className="text-xs font-mono-data text-fg-dim bg-elevated px-2 py-1 rounded border border-border">{shortcut}</kbd>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {/* ── Developer Tools ── */}
            {activeSection === 'developer' && settings && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-600 text-fg mb-1">Developer Tools</h2>
                  <p className="text-xs text-fg-dim">Tools for testing and debugging the frontend system.</p>
                </div>

                <div className="card-panel space-y-0">
                  <SettingRow label="Developer Mode" description="Enable developer tools and diagnostic overlays.">
                    <Toggle active={settings.developerMode} onToggle={() => updateSettings({ developerMode: !settings.developerMode })} label="Toggle developer mode" />
                  </SettingRow>
                </div>

                <div className="card-panel">
                  <h3 className="text-xs font-mono-data uppercase tracking-wider text-fg-dim mb-3">Developer Screens</h3>
                  <div className="space-y-2">
                    <a href="/codec-test" className="flex items-center justify-between p-3 rounded border border-border hover:border-accent/40 hover:bg-accent/5 transition-all group">
                      <div className="flex items-center gap-2.5">
                        <Code2 size={14} className="text-accent" />
                        <div>
                          <p className="text-sm font-500 text-fg">Directive Transport Codec Test</p>
                          <p className="text-xs text-fg-dim">Verify unknown directives survive round-trip</p>
                        </div>
                      </div>
                      <ChevronRight size={13} className="text-fg-dim group-hover:text-accent transition-colors" />
                    </a>
                  </div>
                </div>

                <div className="card-panel">
                  <h3 className="text-xs font-mono-data uppercase tracking-wider text-fg-dim mb-3">Architecture Guard</h3>
                  <div className="space-y-2 text-xs text-fg-dim">
                    <div className="flex items-center gap-2">
                      <CheckCircle size={11} className="text-ok-green flex-shrink-0" />
                      <span>React components import only <code className="font-mono-data text-accent">useBackend()</code> and backend types</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <CheckCircle size={11} className="text-ok-green flex-shrink-0" />
                      <span>No component imports <code className="font-mono-data text-accent">MockSermonBackend</code> or <code className="font-mono-data text-accent">TauriSermonBackend</code></span>
                    </div>
                    <div className="flex items-center gap-2">
                      <CheckCircle size={11} className="text-ok-green flex-shrink-0" />
                      <span>Backend selection happens only at <code className="font-mono-data text-accent">BackendContext</code> boundary</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <CheckCircle size={11} className="text-ok-green flex-shrink-0" />
                      <span>Tauri IPC imports are dynamically isolated inside <code className="font-mono-data text-accent">TauriSermonBackend</code></span>
                    </div>
                    <div className="flex items-center gap-2">
                      <CheckCircle size={11} className="text-ok-green flex-shrink-0" />
                      <span>No Rust-owned logic (parsing, linting, SQLite, Typst, PDF) in TypeScript</span>
                    </div>
                  </div>
                </div>

                <button onClick={saveSettings} className="btn-primary">
                  <CheckCircle size={12} /> Save Developer Settings
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}