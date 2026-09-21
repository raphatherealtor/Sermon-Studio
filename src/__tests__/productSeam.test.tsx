/**
 * Track I — product-seam cleanup tests.
 *
 * Proves the localized seam fixes:
 *  1. mock lint is fixture-driven, not rule computation
 *  2. mock reference output can represent all three resolution states
 *  3. command-palette save clears/updates save state through shared workflow
 *  4. unsupported Reveal does not produce an unhandled rejection
 *  5. unsupported export options are not presented as functional
 *  6. export flow does not assume sermon-001
 *  7. conflict demo cannot accidentally mutate a fake native sermon
 */

import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';

import { MockSermonBackend } from '@/lib/backend/MockSermonBackend';
import { BackendProvider } from '@/lib/backend/BackendContext';
import type { SermonBackend } from '@/lib/backend/SermonBackend';
import type {
  SermonDocument,
  ExportRequest,
  ExportResult,
  ExportSnapshot,
  SaveResult,
} from '@/lib/backend/types';
import { useEditorStore } from '@/lib/store/editorStore';
import CommandPalette from '@/components/CommandPalette';
import ExportScreenContent from '@/app/export-screen/components/ExportScreenContent';
import ConflictResolutionContent from '@/app/conflict-resolution-screen/components/ConflictResolutionContent';

