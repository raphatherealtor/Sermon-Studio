/**
 * Track L — deterministic Sermon Intelligence presentation tests.
 *
 * Proves the presentation layer renders backend-provided insight/evidence and
 * never computes scores, correlations, or AI prose. All intelligence content is
 * fixture-driven (Track J computes; Track L presents).
 */
import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { readFileSync } from 'fs';
import { join } from 'path';

import { MockSermonBackend } from '@/lib/backend/MockSermonBackend';
import { BackendProvider } from '@/lib/backend/BackendContext';
import type { SermonBackend } from '@/lib/backend/SermonBackend';
import type {
  IntelligenceResult, Insight, Evidence, SermonDocument, SermonSummary,
} from '@/lib/backend/types';
import InsightsPanel from '@/app/components/editor/InsightsPanel';
import StudyRail from '@/app/components/editor/StudyRail';
import { useEditorStore } from '@/lib/store/editorStore';

jest.mock('next/navigation', () => ({
  useRouter: () => ({ push: jest.fn(), replace: jest.fn() }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function resetStore(doc: SermonDocument) {
  useEditorStore.setState({
    activeSermonId: doc.id,
    activeDocument: doc,
    isDirty: false,
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

function makeDoc(id = 'doc-a', scripture = 'John 6:35'): SermonDocument {
  return {
    id,
    title: 'The Bread of Life',
    scripture,
    series: 'Gospel of John',
    status: 'draft',
    body: 'Body',
    outline: [],
    tags: [],
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    preachedOn: null,
    version: 1,
    directives: [],
  };
}

function ev(partial: Partial<Evidence> & { label: string; value: string }): Evidence {
  return { kind: 'series', weight: 1, sermonIds: [], references: [], source: 'archive', ...partial };
}

function ins(partial: Partial<Insight> & { id: string; title: string; summary: string }): Insight {
  return { kind: 'related-sermon', score: 0, evidence: [], relatedSermonIds: [], ...partial };
}

const RELATED_SUMMARY: SermonSummary = {
  id: 'sermon-002', title: 'Light of the World', scripture: 'John 8:12–20',
  series: 'Gospel of John', status: 'reviewed', wordCount: 2987,
  createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-01-01T00:00:00Z',
  preachedOn: null, tags: [],
};

function makeInsightsBackend(result: IntelligenceResult) {
  const loadSermon = jest.fn<Promise<SermonDocument>, [string]>(async (id) => makeDoc(id));
  const backend = {
    getSermonInsights: jest.fn(async () => result),
    listSermons: jest.fn(async () => [RELATED_SUMMARY] as SermonSummary[]),
    loadSermon,
  } as unknown as SermonBackend;
  return { backend, loadSermon };
}

function resultWith(insights: Insight[], biblicalDataAvailable = true): IntelligenceResult {
  return {
    engineVersion: 'test',
    generatedAt: '2026-09-20T00:00:00Z',
    subjectSermonId: 'doc-a',
    biblicalDataAvailable,
    insights,
  };
}

// ── 1. Mock fixtures: deterministic, evidence-backed, no computation ─────────

describe('MockSermonBackend intelligence fixtures', () => {
  it('every insight carries evidence', async () => {
    const backend = new MockSermonBackend();
    const result = await backend.getSermonInsights({ sermonId: 'sermon-001' });
    expect(result.insights.length).toBeGreaterThan(0);
    for (const insight of result.insights) {
      expect(insight.evidence.length).toBeGreaterThan(0);
    }
  });

  it('is deterministic across repeated calls', async () => {
    const backend = new MockSermonBackend();
    const a = await backend.getSermonInsights({ sermonId: 'sermon-001' });
    const b = await backend.getSermonInsights({ sermonId: 'sermon-001' });
    expect(a).toEqual(b);
  });

  it('returns an empty result for the empty/insufficient sentinels', async () => {
    const backend = new MockSermonBackend();
    for (const id of ['sermon-empty', 'sermon-insufficient']) {
      const result = await backend.getSermonInsights({ sermonId: id });
      expect(result.insights).toEqual([]);
      expect(result.biblicalDataAvailable).toBe(true);
    }
  });

  it('flags the missing-canon.db case and still returns archive-only insights', async () => {
    const backend = new MockSermonBackend();
    const result = await backend.getSermonInsights({ sermonId: 'sermon-no-canon' });
    expect(result.biblicalDataAvailable).toBe(false);
    expect(result.insights.length).toBeGreaterThan(0);
    for (const insight of result.insights) {
      for (const e of insight.evidence) {
        expect(e.source).toBe('archive');
      }
    }
  });

  it('getRelatedSermons returns only related-sermon insights', async () => {
    const backend = new MockSermonBackend();
    const result = await backend.getRelatedSermons('sermon-001');
    expect(result.insights.length).toBeGreaterThan(0);
    for (const insight of result.insights) {
      expect(insight.kind).toBe('related-sermon');
    }
  });

  it('getPassageHistory returns a passage-history insight with evidence', async () => {
    const backend = new MockSermonBackend();
    const result = await backend.getPassageHistory('John 6:35');
    expect(result.insights.length).toBeGreaterThan(0);
    for (const insight of result.insights) {
      expect(insight.kind).toBe('passage-history');
      expect(insight.evidence.length).toBeGreaterThan(0);
    }
  });
});

// ── 2. InsightsPanel rendering ───────────────────────────────────────────────

describe('InsightsPanel presentation', () => {
  it('renders only insights that carry evidence', async () => {
    const result = resultWith([
      ins({ id: 'a', title: 'Evidence-backed', summary: 'S1', evidence: [ev({ label: 'Same series', value: 'Gospel of John' })] }),
      ins({ id: 'b', title: 'No evidence', summary: 'S2', evidence: [] }),
    ]);
    const { backend } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText('Evidence-backed');
    expect(screen.queryByText('No evidence')).toBeNull();
  });

  it('shows an evidence affordance for every insight (why-this-is-related)', async () => {
    const result = resultWith([
      ins({
        id: 'a', title: 'Related to your series', summary: 'S', score: 87,
        evidence: [ev({ label: 'Same series', value: 'Gospel of John', source: 'archive' })],
      }),
    ]);
    const { backend } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText('Related to your series');
    expect(screen.getByText(/Why this is related \(1\)/)).toBeTruthy();
  });

  it('renders a useful empty state for an empty result', async () => {
    const result = resultWith([]);
    const { backend } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText(/No insights yet/);
  });

  it('renders archive insights and a clear biblical-data-unavailable notice without canon.db', async () => {
    const result = resultWith(
      [ins({ id: 'a', title: 'Archive pattern', summary: 'S', evidence: [ev({ label: 'Same series', value: 'Gospel of John', source: 'archive' })] })],
      false,
    );
    const { backend } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText('Archive pattern');
    expect(screen.getByText(/Biblical cross-reference data is unavailable/)).toBeTruthy();
  });

  it('renders provenance labels for evidence sources', async () => {
    const result = resultWith([
      ins({
        id: 'a', title: 'Mixed evidence', summary: 'S',
        evidence: [
          ev({ label: 'Same series', value: 'Gospel of John', source: 'archive' }),
          ev({ label: 'Shared reference', value: 'John 6:35', source: 'biblical-study', kind: 'shared-reference', references: ['John 6:35'] }),
        ],
      }),
    ]);
    const { backend } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText('Mixed evidence');
    fireEvent.click(screen.getByText(/Why this is related \(2\)/));
    await screen.findByText('Your Archive');
    expect(screen.getByText('Biblical Study')).toBeTruthy();
  });

  it('score is shown only alongside the evidence affordance', async () => {
    const result = resultWith([
      ins({ id: 'a', title: 'Scored insight', summary: 'S', score: 87, evidence: [ev({ label: 'Same series', value: 'Gospel of John' })] }),
    ]);
    const { backend } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText('Scored insight');
    expect(screen.getByText('87')).toBeTruthy();
    expect(screen.getByText(/Why this is related \(1\)/)).toBeTruthy();
  });

  it('navigates a related sermon by its existing id (loadSermon + store)', async () => {
    const result = resultWith([
      ins({
        id: 'a', title: 'Related sermons', summary: 'S', relatedSermonIds: ['sermon-002'],
        evidence: [ev({ label: 'Same series', value: 'Gospel of John', sermonIds: ['sermon-002'] })],
      }),
    ]);
    const { backend, loadSermon } = makeInsightsBackend(result);
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText('Light of the World');
    fireEvent.click(screen.getByText('Light of the World'));

    await waitFor(() => expect(loadSermon).toHaveBeenCalledWith('sermon-002'));
    expect(useEditorStore.getState().activeSermonId).toBe('sermon-002');
  });

  it('surfaces a not-linked native backend as a calm unavailable state', async () => {
    const backend = {
      getSermonInsights: jest.fn(async () => {
        throw new Error('[get_sermon_insights] unsupported: Sermon Intelligence engine is not linked yet');
      }),
      listSermons: jest.fn(async () => [] as SermonSummary[]),
    } as unknown as SermonBackend;
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <InsightsPanel />
      </BackendProvider>
    );
    await screen.findByText(/Sermon Intelligence is not available in this build yet/);
  });
});

// ── 3. Study Rail integration ────────────────────────────────────────────────

describe('StudyRail integration', () => {
  it('keeps the existing tabs and adds an Insights tab', async () => {
    const backend = {
      getPassage: jest.fn(async () => ({ reference: 'John 3:16', text: '', translation: 'KJV', verses: [] })),
      getStrongs: jest.fn(async () => ({ id: 'G1', lemma: '', transliteration: '', definition: '', gloss: '', partOfSpeech: '', occurrences: 0 })),
      getCrossReferences: jest.fn(async () => []),
      getPreachedOn: jest.fn(async () => []),
      getIllustrationFatigue: jest.fn(async () => []),
    } as unknown as SermonBackend;
    render(
      <BackendProvider backend={backend}>
        <StudyRail />
      </BackendProvider>
    );
    for (const label of ["Passage", "Strong's", 'X-Ref', 'History', 'Fatigue', 'Insights']) {
      expect(screen.getByText(label)).toBeTruthy();
    }
  });
});

// ── 4. Static guarantees: no AI wording / no computation in Track L ──────────

describe('Track L static guarantees', () => {
  it('contains no AI wording, scoring computation, or network calls', () => {
    const files = [
      join(__dirname, '..', 'app', 'components', 'editor', 'InsightsPanel.tsx'),
      join(__dirname, '..', 'lib', 'backend', 'types.ts'),
    ];
    const forbidden =
      /OpenAI|Anthropic|Claude|Gemini|LLM|GPT|chatbot|Armarius|cosine|tf-?idf|embedding|bm25|levenshtein|fetch\(|XMLHttpRequest|axios|\bAI\b|\bprompt\b|\btheme\b|tend toward|you usually|you believe|AI thinks/i;
    for (const file of files) {
      const src = readFileSync(file, 'utf8');
      expect(src).not.toMatch(forbidden);
    }
  });
});
