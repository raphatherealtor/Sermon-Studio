/**
 * Release Track R — Reference Context data-layer tests.
 *
 * Covers the V1 contract of the light optional reference layer:
 * optional-with-default-none, clearly labeled fixtures (no fabricated
 * official wording), informational language only, no AI vocabulary, and
 * hard corpus isolation (the context module is never imported by the
 * backend, the index, Intelligence, or Research Packets).
 */
import * as fs from 'fs';
import * as path from 'path';
import {
  MINISTRY_CONTEXTS,
  REFERENCE_CONTEXT_SENTINEL,
  type MinistryContext,
} from '../types';
import { referenceEntriesFor, referenceEntryById } from '../data';
import { getMinistryContext, setMinistryContext } from '../storage';

const SRC_ROOT = path.resolve(__dirname, '../../../../');

function* walk(dir: string): Generator<string> {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
      yield* walk(full);
    } else if (/\.(ts|tsx|rs)$/.test(entry.name)) {
      yield full;
    }
  }
}

beforeEach(() => {
  window.localStorage.clear();
});

describe('reference context catalog', () => {
  it('is optional and defaults to none', () => {
    expect(getMinistryContext()).toBe('none');
    expect(referenceEntriesFor('none')).toEqual([]);
    expect(referenceEntriesFor('nondenominational')).toEqual([]);
    expect(MINISTRY_CONTEXTS.map((c) => c.id)).toContain('none');
  });

  it('persists the selection locally and validates values', () => {
    setMinistryContext('cogic-pentecostal');
    expect(getMinistryContext()).toBe('cogic-pentecostal');
    window.localStorage.setItem('sermon-studio.reference-context', 'bogus' as MinistryContext);
    expect(getMinistryContext()).toBe('none');
  });

  it('provides clearly labeled fixture entries for the COGIC/Pentecostal context', () => {
    const entries = referenceEntriesFor('cogic-pentecostal');
    expect(entries.length).toBeGreaterThanOrEqual(5);
    for (const entry of entries) {
      // Every entry is an explicitly labeled fixture — never official wording.
      expect(entry.summary).toMatch(/fixture/i);
      expect(entry.attribution).toMatch(/fixture/i);
      expect(entry.scriptures.length).toBeGreaterThan(0);
      for (const seeAlso of entry.seeAlso) {
        expect(referenceEntryById('cogic-pentecostal', seeAlso)).not.toBeNull();
      }
    }
    const topics = entries.map((e) => e.topic).join(' ');
    for (const expected of ['Sanctification', 'Holy Spirit', 'Salvation', 'Healing', 'Holiness']) {
      expect(topics).toContain(expected);
    }
  });

  it('uses informational language only — no approval/rejection wording', () => {
    const files = [
      path.join(SRC_ROOT, 'src/lib/reference-context/types.ts'),
      path.join(SRC_ROOT, 'src/lib/reference-context/data.ts'),
      path.join(SRC_ROOT, 'src/app/components/editor/ReferenceContextPanel.tsx'),
    ];
    const combined = files.map((f) => fs.readFileSync(f, 'utf8')).join('\n');
    const forbidden = [
      /\bapproved\b/i,
      /\bincorrect\b/i,
      /should preach/i,
      /\bconflicts?\b/i,
      /correct interpretation/i,
    ];
    for (const pattern of forbidden) {
      expect(combined).not.toMatch(pattern);
    }
    for (const allowed of ['See also', 'Related', 'Reference', 'compare']) {
      expect(combined).toContain(allowed);
    }
  });

  it('contains no AI vocabulary', () => {
    const files = [
      path.join(SRC_ROOT, 'src/lib/reference-context/types.ts'),
      path.join(SRC_ROOT, 'src/lib/reference-context/data.ts'),
      path.join(SRC_ROOT, 'src/app/components/editor/ReferenceContextPanel.tsx'),
    ];
    const combined = files.map((f) => fs.readFileSync(f, 'utf8')).join('\n');
    for (const pattern of [/\bArmarius\b/, /\bLLM\b/, /\bGPT\b/, /\bgenerative\b/i]) {
      expect(combined).not.toMatch(pattern);
    }
  });
});

describe('reference context corpus isolation', () => {
  it('the backend, index, intelligence, and research-packet layers never import the context module', () => {
    const guardedDirs = [
      path.join(SRC_ROOT, 'src/lib/backend'),
      path.join(SRC_ROOT, 'src-tauri/src'),
      path.join(SRC_ROOT, 'crates/core/src'),
    ];
    const offenders: string[] = [];
    for (const dir of guardedDirs) {
      for (const file of walk(dir)) {
        const text = fs.readFileSync(file, 'utf8');
        if (
          text.includes('reference-context') ||
          text.includes('REFERENCE_CONTEXT_SENTINEL') ||
          text.includes(REFERENCE_CONTEXT_SENTINEL)
        ) {
          offenders.push(file);
        }
      }
    }
    expect(offenders).toEqual([]);
  });

  it('the isolation sentinel never appears anywhere in the corpus or fixtures', () => {
    const guardedRoots = [
      path.join(SRC_ROOT, 'src/lib/backend'),
      path.join(SRC_ROOT, 'crates/core'),
      path.join(SRC_ROOT, 'src-tauri/src'),
    ];
    const hits: string[] = [];
    for (const root of guardedRoots) {
      for (const file of walk(root)) {
        if (fs.readFileSync(file, 'utf8').includes(REFERENCE_CONTEXT_SENTINEL)) {
          hits.push(file);
        }
      }
    }
    // The sentinel is defined once, in the context types module itself.
    expect(hits).toEqual([]);
  });

  it('persists only a context preference — never content — in localStorage', () => {
    setMinistryContext('cogic-pentecostal');
    const keys = Object.keys(window.localStorage);
    expect(keys).toEqual(['sermon-studio.reference-context']);
    expect(window.localStorage.getItem('sermon-studio.reference-context')).toBe('cogic-pentecostal');
  });
});