jest.mock('next/navigation', () => ({
  useRouter: () => ({ push: jest.fn(), replace: jest.fn() }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeDoc(overrides: Partial<SermonDocument> = {}): SermonDocument {
  return {
    id: 'doc-a',
    title: 'Doc A',
    scripture: 'John 3:16',
    series: null,
    status: 'draft',
    body: '<p>Some completely unrelated prose about shepherds.</p>',
    outline: [],
    tags: [],
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    preachedOn: null,
    version: 1,
    directives: [],
    ...overrides,
  };
}

const ALLOWED_RULE_IDS = new Set([
  'missing-big-idea',
  'orphaned-movement',
  'missing-application',
  'illustration-fatigue-90d',
]);

function resetStore(doc: SermonDocument | null, isDirty = false) {
  useEditorStore.setState({
    activeSermonId: doc?.id ?? null,
    activeDocument: doc,
    isDirty,
    isSaving: false,
    lastSaved: null,
    saveError: null,
    lintFindings: [],
    isLinting: false,
    conflictInfo: null,
    showMergeDrawer: false,
    wordCount: 5,
    estimatedMinutes: 1,
  });
}

// ── 1. Mock lint is fixture-driven, not rule computation ─────────────────────

describe('MockSermonBackend.lintSermon (fixture-driven)', () => {
  it('returns only canonical Rust rule IDs regardless of document content', async () => {
    const backend = new MockSermonBackend();
    const findings = await backend.lintSermon(makeDoc());
    expect(findings.length).toBeGreaterThan(0);
    for (const f of findings) {
      expect(ALLOWED_RULE_IDS.has(f.ruleId)).toBe(true);
      expect(f.code).toBe(f.ruleId);
    }
  });

  it('never computes from body/outline content — different docs with the same id get identical fixtures', async () => {
    const backend = new MockSermonBackend();
    const long = await backend.lintSermon(
      makeDoc({ body: `<p>${'word '.repeat(5000)}</p>`, outline: [{ id: 'x', level: 1, text: 'X', children: [] }] }),
    );
    const short = await backend.lintSermon(makeDoc({ body: '<p>tiny</p>', outline: [] }));
    expect(long).toEqual(short);
  });

  it('contains no legacy mock-invented rule IDs', async () => {
    const backend = new MockSermonBackend();
    const all = await backend.lintSermon(makeDoc());
    const ruleIds = all.map((f) => f.ruleId);
    for (const legacy of [
      'MISSING_SCRIPTURE', 'LOW_WORD_COUNT', 'NO_SCRIPTURE_QUOTE', 'ILLUSTRATION_FATIGUE', 'NO_DIRECTIVES',
    ]) {
      expect(ruleIds).not.toContain(legacy);
    }
  });

  it('is deterministic across repeated calls', async () => {
    const backend = new MockSermonBackend();
    const a = await backend.lintSermon(makeDoc());
    const b = await backend.lintSermon(makeDoc());
    expect(a).toEqual(b);
  });
});

// ── 2. Mock references represent all three resolution states ─────────────────

describe('MockSermonBackend.parseReferences (fixture-driven)', () => {
  it('returns definite, ambiguous, and invalid fixtures', async () => {
    const backend = new MockSermonBackend();
    const refs = await backend.parseReferences('irrelevant body text — fixtures are static');
    const states = new Set(refs.map((r) => r.resolution));
    expect(states.has('definite')).toBe(true);
    expect(states.has('ambiguous')).toBe(true);
    expect(states.has('invalid')).toBe(true);
    const definite = refs.find((r) => r.resolution === 'definite');
    expect(definite?.osisId).toBeTruthy();
    const ambiguous = refs.find((r) => r.resolution === 'ambiguous');
    expect(ambiguous?.reason).toBeTruthy();
    const invalid = refs.find((r) => r.resolution === 'invalid');
    expect(invalid?.reason).toBeTruthy();
  });

  it('does not parse the input — different texts yield identical fixtures', async () => {
    const backend = new MockSermonBackend();
    const a = await backend.parseReferences('John wrote about Romans 9:1 and Zechariah 4:6');
    const b = await backend.parseReferences('completely different text with no references');
    expect(a).toEqual(b);
  });
});

// ── 3. Command-palette save goes through the shared workflow ─────────────────

describe('CommandPalette save (shared workflow)', () => {
  it('calls backend.saveSermon AND clears the unsaved state via store transitions', async () => {
    const saveSermon = jest.fn<Promise<SaveResult>, [SermonDocument]>(async () => ({
      success: true,
      savedAt: '2026-09-20T12:00:00Z',
      version: 2,
    }));
    const backend = { saveSermon } as unknown as SermonBackend;
    resetStore(makeDoc(), true); // unsaved work on screen

    render(
      <BackendProvider backend={backend}>
        <CommandPalette open onClose={jest.fn()} />
      </BackendProvider>,
    );

    fireEvent.click(screen.getByText('Save'));

    await waitFor(() => expect(saveSermon).toHaveBeenCalledTimes(1));
    const state = useEditorStore.getState();
    expect(state.lastSaved).toBe('2026-09-20T12:00:00Z');
    expect(state.isDirty).toBe(false); // "Unsaved" cleared
    expect(state.isSaving).toBe(false);
    expect(state.saveError).toBeNull();
  });
});

// ── 4–6. Export screen: reveal, honest controls, no sermon-001 ───────────────

const STUB_SERMONS = [
  {
    id: 'alpha-first', title: 'Alpha First', scripture: 'John 3:16', series: null,
    status: 'preached' as const, wordCount: 100, createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z', preachedOn: '2026-01-01', tags: [],
  },
  {
    id: 'beta-second', title: 'Beta Second', scripture: 'John 3:16', series: null,
    status: 'draft' as const, wordCount: 10, createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z', preachedOn: null, tags: [],
  },
];

const STUB_SNAPSHOT: ExportSnapshot = {
  snapshotId: 'snap-1', sermonId: 'alpha-first', sermonTitle: 'Alpha First',
  createdAt: '2026-09-20T00:00:00Z', wordCount: 100, status: 'ready',
};

const STUB_EXPORT_RESULT: ExportResult = {
  success: true,
  outputPath: '/exports/alpha-first.pdf',
  message: 'Exported successfully as pulpit_manuscript.',
  format: 'pulpit_manuscript',
  exportedAt: '2026-09-20T00:00:01Z',
  fileSizeBytes: 12345,
};

function makeExportBackend(overrides: Partial<Record<string, unknown>> = {}) {
  const executeExportJob = jest.fn<Promise<ExportResult>, [ExportRequest]>(async () => STUB_EXPORT_RESULT);
  const backend = {
    listSermons: jest.fn(async () => STUB_SERMONS),
    createExportSnapshot: jest.fn(async () => STUB_SNAPSHOT),
    executeExportJob,
    revealExportedFile: jest.fn(async () => undefined),
    ...overrides,
  } as unknown as SermonBackend;
  return { backend, executeExportJob };
}

async function exportThroughUI(backend: SermonBackend) {
  render(
    <BackendProvider backend={backend}>
      <ExportScreenContent />
    </BackendProvider>,
  );
  // Wait for the sermon list (unique list-only title), then snapshot + export.
  await screen.findByText('Beta Second');
  fireEvent.click(screen.getByText('Create Snapshot'));
  await screen.findByText('Snapshot ID'); // snapshot ready → export enabled
  fireEvent.click(screen.getByText(/Export as Pulpit Manuscript/));
  await screen.findByText('Export successful');
}

describe('Export screen (honest controls)', () => {
  it('does not present unsupported options as functional', async () => {
    const { backend } = makeExportBackend();
    render(
      <BackendProvider backend={backend}>
        <ExportScreenContent />
      </BackendProvider>,
    );
    await screen.findByText('Beta Second');
    // Removed silent no-ops…
    expect(screen.queryByText('Page Size')).toBeNull();
    expect(screen.queryByText('Font Size (pt)')).toBeNull();
    expect(screen.queryByText('Typography')).toBeNull();
    expect(screen.queryByText('Title Page')).toBeNull();
    expect(screen.queryByText('Illustrations')).toBeNull();
    // …with an honest explanation of what the backend actually does.
    expect(screen.getByText(/V1 layouts are fixed by the canonical Typst templates/)).toBeTruthy();
    // And the one honored option remains, for the format that supports it.
    expect(screen.getByText(/Include private study notes/)).toBeTruthy();
  });

  it('sends only backend-honored options and hides the notes toggle for the bulletin', async () => {
    const { backend, executeExportJob } = makeExportBackend();
    await exportThroughUI(backend);
    expect(executeExportJob).toHaveBeenCalledTimes(1);
    const request = executeExportJob.mock.calls[0][0];
    expect(Object.keys(request.options).sort()).toEqual(['includeNotes', 'outputFilename', 'outputPath']);

    // Switch to bulletin: notes toggle disappears (bulletin is structurally public).
    fireEvent.click(screen.getByText('Church Bulletin'));
    expect(screen.queryByText(/Include private study notes/)).toBeNull();
  });

  it('auto-selects from listSermons() and never assumes sermon-001', async () => {
    const { backend } = makeExportBackend();
    render(
      <BackendProvider backend={backend}>
        <ExportScreenContent />
      </BackendProvider>,
    );
    // Wait for the list, then verify the auto-selection came from listSermons.
    await screen.findByText('Beta Second');
    const checkedInput = document.querySelector(
      'input[name="sermonId"]:checked'
    ) as HTMLInputElement | null;
    expect(checkedInput).not.toBeNull();
    expect(checkedInput!.value).toBe('alpha-first');
    // No element in the screen is keyed to the magic id or its document.
    expect(screen.queryByText('The Bread of Life')).toBeNull();
    expect(document.body.innerHTML.includes('sermon-001')).toBe(false);
  });

  it('surfaces the unsupported Reveal state instead of an unhandled rejection', async () => {
    // Build the rejection so it is only "unhandled" if the component fails
    // to catch it. A pre-attached catch keeps the promise itself tame; the
    // process-level spy then fires only on a REAL unhandled rejection.
    let rejectReveal!: (e: Error) => void;
    const revealPromise = new Promise<void>((_resolve, reject) => {
      rejectReveal = reject;
    });
    revealPromise.catch(() => {});

    const revealExportedFile = jest.fn(() => {
      queueMicrotask(() => rejectReveal(new Error('unsupported: file reveal is not available')));
      return revealPromise;
    });
    const { backend } = makeExportBackend({ revealExportedFile });
    await exportThroughUI(backend);

    const unhandledSpy = jest.fn();
    process.on('unhandledRejection', unhandledSpy);

    fireEvent.click(screen.getByText('Reveal in File Manager'));
    const status = await screen.findByRole('status');
    expect(status.textContent).toContain('Reveal is not available');

    await new Promise((r) => setTimeout(r, 25));
    expect(unhandledSpy).not.toHaveBeenCalled();
    process.off('unhandledRejection', unhandledSpy);
  });
});

// ── 7. Conflict demo cannot mutate anything real ──────────────────────────────

describe('Conflict resolution demo screen', () => {
  it('is labeled as a demo and never calls backend.resolveConflict', async () => {
    const resolveConflict = jest.fn();
    const backend = { resolveConflict } as unknown as SermonBackend;
    render(
      <BackendProvider backend={backend}>
        <ConflictResolutionContent />
      </BackendProvider>,
    );

    // Explicit demo data boundary.
    expect(screen.getByText('Demo — Preview Data')).toBeTruthy();

    // Choosing a strategy and applying it only updates the local preview.
    fireEvent.click(screen.getByText('Keep My Version'));
    fireEvent.click(screen.getByText(/Apply Resolution/));
    await screen.findByText('Conflict Resolved');

    expect(resolveConflict).not.toHaveBeenCalled();
  });
});
