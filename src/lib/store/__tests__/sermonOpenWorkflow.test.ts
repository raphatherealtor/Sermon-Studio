import { openSermon, switchToSermon } from '../sermonOpenWorkflow';
import { useEditorStore } from '../editorStore';
import type { SermonBackend } from '@/lib/backend/SermonBackend';
import type { SermonDocument } from '@/lib/backend/types';

const documentFor = (id: string): SermonDocument => ({
  id, title: id, scripture: 'John 6:35', series: null, status: 'draft', body: '# Sermon',
  outline: [], tags: [], createdAt: '2026-09-24T00:00:00Z', updatedAt: '2026-09-24T00:00:00Z',
  preachedOn: null, version: 1, directives: [],
});

function backend() {
  return {
    saveSermon: jest.fn().mockResolvedValue({ success: true, savedAt: '2026-09-24T00:00:00Z', version: 2 }),
    loadSermon: jest.fn().mockImplementation(async (id: string) => documentFor(id)),
    createSermon: jest.fn().mockResolvedValue(documentFor('new')),
  } as unknown as jest.Mocked<SermonBackend>;
}

beforeEach(() => {
  useEditorStore.setState({ activeDocument: documentFor('current'), activeSermonId: 'current',
    isDirty: true, isSaving: false, conflictInfo: null, saveError: null });
});

it('saves before opening the requested sermon', async () => {
  const api = backend();
  expect(await openSermon(api, 'target')).toBe(true);
  expect(api.saveSermon).toHaveBeenCalledWith(expect.objectContaining({ id: 'current' }));
  expect(api.saveSermon.mock.invocationCallOrder[0]).toBeLessThan(api.loadSermon.mock.invocationCallOrder[0]);
  expect(useEditorStore.getState().activeDocument?.id).toBe('target');
});

it('keeps the current sermon when the pending save conflicts', async () => {
  const api = backend();
  api.saveSermon.mockResolvedValue({ success: false, savedAt: '', version: 1, conflict: {
    localTitle: 'current', localModifiedAt: '2026-09-24T00:00:00Z',
    diskModifiedAt: '2026-09-24T00:00:00Z', diskVersion: 2, diskWordCount: 9,
    sourcePath: '', explanation: 'Conflict',
  } });
  expect(await openSermon(api, 'target')).toBe(false);
  expect(api.loadSermon).not.toHaveBeenCalled();
  expect(useEditorStore.getState().activeDocument?.id).toBe('current');
  expect(useEditorStore.getState().conflictInfo).not.toBeNull();
});

it('saves before creating a new sermon and does not create on failure', async () => {
  const api = backend();
  api.saveSermon.mockRejectedValue(new Error('Read-only vault'));
  expect(await switchToSermon(api, () => api.createSermon({ title: 'New' }))).toBe(false);
  expect(api.createSermon).not.toHaveBeenCalled();
  expect(useEditorStore.getState().activeDocument?.id).toBe('current');
});

it('rejects another switch while a save is pending', async () => {
  const api = backend();
  let completeSave!: (value: { success: boolean; savedAt: string; version: number }) => void;
  api.saveSermon.mockReturnValue(new Promise((resolve) => { completeSave = resolve; }));
  const first = openSermon(api, 'first');
  expect(await openSermon(api, 'second')).toBe(false);
  completeSave({ success: true, savedAt: '2026-09-24T00:00:00Z', version: 2 });
  expect(await first).toBe(true);
  expect(api.loadSermon).toHaveBeenCalledTimes(1);
  expect(useEditorStore.getState().activeDocument?.id).toBe('first');
});
