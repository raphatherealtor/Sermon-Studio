/**
 * Track H — SermonEditorWorkspace startup + lint seam tests.
 *
 * - No hardcoded 'sermon-001': startup picks the first sermon from
 *   listSermons() and tolerates an empty vault.
 * - Lint re-trigger uses content identity (lintIdentity), so same-length
 *   edits re-lint; lintSermon receives the current unsaved body.
 */

import React from 'react';
import { render, act } from '@testing-library/react';
import SermonEditorWorkspace from '../SermonEditorWorkspace';
import { BackendProvider } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import type { SermonDocument, SermonSummary } from '@/lib/backend/types';
import type { SermonBackend } from '@/lib/backend/SermonBackend';
import ArchiveOverviewContent from '@/app/archive-overview/components/ArchiveOverviewContent';
import { fireEvent, screen, waitFor } from '@testing-library/react';

jest.mock('next/link', () => ({
  __esModule: true,
  default: ({ href, ...props }: { href: string; children: React.ReactNode }) =>
    React.createElement('a', {
      ...props,
      href,
      onClick: (event: React.MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault();
        window.history.pushState({}, '', href);
      },
    }),
}));
jest.mock('next/dynamic', () => () => () => null);

// The rails and the editor are not under test here; stub them so the
// workspace can mount without the full editor surface.
jest.mock('../editor/ArchiveRail', () => () => null);
jest.mock('../editor/EditorPanel', () => () => null);
jest.mock('../editor/StudyRail', () => () => null);

const DOC: SermonDocument = {
  id: 'sermon-real-first',
  title: 'The Bread of Life',
  scripture: 'John 6:35',
  series: null,
  status: 'in-progress',
  body: '# The Bread of Life\n\nEvery person has known hunger.\n',
  outline: [],
  tags: [],
  createdAt: '2026-09-01T09:00:00Z',
  updatedAt: '2026-09-01T09:00:00Z',
  preachedOn: null,
  version: 1,
  directives: [],
};

const SUMMARY: SermonSummary = {
  id: DOC.id,
  title: DOC.title,
  scripture: DOC.scripture,
  series: null,
  status: 'in-progress',
  wordCount: 8,
  createdAt: DOC.createdAt,
  updatedAt: DOC.updatedAt,
  preachedOn: null,
  tags: [],
};

function createBackend(overrides: Partial<SermonBackend> = {}): jest.Mocked<SermonBackend> {
  const backend = {
    listSermons: jest.fn().mockResolvedValue([SUMMARY]),
    loadSermon: jest.fn().mockResolvedValue(DOC),
    lintSermon: jest.fn().mockResolvedValue([]),
  };
  return backend as unknown as jest.Mocked<SermonBackend>;
}

async function renderWorkspace(backend: SermonBackend) {
  const view = render(
    <BackendProvider backend={backend}>
      <SermonEditorWorkspace />
    </BackendProvider>
  );
  // Flush the async startup (listSermons → loadSermon).
  await act(async () => {});
  return view;
}

describe('SermonEditorWorkspace startup', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    window.history.replaceState({}, '', '/');
    useEditorStore.setState({
      activeDocument: null,
      activeSermonId: null,
      lintFindings: [],
      isDirty: false,
    });
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('loads the first sermon from listSermons instead of a hardcoded id', async () => {
    const backend = createBackend();
    await renderWorkspace(backend);

    expect(backend.listSermons).toHaveBeenCalled();
    expect(backend.loadSermon).toHaveBeenCalledTimes(1);
    expect(backend.loadSermon).toHaveBeenCalledWith(DOC.id);
    expect(useEditorStore.getState().activeDocument?.id).toBe(DOC.id);
  });

  it('treats an empty vault as a valid state (no load, no crash)', async () => {
    const backend = createBackend();
    backend.listSermons.mockResolvedValue([]);
    const { container } = await renderWorkspace(backend);

    expect(backend.loadSermon).not.toHaveBeenCalled();
    expect(useEditorStore.getState().activeDocument).toBeNull();
    // Loading finished (skeleton gone) and no error boundary tripped.
    expect(container.querySelector('.animate-pulse')).toBeNull();
  });
});

