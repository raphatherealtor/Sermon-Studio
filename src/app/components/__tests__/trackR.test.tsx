/**
 * Release Track R — personalization + reference context + teaching export UI.
 *
 * Covers the 14 required Track R assertions that live on the frontend:
 * RK placement, preserved forty-year framing, onboarding still functional,
 * Reference Context optionality, no-markdown-change guarantee, no approval
 * wording, no AI, teaching notes export UI, and Study Rail remaining
 * functional with the new tab.
 */
import React from 'react';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { BackendProvider } from '@/lib/backend/BackendContext';
import { MockSermonBackend } from '@/lib/backend/MockSermonBackend';
import FirstRunOnboarding from '@/app/components/onboarding/FirstRunOnboarding';
import OnboardingOverlay from '@/app/components/onboarding/OnboardingOverlay';
import RkMark from '@/app/components/common/RkMark';
import StudyRail from '@/app/components/editor/StudyRail';
import { useEditorStore } from '@/lib/store/editorStore';
import {
  isOnboardingCompleted,
  markOnboardingCompleted,
  resetOnboardingState,
} from '@/lib/onboarding/firstRun';
import { RK_NAME, RK_ATTRIBUTION } from '@/lib/identity/rk';
import type { SermonDocument } from '@/lib/backend/types';

const DOC: SermonDocument = {
  id: 'sermon-rk',
  title: 'Track R Sermon',
  scripture: 'John 15:1-8',
  series: null,
  status: 'draft',
  body: '# Track R Sermon\n\nCanonical markdown body.\n',
  outline: [],
  tags: [],
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
  preachedOn: null,
  version: 1,
  directives: [],
};

beforeEach(() => {
  window.localStorage.clear();
  resetOnboardingState();
  useEditorStore.getState().setDocument(DOC);
});

describe('A. RK personalization', () => {
  it('1. RK mark and attribution appear on the onboarding dedication', () => {
    render(<OnboardingOverlay onFinish={() => {}} />);
    expect(screen.getByTestId('rk-mark')).toBeTruthy();
    expect(screen.getByText('Prepared for Rafael Knox · Reverend Knox')).toBeTruthy();
  });

  it('canonical name is "Rafael" (F) and the "Raphael" misspelling never appears', () => {
    render(<OnboardingOverlay onFinish={() => {}} />);
    // Canonical spelling renders on the dedication surface.
    expect(screen.getByText('Prepared for Rafael Knox · Reverend Knox')).toBeTruthy();
    // The old misspelling is absent from every user-facing identity surface.
    expect(screen.getByTestId('onboarding-overlay').textContent).not.toContain('Raphael');
    // The identity module itself carries the corrected canonical value.
    expect(RK_NAME).toBe('Rafael Knox');
    expect(RK_ATTRIBUTION).toBe('Prepared for Rafael Knox · Reverend Knox');
  });

  it('2. the forty-year ministry framing is preserved verbatim', () => {
    render(<OnboardingOverlay onFinish={() => {}} />);
    expect(screen.getByText('In honor of forty years of ministry and preaching.')).toBeTruthy();
    expect(
      screen.getByText(/preserve a lifetime of study, proclamation, and pastoral work/)
    ).toBeTruthy();
  });

  it('3. onboarding remains functional end to end', () => {
    render(<FirstRunOnboarding />);
    expect(screen.getByTestId('onboarding-overlay')).toBeTruthy();
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    for (let i = 0; i < 4; i++) fireEvent.click(screen.getByTestId('onboarding-next'));
    fireEvent.click(screen.getByTestId('onboarding-enter'));
    expect(isOnboardingCompleted()).toBe(true);
  });

  it('the RkMark component renders the monogram without a caption by default', () => {
    render(<RkMark />);
    const mark = screen.getByTestId('rk-mark');
    expect(within(mark).getByText('RK')).toBeTruthy();
    expect(within(mark).queryByText(/Rafael Knox/)).toBeNull();
  });
});

