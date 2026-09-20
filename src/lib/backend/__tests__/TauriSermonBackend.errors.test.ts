/**
 * TauriSermonBackend error-mapping and transport-shape tests.
 *
 * The @tauri-apps/api module is virtual-mocked: these tests verify that
 * command names, payload casing, and backend error semantics cross the IPC
 * boundary exactly as the Rust command layer produces them — including that
 * unsupported / not-yet-linked errors never surface as fake successes.
 */

import { TauriSermonBackend, BackendCommandError } from '../TauriSermonBackend';

const mockInvoke = jest.fn();

jest.mock(
  '@tauri-apps/api/core',
  () => ({
    invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
  }),
  { virtual: true }
);

beforeEach(() => {
  mockInvoke.mockReset();
  mockInvoke.mockResolvedValue(undefined);
});

describe('command surface completeness', () => {
  const EXPECTED_METHODS = [
    'listSermons',
    'searchSermons',
    'createSermon',
    'loadSermon',
    'saveSermon',
    'renameSermon',
    'duplicateSermon',
    'archiveSermon',
    'deleteSermon',
    'pinSermon',
    'parseReferences',
    'lintSermon',
    'getPassage',
    'getStrongs',
    'getCrossReferences',
    'getPreachedOn',
    'syncIndex',
    'rebuildIndex',
    'rescanLibrary',
    'repairIndex',
    'cancelIndexOperation',
    'getIndexStatus',
    'getArchiveStats',
    'getIllustrationFatigue',
    'setLibrarianEnabled',
    'createExportSnapshot',
    'executeExportJob',
    'exportSermon',
    'revealExportedFile',
    'resolveConflict',
    'prepareDiff',
    'prepareMerge',
    'getFilesystemStatus',
    'recoverSermon',
    'reconnectSermon',
    'loadSettings',
    'saveSettings',
    'testDirectiveCodec',
  ] as const;

  it('implements every SermonBackend method', () => {
    const backend = new TauriSermonBackend();
    const missing: string[] = [];
    for (const m of EXPECTED_METHODS) {
      const key = m as keyof TauriSermonBackend;
      if (typeof backend[key] !== 'function') missing.push(m);
    }
    expect(missing).toEqual([]);
  });
});

describe('command names and payload casing', () => {
  it('uses snake_case command names with camelCase payloads', async () => {
    const backend = new TauriSermonBackend();
    await backend.loadSermon('abc');
    expect(mockInvoke).toHaveBeenCalledWith('load_sermon', { id: 'abc' });

    await backend.getFilesystemStatus('s1');
    expect(mockInvoke).toHaveBeenCalledWith('get_filesystem_status', { sermonId: 's1' });

    await backend.resolveConflict({
      sermonId: 's1',
      strategy: 'keep-local',
    });
    expect(mockInvoke).toHaveBeenCalledWith('resolve_conflict', {
      request: { sermonId: 's1', strategy: 'keep-local' },
    });

    await backend.parseReferences('John 3:16');
    expect(mockInvoke).toHaveBeenCalledWith('parse_references', { text: 'John 3:16' });
  });
});

describe('backend error mapping', () => {
  it('unsupported errors keep their semantics (never fake success)', async () => {
    mockInvoke.mockRejectedValue('unsupported: sermon pinning is not backed yet');
    const backend = new TauriSermonBackend();
    await expect(backend.pinSermon('x', true)).rejects.toMatchObject({
      name: 'BackendCommandError',
      code: 'unsupported',
      command: 'pin_sermon',
    });
  });

  it('Track D/E seams surface as not-linked, not success', async () => {
    mockInvoke.mockRejectedValue('export: not yet linked in this branch (Track D owns export)');
    const backend = new TauriSermonBackend();
    const failure = backend.exportSermon({
      sermonId: 's1',
      format: 'pulpit_manuscript',
      options: {},
    });
    await expect(failure).rejects.toBeInstanceOf(BackendCommandError);
    await expect(failure).rejects.toMatchObject({ code: 'not-linked' });
  });

  it('ordinary failures carry command context', async () => {
    mockInvoke.mockRejectedValue('sermon not found: gone');
    const backend = new TauriSermonBackend();
    await expect(backend.loadSermon('gone')).rejects.toMatchObject({
      code: 'command-failed',
      message: expect.stringContaining('[load_sermon]'),
    });
  });
});
