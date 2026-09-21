'use client';
import React, { useState } from 'react';
import { GitMerge, AlertTriangle, CheckCircle, FileText, Loader2, User, HardDrive,  } from 'lucide-react';

type ResolutionStrategy = 'keep-mine' | 'keep-theirs' | 'merge';

// Track I: this screen is an explicit DEMO/PREVIEW surface. All data below is
// local fixture data and is never sent to any backend. The real inline
// conflict workflow lives in the editor workspace (Track H); this component
// must not invoke native backend operations against fixture IDs.
//
// Vocabulary note: the backend contract names the strategies
// keep-local / use-disk / merge; this demo's keep-mine / keep-theirs / merge
// map onto them one-to-one.

interface DiffLine {
  type: 'added' | 'removed' | 'context';
  lineNo: number;
  text: string;
}

const MINE_BODY = `I. Introduction: The Hunger That Bread Cannot Satisfy

Every person in this room has known physical hunger. We know the gnawing emptiness, the distraction it produces, the singular focus it demands. Jesus draws on this universal experience to address a deeper hunger — the hunger of the soul that no earthly provision can satisfy.

In John 6, the crowd has just witnessed the feeding of five thousand. They are full, satisfied, and enthusiastic. They want to make Jesus king by force (v. 15). But Jesus withdraws. He does not come to be a bread-provider; He comes to be the Bread.

II. The Claim: "I Am the Bread of Life" (vv. 35–40)

The first of the seven "I AM" declarations in John's Gospel is found here. It is not a modest claim. Jesus says: "I am the bread of life; whoever comes to me shall not hunger, and whoever believes in me shall never thirst."

Notice the two verbs: comes and believes. Coming is the act; believing is the posture.`;

const THEIRS_BODY = `I. Introduction: The Hunger That Bread Cannot Satisfy

Every person in this room has known physical hunger. We know the gnawing emptiness, the distraction it produces, the singular focus it demands. Jesus draws on this universal experience to address a deeper hunger — the hunger of the soul.

In John 6, the crowd has just witnessed the feeding of five thousand. They want to make Jesus king by force (v. 15). But Jesus withdraws. He does not come to be a bread-provider; He comes to be the Bread itself.

II. The Claim: "I Am the Bread of Life" (vv. 35–40)

The first of the seven "I AM" declarations in John's Gospel. It is not a modest claim. Jesus says: "I am the bread of life; whoever comes to me shall not hunger, and whoever believes in me shall never thirst."

Notice the two verbs: comes and believes. Coming is the act; believing is the ongoing posture of the soul. The perfect tense implies permanence.`;

function buildDiff(mine: string, theirs: string): DiffLine[] {
  const mineLines = mine.split('\n');
  const theirLines = theirs.split('\n');
  const diff: DiffLine[] = [];
  const maxLen = Math.max(mineLines.length, theirLines.length);
  for (let i = 0; i < maxLen; i++) {
    const m = mineLines[i];
    const t = theirLines[i];
    if (m === t) {
      diff.push({ type: 'context', lineNo: i + 1, text: m ?? '' });
    } else {
      if (m !== undefined) diff.push({ type: 'removed', lineNo: i + 1, text: m });
      if (t !== undefined) diff.push({ type: 'added', lineNo: i + 1, text: t });
    }
  }
  return diff;
}

const DIFF_LINES = buildDiff(MINE_BODY, THEIRS_BODY);

const DEMO_CONFLICT = {
  sermonId: 'demo-conflict-fixture (not a real sermon id)',
  sermonTitle: 'The Bread of Life',
  scripture: 'John 6:35–51',
  mine: {
    version: 14,
    updatedAt: '2026-09-19T21:45:00Z',
    wordCount: 3412,
    source: 'In-memory (editor)',
  },
  theirs: {
    version: 13,
    updatedAt: '2026-09-19T18:22:00Z',
    wordCount: 3388,
    source: 'On-disk (filesystem)',
  },
};