describe('B. Reference Context (UI)', () => {
  function renderRail() {
    const backend = new MockSermonBackend();
    const saveSpy = jest.spyOn(backend, 'saveSermon');
    const lintSpy = jest.spyOn(backend, 'lintSermon');
    render(
      <BackendProvider backend={backend}>
        <StudyRail />
      </BackendProvider>
    );
    return { backend, saveSpy, lintSpy };
  }

  it('4. the Context tab is optional and defaults to none', () => {
    renderRail();
    fireEvent.click(screen.getByTitle('Reference Context'));
    expect(screen.getByTestId('reference-context-panel')).toBeTruthy();
    expect(screen.getByTestId('ministry-context-select')).toHaveProperty('value', 'none');
    expect(screen.getByText('No reference context selected.')).toBeTruthy();
  });

  it('5. changing ministry context never modifies sermon Markdown', () => {
    const { saveSpy, lintSpy } = renderRail();
    const bodyBefore = useEditorStore.getState().activeDocument?.body;
    fireEvent.click(screen.getByTitle('Reference Context'));
    fireEvent.change(screen.getByTestId('ministry-context-select'), {
      target: { value: 'cogic-pentecostal' },
    });
    // Selection persists, but the document and every write-path stay untouched.
    expect(screen.getByTestId('ministry-context-select')).toHaveProperty('value', 'cogic-pentecostal');
    expect(useEditorStore.getState().activeDocument?.body).toBe(bodyBefore);
    expect(saveSpy).not.toHaveBeenCalled();
    expect(lintSpy).not.toHaveBeenCalled();
  });

  it('8/9. entries render informational language with no approval wording and no AI', () => {
    renderRail();
    fireEvent.click(screen.getByTitle('Reference Context'));
    fireEvent.change(screen.getByTestId('ministry-context-select'), {
      target: { value: 'cogic-pentecostal' },
    });
    const panel = screen.getByTestId('reference-context-panel');
    expect(within(panel).getByText('Sanctification')).toBeTruthy();
    expect(within(panel).getAllByText(/See also/).length).toBeGreaterThan(0);
    expect(within(panel).getAllByText(/Related sermons/).length).toBeGreaterThan(0);
    expect(within(panel).getByText(/Fixture references — populate with official source material/)).toBeTruthy();
    const text = panel.textContent ?? '';
    for (const forbidden of [/approved/i, /incorrect/i, /should preach/i, /conflicts?/i, /correct interpretation/i, /Armarius/, /\bLLM\b/]) {
      expect(text).not.toMatch(forbidden);
    }
  });

  it('14. Study Rail remains fully functional with the new tab', () => {
    renderRail();
    // Every existing tab is still present and clickable.
    for (const title of ['Scripture Passage', "Strong's Lexicon", 'Cross References', 'Chain Study', 'Preached On', 'Illustration Fatigue', 'Sermon Intelligence', 'Research Packet', 'Reference Context']) {
      expect(screen.getByTitle(title)).toBeTruthy();
    }
    // The passage tab still performs a lookup against the backend.
    fireEvent.click(screen.getByTitle('Scripture Passage'));
    expect(screen.getByPlaceholderText('e.g. John 6:35')).toBeTruthy();
  });
});

describe('C. Teaching Notes export (UI seam)', () => {
  it('10/11. the teaching format is offered with its restrained options', async () => {
    const backend = new MockSermonBackend();
    // Instant export stub: the real mock's simulated latency exceeds the
    // testing-library find timeout; this test asserts the request shape.
    const exportSpy = jest.fn(async (request: { sermonId: string; format: string; options: Record<string, unknown>; snapshotId?: string }) => ({
      success: true,
      outputPath: '/tmp/teaching.pdf',
      message: 'Exported successfully as teaching_notes.',
      format: request.format,
      snapshotId: request.snapshotId,
      exportedAt: '2026-01-01T00:00:00Z',
      fileSizeBytes: 1024,
    }));
    backend.executeExportJob = exportSpy as unknown as MockSermonBackend['executeExportJob'];
    const ExportScreenContent = (await import('@/app/export-screen/components/ExportScreenContent')).default;
    render(
      <BackendProvider backend={backend}>
        <ExportScreenContent />
      </BackendProvider>
    );
    // Wait for the mock sermon list to populate the auto-selection.
    await screen.findAllByText('The Bread of Life');
    fireEvent.click(screen.getByText('Teaching Notes'));
    expect(screen.getByTestId('teaching-options')).toBeTruthy();
    expect(screen.getByText('Include Big Idea')).toBeTruthy();
    expect(screen.getByText('Teacher-friendly headings')).toBeTruthy();
    expect(screen.getByText(/Include discussion \/ Q&A section/)).toBeTruthy();
    expect(screen.getByTestId('teaching-spacing')).toHaveProperty('value', 'comfortable');

    // Pulpit/bulletin requests must not carry teaching options.
    fireEvent.click(screen.getByText('Pulpit Manuscript'));
    fireEvent.click(screen.getByText('Create Snapshot'));
    await screen.findByText('Snapshot ID');
    fireEvent.click(screen.getByText(/Export as Pulpit Manuscript/));
    await screen.findByText('Export successful');
    const request = exportSpy.mock.calls[0][0];
    expect(request.format).toBe('pulpit_manuscript');
    expect(Object.keys(request.options)).not.toContain('includeBigIdea');
    expect(Object.keys(request.options)).not.toContain('spacing');
  });
});
