/**
 * Browser/static-preview behavior: with no Tauri runtime (and no
 * @tauri-apps/api module installed), every adapter call must fail gracefully
 * with BackendUnavailableError so the app can fall back to MockSermonBackend.
 */

import {
  TauriSermonBackend,
  BackendUnavailableError,
  isBackendUnavailableError,
} from '../TauriSermonBackend';

describe('graceful non-Tauri failure', () => {
  it('rejects with BackendUnavailableError instead of a raw module error', async () => {
    const backend = new TauriSermonBackend();
    let caught: unknown;
    try {
      await backend.listSermons();
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(BackendUnavailableError);
    expect(isBackendUnavailableError(caught)).toBe(true);
    expect((caught as Error).message).toContain('Native backend unavailable');
    expect((caught as Error).message).toContain('list_sermons');
  });

  it('marks the error with the unavailable code', async () => {
    const backend = new TauriSermonBackend();
    await expect(backend.getIndexStatus()).rejects.toMatchObject({
      code: 'unavailable',
      command: 'get_index_status',
    });
  });
});
