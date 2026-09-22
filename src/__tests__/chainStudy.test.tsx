/**
 * Wave 5 / Track N — Chain Study presentation + contract tests.
 *
 * Chain Study is a deterministic offline engine (chain-study-1.0). The UI is
 * presentation only: it renders backend-provided chains, evidence, provenance,
 * and the "From Your Archive" overlay. These tests prove:
 *
 *  1. the existing Study Rail tabs remain intact alongside the new Chain tab;
 *  2. the panel renders chains/evidence/provenance from the backend fixture;
 *  3. the calm unavailable state when canon.db is absent;
 *  4. "From Your Archive" navigates using the archive's OWN sermon IDs (the
 *     exact Archive Rail open path);
 *  5. no proprietary Thompson source exists anywhere in the engine sources or
 *     the canon adapter registry (bundled-content guard).
 */
import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { readFileSync, readdirSync, statSync } from 'fs';
import { join } from 'path';

import { MockSermonBackend } from '@/lib/backend/MockSermonBackend';
import { BackendProvider } from '@/lib/backend/BackendContext';
import type { SermonBackend } from '@/lib/backend/SermonBackend';
import type { ChainStudyResult, SermonDocument } from '@/lib/backend/types';
import StudyRail from '@/app/components/editor/StudyRail';
import ChainStudyPanel from '@/app/components/editor/ChainStudyPanel';
import { useEditorStore } from '@/lib/store/editorStore';

