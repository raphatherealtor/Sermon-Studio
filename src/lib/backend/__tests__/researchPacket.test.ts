/**
 * Track O — research-packet transport tests (browser mock backend).
 *
 * These cover the frontend transport seam only: the mock backend preserves
 * the research-packet wire shapes so the StudyRail surface is developable in
 * the browser. Rust owns real content-addressing, confinement, extraction,
 * and manifest semantics (crates/core/src/research_store.rs).
 */
import { describe, it, expect, jest, afterEach } from '@jest/globals';
import { MockSermonBackend } from '../MockSermonBackend';
import type {
  AttachResearchFileRequest,
  UpdateResearchMetadataRequest,
} from '../contracts/research_packet';

const SERMON_ID = 'sermon-001';
const req = (path: string): AttachResearchFileRequest => ({
  sermonId: SERMON_ID,
  sourcePath: path,
});

afterEach(() => {
  jest.restoreAllMocks();
});

describe('research packet transport (mock backend)', () => {
  it('attaches a PDF and lists it with research-packet provenance', async () => {
    const backend = new MockSermonBackend();
    const att = await backend.attachResearchFile(req('/home/pastor/commentary.pdf'));

    expect(att.provenanceClass).toBe('research-packet');
    expect(att.originalFilename).toBe('commentary.pdf');
    expect(att.extraction).toBe('ok');
    expect(att.importProvenance.importedFrom).toBe('/home/pastor/commentary.pdf');

    const list = await backend.listResearchAttachments(SERMON_ID);
    expect(list).toHaveLength(1);
    expect(list[0].id).toBe(att.id);
  });

  it('deduplicates the same source path idempotently', async () => {
    const backend = new MockSermonBackend();
    const first = await backend.attachResearchFile(req('/docs/a.pdf'));
    const second = await backend.attachResearchFile(req('/docs/a.pdf'));
    expect(second.id).toBe(first.id);
    const list = await backend.listResearchAttachments(SERMON_ID);
    expect(list).toHaveLength(1);
  });

  it('keeps packets isolated per sermon', async () => {
    const backend = new MockSermonBackend();
    await backend.attachResearchFile(req('/docs/a.pdf'));
    expect(await backend.listResearchAttachments('sermon-other')).toEqual([]);
  });

  it('returns canned page-aware extraction', async () => {
    const backend = new MockSermonBackend();
    const att = await backend.attachResearchFile(req('/docs/a.pdf'));
    const pages = await backend.getExtractedPages(SERMON_ID, att.id);
    expect(pages.length).toBeGreaterThan(0);
    expect(pages[0].page).toBe(1);
    expect(typeof pages[0].text).toBe('string');
  });

  it('updates metadata and preserves identity', async () => {
    const backend = new MockSermonBackend();
    const att = await backend.attachResearchFile(req('/docs/a.pdf'));
    const patch: UpdateResearchMetadataRequest = {
      title: 'Barth on Romans',
      author: 'Karl Barth',
      userNotes: 'Check the excursus on 9:6.',
    };
    const updated = await backend.updateResearchMetadata(SERMON_ID, att.id, patch);
    expect(updated.id).toBe(att.id);
    expect(updated.title).toBe('Barth on Romans');
    expect(updated.userNotes).toBe('Check the excursus on 9:6.');
    const fetched = await backend.getResearchAttachment(SERMON_ID, att.id);
    expect(fetched.author).toBe('Karl Barth');
  });

  it('removes an attachment and its extracted pages', async () => {
    const backend = new MockSermonBackend();
    const att = await backend.attachResearchFile(req('/docs/a.pdf'));
    await backend.removeResearchAttachment(SERMON_ID, att.id);
    expect(await backend.listResearchAttachments(SERMON_ID)).toEqual([]);
    await expect(backend.getExtractedPages(SERMON_ID, att.id)).rejects.toThrow();
  });

  it('rejects unknown attachment ids', async () => {
    const backend = new MockSermonBackend();
    await expect(backend.getResearchAttachment(SERMON_ID, 'nope')).rejects.toThrow();
    await expect(backend.removeResearchAttachment(SERMON_ID, 'nope')).rejects.toThrow();
  });

  it('open returns the imported source path (mock transport)', async () => {
    const backend = new MockSermonBackend();
    const att = await backend.attachResearchFile(req('/docs/a.pdf'));
    const res = await backend.openResearchFile(SERMON_ID, att.id);
    expect(res.absolutePath).toBe('/docs/a.pdf');
  });

  it('performs no network activity during the round trip', async () => {
    const originalFetch = globalThis.fetch;
    const fetchMock = jest.fn();
    // jsdom may not expose fetch; install a probe in either case.
    (globalThis as { fetch?: unknown }).fetch = fetchMock;
    const xhrSpy = jest.spyOn(XMLHttpRequest.prototype, 'open');
    const backend = new MockSermonBackend();
    const att = await backend.attachResearchFile(req('/docs/a.pdf'));
    await backend.listResearchAttachments(SERMON_ID);
    await backend.getExtractedPages(SERMON_ID, att.id);
    await backend.updateResearchMetadata(SERMON_ID, att.id, { userNotes: 'n' });
    await backend.openResearchFile(SERMON_ID, att.id);
    await backend.removeResearchAttachment(SERMON_ID, att.id);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(xhrSpy).not.toHaveBeenCalled();
    (globalThis as { fetch?: unknown }).fetch = originalFetch;
  });
});