export default function ConflictResolutionContent() {
  const [strategy, setStrategy] = useState<ResolutionStrategy | null>(null);
  const [resolving, setResolving] = useState(false);
  const [resolved, setResolved] = useState(false);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [diffView, setDiffView] = useState<'unified' | 'split'>('unified');

  const handleResolve = async () => {
    if (!strategy) return;
    setResolving(true);
    setResolveError(null);
    try {
      // Demo boundary: NO backend call. This preview simulates resolution
      // locally so it can never mutate a real (or fake) native sermon. The
      // real resolution flow is backend.resolveConflict via the editor
      // workspace (Track H), which maps strategies keep-local / use-disk /
      // merge one-to-one.
      await new Promise((r) => setTimeout(r, 500));
      setResolved(true);
    } finally {
      setResolving(false);
    }
  };

  const formatTime = (iso: string) => {
    const d = new Date(iso);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  };

  if (resolved) {
    return (
      <div className="flex flex-col h-full items-center justify-center gap-4">
        <CheckCircle size={40} className="text-ok-green" />
        <h2 className="text-xl font-600 text-foreground">Conflict Resolved</h2>
        <p className="text-sm text-muted-foreground">
          Strategy applied: <span className="text-primary font-mono-data">{strategy}</span>
        </p>
        <p className="text-xs text-muted-foreground">Demo resolution applied locally — no real sermon was modified. The live workflow lives in the editor workspace.</p>
        <a href="/" className="btn-primary mt-2">
          Return to Editor
        </a>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-3 px-6 py-4 border-b border-border bg-surface-2 flex-shrink-0">
        <GitMerge size={16} className="text-warn-amber" />
        <div>
          <h1 className="text-lg font-600 text-foreground">Save Conflict Detected</h1>
          <p className="text-xs text-muted-foreground">
            The on-disk version differs from your in-memory edits. Choose how to resolve.
          </p>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-screen-2xl mx-auto px-6 py-6 space-y-6">
          {/* Track I: explicit demo/preview boundary */}
          <div className="flex items-start gap-3 px-4 py-3 rounded border border-ref-blue/30 bg-ref-blue/8">
            <FileText size={16} className="text-ref-blue flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-sm font-600 text-ref-blue">Demo — Preview Data</p>
              <p className="text-xs text-fg-dim mt-0.5 leading-relaxed">
                This screen is a design preview built from local fixture data. It does not
                touch any real sermon; applying a resolution here only updates this preview.
                Conflicts in the actual product are handled inline in the editor workspace.
              </p>
            </div>
          </div>

          {/* Alert banner */}
          <div className="flex items-start gap-3 px-4 py-3 rounded border border-warn-amber/30 bg-warn-amber/8">
            <AlertTriangle size={16} className="text-warn-amber flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-sm font-600 text-warn-amber">
                Conflict in &ldquo;{DEMO_CONFLICT.sermonTitle}&rdquo;
              </p>
              <p className="text-xs text-foreground/70 mt-0.5">
                Your in-memory version (v{DEMO_CONFLICT.mine.version}) and the on-disk version (v{DEMO_CONFLICT.theirs.version}) have diverged. 
                Review the differences below and select a resolution strategy before saving.
              </p>
            </div>
          </div>

          {/* Version comparison cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* Mine */}
            <div className="card-panel border-ref-blue/30">
              <div className="flex items-center gap-2 mb-3">
                <User size={14} className="text-ref-blue" />
                <span className="text-xs font-600 text-ref-blue uppercase tracking-wider font-mono-data">
                  Your Version (In-Memory)
                </span>
              </div>
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Version</span>
                  <span className="text-xs font-mono-data text-foreground">v{DEMO_CONFLICT.mine.version}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Modified</span>
                  <span className="text-xs font-mono-data text-foreground">{formatTime(DEMO_CONFLICT.mine.updatedAt)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Word count</span>
                  <span className="text-xs font-mono-data text-foreground">{DEMO_CONFLICT.mine.wordCount.toLocaleString()}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Source</span>
                  <span className="text-2xs font-mono-data text-ref-blue">{DEMO_CONFLICT.mine.source}</span>
                </div>
              </div>
            </div>

            {/* Theirs */}
            <div className="card-panel border-warn-amber/30">
              <div className="flex items-center gap-2 mb-3">
                <HardDrive size={14} className="text-warn-amber" />
                <span className="text-xs font-600 text-warn-amber uppercase tracking-wider font-mono-data">
                  Disk Version (Filesystem)
                </span>
              </div>
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Version</span>
                  <span className="text-xs font-mono-data text-foreground">v{DEMO_CONFLICT.theirs.version}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Modified</span>
                  <span className="text-xs font-mono-data text-foreground">{formatTime(DEMO_CONFLICT.theirs.updatedAt)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Word count</span>
                  <span className="text-xs font-mono-data text-foreground">{DEMO_CONFLICT.theirs.wordCount.toLocaleString()}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-2xs text-muted-foreground">Source</span>
                  <span className="text-2xs font-mono-data text-warn-amber">{DEMO_CONFLICT.theirs.source}</span>
                </div>
              </div>
            </div>
          </div>

          {/* Diff view */}
          <div className="card-panel">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-600 text-foreground flex items-center gap-2">
                <FileText size={14} className="text-primary" />
                Content Diff
              </h2>
              <div className="flex items-center gap-1 bg-surface-3 rounded p-0.5">
                {(['unified', 'split'] as const).map((v) => (
                  <button
                    key={`diff-view-${v}`}
                    onClick={() => setDiffView(v)}
                    className={[
                      'px-3 py-1 text-xs rounded transition-all duration-150',
                      diffView === v ? 'bg-surface-4 text-foreground' : 'text-muted-foreground hover:text-foreground',
                    ].join(' ')}
                  >
                    {v === 'unified' ? 'Unified' : 'Split'}
                  </button>
                ))}
              </div>
            </div>

            {diffView === 'unified' ? (
              <div className="bg-surface-3 rounded overflow-hidden">
                <div className="flex items-center gap-4 px-4 py-2 border-b border-border text-2xs font-mono-data text-muted-foreground">
                  <span className="flex items-center gap-1"><span className="text-alert-red">−</span> Removed (your version)</span>
                  <span className="flex items-center gap-1"><span className="text-ok-green">+</span> Added (disk version)</span>
                  <span className="flex items-center gap-1"><span className="text-muted-foreground">·</span> Unchanged</span>
                </div>
                <div className="max-h-80 overflow-y-auto font-mono-data text-xs leading-relaxed">
                  {DIFF_LINES.map((line, idx) => (
                    <div
                      key={`diff-line-${idx}-${line.lineNo}`}
                      className={[
                        'flex items-start px-4 py-0.5',
                        line.type === 'added' ? 'diff-added' : line.type === 'removed' ? 'diff-removed' : '',
                      ].join(' ')}
                    >
                      <span className="w-6 text-muted-foreground text-right mr-3 flex-shrink-0 select-none">
                        {line.lineNo}
                      </span>
                      <span
                        className={[
                          'flex-shrink-0 w-4 mr-1',
                          line.type === 'added' ? 'text-ok-green' : line.type === 'removed' ? 'text-alert-red' : 'text-muted-foreground',
                        ].join(' ')}
                      >
                        {line.type === 'added' ? '+' : line.type === 'removed' ? '−' : ' '}
                      </span>
                      <span
                        className={
                          line.type === 'added' ?'text-ok-green/90'
                            : line.type === 'removed' ?'text-alert-red/90' :'diff-context'
                        }
                      >
                        {line.text || '\u00A0'}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-2">
                <div className="bg-surface-3 rounded overflow-hidden">
                  <div className="px-3 py-1.5 border-b border-border text-2xs font-mono-data text-ref-blue">
                    Your version (v{DEMO_CONFLICT.mine.version})
                  </div>
                  <pre className="p-3 text-xs font-mono-data text-foreground/80 max-h-72 overflow-y-auto whitespace-pre-wrap leading-relaxed">
                    {MINE_BODY}
                  </pre>
                </div>
                <div className="bg-surface-3 rounded overflow-hidden">
                  <div className="px-3 py-1.5 border-b border-border text-2xs font-mono-data text-warn-amber">
                    Disk version (v{DEMO_CONFLICT.theirs.version})
                  </div>
                  <pre className="p-3 text-xs font-mono-data text-foreground/80 max-h-72 overflow-y-auto whitespace-pre-wrap leading-relaxed">
                    {THEIRS_BODY}
                  </pre>
                </div>
              </div>
            )}
          </div>

          {/* Resolution strategy */}
          <div className="card-panel">
            <h2 className="text-sm font-600 text-foreground mb-4">Choose Resolution Strategy</h2>
            <div className="space-y-3">
              {/* Keep mine */}
              <label
                className={[
                  'flex items-start gap-3 p-4 rounded border cursor-pointer transition-all duration-150',
                  strategy === 'keep-mine' ?'border-ref-blue bg-ref-blue/8' :'border-border hover:border-border/80 hover:bg-surface-3',
                ].join(' ')}
              >
                <input
                  type="radio"
                  name="strategy"
                  value="keep-mine"
                  checked={strategy === 'keep-mine'}
                  onChange={() => setStrategy('keep-mine')}
                  className="hidden"
                />
                <div
                  className={[
                    'w-4 h-4 rounded-full border-2 mt-0.5 flex-shrink-0 flex items-center justify-center',
                    strategy === 'keep-mine' ? 'border-ref-blue' : 'border-border',
                  ].join(' ')}
                >
                  {strategy === 'keep-mine' && (
                    <div className="w-2 h-2 rounded-full bg-ref-blue" />
                  )}
                </div>
                <div>
                  <p className="text-sm font-600 text-foreground flex items-center gap-2">
                    <User size={13} className="text-ref-blue" />
                    Keep My Version
                  </p>
                  <p className="text-xs text-muted-foreground mt-1 leading-relaxed">
                    Discard the on-disk version. Your in-memory edits (v{DEMO_CONFLICT.mine.version}, {DEMO_CONFLICT.mine.wordCount.toLocaleString()} words) become the canonical version. The disk file is overwritten.
                  </p>
                </div>
              </label>

              {/* Keep theirs */}
              <label
                className={[
                  'flex items-start gap-3 p-4 rounded border cursor-pointer transition-all duration-150',
                  strategy === 'keep-theirs' ?'border-warn-amber bg-warn-amber/8' :'border-border hover:border-border/80 hover:bg-surface-3',
                ].join(' ')}
              >
                <input
                  type="radio"
                  name="strategy"
                  value="keep-theirs"
                  checked={strategy === 'keep-theirs'}
                  onChange={() => setStrategy('keep-theirs')}
                  className="hidden"
                />
                <div
                  className={[
                    'w-4 h-4 rounded-full border-2 mt-0.5 flex-shrink-0 flex items-center justify-center',
                    strategy === 'keep-theirs' ? 'border-warn-amber' : 'border-border',
                  ].join(' ')}
                >
                  {strategy === 'keep-theirs' && (
                    <div className="w-2 h-2 rounded-full bg-warn-amber" />
                  )}
                </div>
                <div>
                  <p className="text-sm font-600 text-foreground flex items-center gap-2">
                    <HardDrive size={13} className="text-warn-amber" />
                    Keep Disk Version
                  </p>
                  <p className="text-xs text-muted-foreground mt-1 leading-relaxed">
                    Discard your in-memory edits. The on-disk version (v{DEMO_CONFLICT.theirs.version}, {DEMO_CONFLICT.theirs.wordCount.toLocaleString()} words) is loaded into the editor. Your unsaved changes are lost.
                  </p>
                </div>
              </label>

              {/* Merge */}
              <label
                className={[
                  'flex items-start gap-3 p-4 rounded border cursor-pointer transition-all duration-150',
                  strategy === 'merge' ?'border-primary bg-primary/8' :'border-border hover:border-border/80 hover:bg-surface-3',
                ].join(' ')}
              >
                <input
                  type="radio"
                  name="strategy"
                  value="merge"
                  checked={strategy === 'merge'}
                  onChange={() => setStrategy('merge')}
                  className="hidden"
                />
                <div
                  className={[
                    'w-4 h-4 rounded-full border-2 mt-0.5 flex-shrink-0 flex items-center justify-center',
                    strategy === 'merge' ? 'border-primary' : 'border-border',
                  ].join(' ')}
                >
                  {strategy === 'merge' && (
                    <div className="w-2 h-2 rounded-full bg-primary" />
                  )}
                </div>
                <div>
                  <p className="text-sm font-600 text-foreground flex items-center gap-2">
                    <GitMerge size={13} className="text-primary" />
                    Merge Both Versions
                  </p>
                  <p className="text-xs text-muted-foreground mt-1 leading-relaxed">
                    Attempt a three-way merge. The native backend will combine non-conflicting sections. Conflicting sections will be marked with conflict markers for manual resolution in the editor.
                  </p>
                </div>
              </label>
            </div>
          </div>

          {/* Action bar */}
          <div className="flex items-center gap-3 pb-6">
            <button
              onClick={handleResolve}
              disabled={!strategy || resolving}
              className="btn-primary"
            >
              {resolving ? (
                <>
                  <Loader2 size={13} className="animate-spin-slow" />
                  <span>Resolving…</span>
                </>
              ) : (
                <>
                  <CheckCircle size={13} />
                  <span>
                    Apply Resolution
                    {strategy && (
                      <span className="ml-1 opacity-70">
                        — {strategy === 'keep-mine' ? 'Keep Mine' : strategy === 'keep-theirs' ? 'Keep Disk' : 'Merge'}
                      </span>
                    )}
                  </span>
                </>
              )}
            </button>
            {!strategy && (
              <p className="text-xs text-muted-foreground">Select a strategy above to continue</p>
            )}
            {resolveError && (
              <p className="text-xs text-alert-red">{resolveError}</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}