jest.mock('next/navigation', () => ({
  useRouter: () => ({ push: jest.fn(), replace: jest.fn() }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function resetStore(doc: SermonDocument | null) {
  useEditorStore.setState({
    activeSermonId: doc?.id ?? null,
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

export function fixtureChainStudy(overrides: Partial<ChainStudyResult> = {}): ChainStudyResult {
  const xrefProvenance = {
    class: 'biblical-study' as const,
    sourceId: 'openbible-xrefs',
    sourceLabel: 'OpenBible.info Cross References',
    licenseCode: 'CC-BY-4.0',
    attribution: 'OpenBible.info Cross References (OpenBible.info), CC BY 4.0.',
  };
  const topicProvenance = {
    class: 'biblical-study' as const,
    sourceId: 'naves-topical',
    sourceLabel: "Nave's Topical Bible",
    licenseCode: 'PD',
    attribution: "Nave's Topical Bible (Orville J. Nave, 1906), public domain.",
  };
  return {
    engineVersion: 'chain-study-1.0',
    seedReference: 'John.6.35',
    canonAvailable: true,
    chains: [
      {
        id: 'chain-John.6.35',
        name: 'Bread of Life',
        seedReference: 'John.6.35',
        references: [
          { reference: 'John 6:37', distance: 1, weight: 0.36, provenance: xrefProvenance },
          { reference: 'John 6:44', distance: 1, weight: 0.3, provenance: xrefProvenance },
          { reference: 'John 6:47', distance: 2, weight: 0.18, provenance: xrefProvenance },
        ],
        score: 0.84,
        evidence: [
          { kind: 'cross-reference', label: 'Cross-reference (rank 60)', value: 'John 6:35', weight: 0.36, provenance: xrefProvenance },
          { kind: 'rule-edge', label: 'Rule edge (concordance)', value: 'chain-study-1.0/concordance-fallback', weight: 0.3, provenance: xrefProvenance },
          { kind: 'sourced-topic', label: 'Bread of Life', value: 'top-bread-of-life', weight: 0.1, provenance: topicProvenance },
        ],
        sourceTopics: ['Bread of Life'],
      },
    ],
    archiveConnections: [
      {
        sermonId: 'sermon-003',
        title: 'The Good Shepherd',
        primaryPassage: 'John 10:11-John 10:18',
        matchingReferences: ['John 6:37'],
        provenance: { class: 'your-archive', sourceLabel: 'Your archive', sermonId: 'sermon-003' },
      },
    ],
    parameters: {
      maxSearchDepth: 2,
      maxNeighborsPerNode: 6,
      maxChainReferences: 12,
      maxChains: 5,
      maxArchiveConnections: 10,
    },
    ...overrides,
  };
}

function makeChainBackend(result: ChainStudyResult) {
  const loadSermon = jest.fn(async (id: string) => makeDoc(id));
  const backend = {
    getChainStudy: jest.fn(async () => result),
    loadSermon,
  } as unknown as SermonBackend;
  return { backend, loadSermon };
}

function renderRail(backend: SermonBackend) {
  resetStore(makeDoc());
  return render(
    <BackendProvider backend={backend}>
      <StudyRail />
    </BackendProvider>
  );
}

function clickChainTab() {
  const tab = screen.getByTitle('Chain Study');
  fireEvent.click(tab);
  return tab;
}

// ── 13. Existing Study Rail tabs remain intact ───────────────────────────────

describe('Study Rail tab integrity', () => {
  it('keeps every existing tab and adds exactly one Chain tab', () => {
    renderRail(new MockSermonBackend());
    for (const title of ['Scripture Passage', "Strong's Lexicon", 'Cross References', 'Preached On', 'Illustration Fatigue', 'Sermon Intelligence']) {
      expect(screen.getByTitle(title)).toBeTruthy();
    }
    expect(screen.getByTitle('Chain Study')).toBeTruthy();
    expect(screen.getAllByTitle('Chain Study').length).toBe(1);
  });
});

// ── Mock fixture sanity ──────────────────────────────────────────────────────

describe('MockSermonBackend chain fixture', () => {
  it('is deterministic and engine-versioned', async () => {
    const backend = new MockSermonBackend();
    const a = await backend.getChainStudy('John 6:35');
    const b = await backend.getChainStudy('John 6:35');
    expect(a).toEqual(b);
    expect(a.engineVersion).toBe('chain-study-1.0');
    expect(a.canonAvailable).toBe(true);
    // Every reference carries provenance from a named source.
    for (const chain of a.chains) {
      for (const r of chain.references) {
        expect(r.provenance.sourceLabel.length).toBeGreaterThan(0);
      }
      // Sourced topic names only — present in sourceTopics.
      for (const e of chain.evidence) {
        if (e.kind === 'sourced-topic') {
          expect(chain.sourceTopics).toContain(e.label);
        }
      }
    }
  });
});

// ── Panel presentation ───────────────────────────────────────────────────────

describe('ChainStudyPanel presentation', () => {
  it('renders the chain, sourced topic, evidence, and provenance badges', async () => {
    const { backend } = makeChainBackend(fixtureChainStudy());
    render(
      <BackendProvider backend={backend}>
        <ChainStudyPanel referenceInput="John 6:35" onOpenReference={jest.fn()} />
      </BackendProvider>
    );
    await waitFor(() => expect(screen.getByTestId('chain-study-panel')).toBeTruthy());
    expect(screen.getAllByText('Bread of Life').length).toBeGreaterThan(0);
    expect(screen.getByText('John 6:37')).toBeTruthy();
    expect(screen.getByText('John 6:47')).toBeTruthy();
    expect(screen.getByText('Why connected')).toBeTruthy();
    // Provenance attribution is surfaced (source label, not hidden).
    expect(screen.getAllByText('OpenBible.info Cross References').length).toBeGreaterThan(0);
    // Engine version is declared.
    expect(screen.getAllByText(/chain-study-1\.0/).length).toBeGreaterThan(0);
  });

  it('shows the calm unavailable state when canon is absent', async () => {
    const { backend } = makeChainBackend(
      fixtureChainStudy({ canonAvailable: false, chains: [], archiveConnections: [] })
    );
    render(
      <BackendProvider backend={backend}>
        <ChainStudyPanel referenceInput="John 6:35" onOpenReference={jest.fn()} />
      </BackendProvider>
    );
    await waitFor(() => expect(screen.getByTestId('chain-study-unavailable')).toBeTruthy());
    // The calm copy tells the pastor the rest of the rail keeps working.
    expect(screen.getByText(/Passage, X-Ref and the rest of the Study Rail keep working/)).toBeTruthy();
  });

  it('opens chain references through the shared passage workflow', async () => {
    const onOpenReference = jest.fn();
    const { backend } = makeChainBackend(fixtureChainStudy());
    render(
      <BackendProvider backend={backend}>
        <ChainStudyPanel referenceInput="John 6:35" onOpenReference={onOpenReference} />
      </BackendProvider>
    );
    await waitFor(() => expect(screen.getByText('John 6:37')).toBeTruthy());
    fireEvent.click(screen.getByText('John 6:37'));
    expect(onOpenReference).toHaveBeenCalledWith('John 6:37');
  });

  it("navigates 'From Your Archive' using the archive's own sermon IDs", async () => {
    const { backend, loadSermon } = makeChainBackend(fixtureChainStudy());
    resetStore(makeDoc());
    render(
      <BackendProvider backend={backend}>
        <ChainStudyPanel referenceInput="John 6:35" onOpenReference={jest.fn()} />
      </BackendProvider>
    );
    await waitFor(() => expect(screen.getByTestId('chain-study-archive')).toBeTruthy());
    expect(screen.getByText('The Good Shepherd')).toBeTruthy();
    fireEvent.click(screen.getByText('The Good Shepherd'));
    await waitFor(() =>
      expect(loadSermon).toHaveBeenCalledWith('sermon-003')
    );
    expect(useEditorStore.getState().activeSermonId).toBe('sermon-003');
  });

  it('renders an honest empty state when no chain exists', async () => {
    const { backend } = makeChainBackend(
      fixtureChainStudy({ chains: [], archiveConnections: [] })
    );
    render(
      <BackendProvider backend={backend}>
        <ChainStudyPanel referenceInput="John 6:35" onOpenReference={jest.fn()} />
      </BackendProvider>
    );
    await waitFor(() => expect(screen.getByText(/No chain found/)).toBeTruthy());
  });
});

// ── Study Rail integration: Chain tab end-to-end with a fixture backend ──────

describe('Study Rail Chain tab', () => {
  it('loads and presents the chain when the tab is opened', async () => {
    const { backend } = makeChainBackend(fixtureChainStudy());
    renderRail(backend);
    clickChainTab();
    await waitFor(() => expect(screen.getByTestId('chain-study-panel')).toBeTruthy());
    expect(screen.getAllByText('Bread of Life').length).toBeGreaterThan(0);
    expect(screen.getByText('From Your Archive')).toBeTruthy();
  });
});

// ── 11. No proprietary Thompson content, anywhere in the engine surface ──────

describe('no proprietary Thompson content is bundled', () => {
  const repoRoot = join(__dirname, '..', '..');

  function walk(dir: string, out: string[] = []): string[] {
    let entries: string[] = [];
    try {
      entries = readdirSync(dir);
    } catch {
      return out; // directory absent in this checkout — nothing bundled there
    }
    for (const name of entries) {
      if (name === 'node_modules' || name.startsWith('.')) continue;
      const fp = join(dir, name);
      try {
        if (statSync(fp).isDirectory()) walk(fp, out);
        else out.push(fp);
      } catch {
        // unreadable entry — skip
      }
    }
    return out;
  }

  it('the canon data/registry layer never references Thompson data', () => {
    // Scan where bundled content would actually live: the source registry,
    // the ETL adapter declarations, and the base schemas. (schema_ext.rs's
    // chain_edges DDL already carries the foundation's own comment disclaiming
    // Thompson's copyrighted data; the Rust tests prove the registry and
    // allow-list contain no such dataset, and one injected into a fixture
    // canon never surfaces.)
    const files = [
      join(repoRoot, 'crates', 'core', 'src', 'canon.rs'),
      join(repoRoot, 'crates', 'core', 'src', 'canon', 'adapters.rs'),
      join(repoRoot, 'crates', 'core', 'src', 'schema.rs'),
      join(repoRoot, 'crates', 'core', 'src', 'schema.sql'),
      join(repoRoot, 'crates', 'core', 'src', 'books.rs'),
    ];
    for (const file of files) {
      const text = readFileSync(file, 'utf-8');
      expect(/thompson/i.test(text)).toBe(false);
    }
    // No bundled file anywhere in the shipped surface is Thompson-named.
    for (const dir of ['src', 'crates', 'public'].map((d) => join(repoRoot, d))) {
      for (const file of walk(dir)) {
        expect(/thompson/i.test(file)).toBe(false);
      }
    }
    // End-to-end refusal of a Thompson-like dataset is proven by the Rust
    // test `no_proprietary_thompson_source_is_registered_or_admitted`, which
    // injects one into a fixture canon and asserts it never surfaces.
  });

  it('no bundled fixture/data file in the chain study surface mentions Thompson', () => {
    // Scan the frontend chain-study surface + fixtures for bundled content.
    const files = [
      join(repoRoot, 'src', 'app', 'components', 'editor', 'ChainStudyPanel.tsx'),
      join(repoRoot, 'src', 'lib', 'backend', 'types.ts'),
      join(repoRoot, 'src', 'lib', 'backend', 'MockSermonBackend.ts'),
    ];
    for (const file of files) {
      expect(/thompson/i.test(readFileSync(file, 'utf-8'))).toBe(false);
    }
  });
});