describe('SermonEditorWorkspace lint seam', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    window.history.replaceState({}, '', '/');
    useEditorStore.setState({
      activeDocument: null,
      activeSermonId: null,
      lintFindings: [],
      isDirty: false,
    });
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('lints the current unsaved body after the debounce', async () => {
    const backend = createBackend();
    await renderWorkspace(backend);

    act(() => {
      jest.advanceTimersByTime(1600);
    });
    await act(async () => {});

    expect(backend.lintSermon).toHaveBeenCalledTimes(1);
    expect(backend.lintSermon).toHaveBeenCalledWith(
      expect.objectContaining({ id: DOC.id, body: DOC.body })
    );
  });

  it('re-lints on a same-length edit (content identity, not body.length)', async () => {
    const backend = createBackend();
    await renderWorkspace(backend);

    act(() => {
      jest.advanceTimersByTime(1600);
    });
    await act(async () => {});
    expect(backend.lintSermon).toHaveBeenCalledTimes(1);

    // Same length as DOC.body's replaced word, different content.
    const editedBody = DOC.body.replace('hunger', 'thirst');
    expect(editedBody.length).toBe(DOC.body.length);
    act(() => {
      useEditorStore.getState().updateBody(editedBody);
    });
    act(() => {
      jest.advanceTimersByTime(1600);
    });
    await act(async () => {});

    expect(backend.lintSermon).toHaveBeenCalledTimes(2);
    expect(backend.lintSermon).toHaveBeenLastCalledWith(
      expect.objectContaining({ body: editedBody })
    );
  });
});

describe('archive Open handoff', () => {
  beforeEach(() => {
    window.history.replaceState({}, '', '/archive-overview');
    useEditorStore.setState({ activeDocument: DOC, activeSermonId: DOC.id, isDirty: false, conflictInfo: null, isSaving: false });
  });

  afterEach(() => window.history.replaceState({}, '', '/'));

  it('opens selected sermon B through the editor backend/store lifecycle', async () => {
    const sermonB = { ...DOC, id: 'sermon-b', title: 'Sermon B' };
    const backend = createBackend();
    backend.listSermons.mockResolvedValue([SUMMARY, { ...SUMMARY, id: sermonB.id, title: sermonB.title }]);
    backend.loadSermon.mockImplementation(async (id) => id === sermonB.id ? sermonB : DOC);
    backend.getArchiveStats = jest.fn().mockResolvedValue({
      totalSermons: 2, totalSeries: 0, totalWords: 16, lastPreachedOn: null,
      oldestSermon: null, newestSermon: null, sermonsByStatus: { draft: 2 }, sermonsByMonth: [],
    });
    backend.getIllustrationFatigue = jest.fn().mockResolvedValue([]);

    const archive = render(<BackendProvider backend={backend}><ArchiveOverviewContent /></BackendProvider>);
    fireEvent.click(await screen.findByTitle('Open "Sermon B"'));
    expect(window.location.search).toBe('?sermonId=sermon-b');
    archive.unmount();

    await renderWorkspace(backend);
    expect(backend.loadSermon).toHaveBeenCalledWith(sermonB.id);
    expect(useEditorStore.getState().activeDocument?.id).toBe(sermonB.id);
  });

  it('saves a dirty current sermon before opening the requested one', async () => {
    window.history.replaceState({}, '', '/?sermonId=sermon-b');
    const sermonB = { ...DOC, id: 'sermon-b', title: 'Sermon B' };
    const backend = createBackend();
    backend.listSermons.mockResolvedValue([SUMMARY, { ...SUMMARY, id: sermonB.id, title: sermonB.title }]);
    backend.loadSermon.mockResolvedValue(sermonB);
    backend.saveSermon = jest.fn().mockResolvedValue({ success: true, savedAt: '2026-09-24T00:00:00Z', version: 2 });
    useEditorStore.setState({ isDirty: true });

    await renderWorkspace(backend);
    expect(backend.saveSermon).toHaveBeenCalledWith(DOC);
    expect(backend.saveSermon.mock.invocationCallOrder[0]).toBeLessThan(backend.loadSermon.mock.invocationCallOrder[0]);
    expect(useEditorStore.getState().activeDocument?.id).toBe(sermonB.id);
  });

  it('keeps the current sermon open when the pending save conflicts', async () => {
    window.history.replaceState({}, '', '/?sermonId=sermon-b');
    const backend = createBackend();
    backend.listSermons.mockResolvedValue([SUMMARY, { ...SUMMARY, id: 'sermon-b', title: 'Sermon B' }]);
    backend.saveSermon = jest.fn().mockResolvedValue({
      success: false,
      conflict: { diskModifiedAt: '2026-09-24T00:00:00Z', diskVersion: 2, diskWordCount: 9 },
    });
    useEditorStore.setState({ isDirty: true });

    await renderWorkspace(backend);
    await waitFor(() => expect(useEditorStore.getState().conflictInfo).not.toBeNull());
    expect(backend.loadSermon).not.toHaveBeenCalled();
    expect(useEditorStore.getState().activeDocument?.id).toBe(DOC.id);
    expect(useEditorStore.getState().conflictInfo).not.toBeNull();
  });
});
