/**
 * BackendContext composition-root tests.
 *
 * Prove that:
 *   - browser/static preview resolves to MockSermonBackend
 *   - the native Tauri runtime resolves to TauriSermonBackend
 *   - detection is SSR/build-safe (no `window`)
 *   - React components consume the SermonBackend abstraction via useBackend()
 *   - a detected native runtime never silently falls back to mock data
 *   - no application component imports the concrete backends or @tauri-apps/api
 */
import { render } from '@testing-library/react';
import { readdirSync, readFileSync, statSync } from 'fs';
import { join, sep } from 'path';
import {
  BackendProvider,
  createRuntimeBackend,
  useBackend,
} from '../BackendContext';
import { isTauriRuntime } from '../runtime';
import { MockSermonBackend } from '../MockSermonBackend';
import { TauriSermonBackend } from '../TauriSermonBackend';
import type { SermonBackend } from '../SermonBackend';

const TAURI_INTERNALS = '__TAURI_INTERNALS__';

function setTauriRuntime(present: boolean) {
  const w = window as unknown as Record<string, unknown>;
  if (present) {
    w[TAURI_INTERNALS] = {};
  } else {
    delete w[TAURI_INTERNALS];
  }
}

afterEach(() => {
  setTauriRuntime(false);
});

describe('runtime backend selection', () => {
  it('resolves to MockSermonBackend in browser/static preview', () => {
    setTauriRuntime(false);
    expect(isTauriRuntime()).toBe(false);
    expect(createRuntimeBackend()).toBeInstanceOf(MockSermonBackend);
  });

  it('resolves to TauriSermonBackend in the native Tauri runtime', () => {
    setTauriRuntime(true);
    expect(isTauriRuntime()).toBe(true);
    expect(createRuntimeBackend()).toBeInstanceOf(TauriSermonBackend);
  });

  it('is SSR/build-safe: no window access when window is undefined', () => {
    const g = globalThis as { window?: unknown };
    const original = g.window;
    delete g.window;
    try {
      expect(isTauriRuntime()).toBe(false);
      expect(createRuntimeBackend()).toBeInstanceOf(MockSermonBackend);
    } finally {
      g.window = original;
    }
  });

  it('does not silently fall back to mock when native is detected', async () => {
    setTauriRuntime(true);
    const backend = createRuntimeBackend();
    expect(backend).toBeInstanceOf(TauriSermonBackend);
    expect(backend).not.toBeInstanceOf(MockSermonBackend);
    // A real native failure surfaces as an error; it is never disguised as
    // successful mock behavior (and never replaced with Mock data).
    await expect(backend.listSermons()).rejects.toThrow();
  });
});

describe('BackendProvider composition', () => {
  it('exposes the injected backend through useBackend()', () => {
    const stub = new MockSermonBackend();
    let seen: SermonBackend | null = null;
    function Probe() {
      seen = useBackend();
      return null;
    }
    render(
      <BackendProvider backend={stub}>
        <Probe />
      </BackendProvider>
    );
    expect(seen).toBe(stub);
  });

  it('defaults to runtime detection when no override is supplied', () => {
    setTauriRuntime(false);
    let seen: SermonBackend | null = null;
    function Probe() {
      seen = useBackend();
      return null;
    }
    render(
      <BackendProvider>
        <Probe />
      </BackendProvider>
    );
    expect(seen).toBeInstanceOf(MockSermonBackend);
  });
});

describe('application code stays on the abstraction', () => {
  it('no component/application module imports the concrete backends or @tauri-apps/api', () => {
    const srcRoot = join(__dirname, '..', '..', '..');
    const compositionRoot = join(srcRoot, 'lib', 'backend');

    const files: string[] = [];
    const walk = (dir: string) => {
      for (const entry of readdirSync(dir)) {
        const full = join(dir, entry);
        const st = statSync(full);
        if (st.isDirectory()) {
          if (entry === '__tests__') continue;
          walk(full);
        } else if (st.isFile() && /\.(ts|tsx)$/.test(entry)) {
          files.push(full);
        }
      }
    };
    walk(srcRoot);

    const forbidden = ['@tauri-apps/api', 'TauriSermonBackend', 'MockSermonBackend'];
    const offenders: string[] = [];
    for (const file of files) {
      if (file.startsWith(compositionRoot + sep)) continue; // the composition root is allowed
      const content = readFileSync(file, 'utf8');
      const specifiers = Array.from(
        content.matchAll(/(?:from\s+|import\s*\(\s*|require\s*\(\s*)['"]([^'"]+)['"]/g)
      ).map((m) => m[1]);
      if (specifiers.some((spec) => forbidden.some((f) => spec.includes(f)))) {
        offenders.push(file);
      }
    }
    expect(offenders).toEqual([]);
  });
});
