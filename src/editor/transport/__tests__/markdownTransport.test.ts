/**
 * Track H — Canonical Markdown transport tests.
 *
 * These verify the seam between TipTap editor state and the canonical
 * Markdown body contract. Directive fences must survive as text (the Rust
 * parser in crates/core remains authoritative; the frontend never parses
 * sermon grammar).
 */

import {
  createHeadlessSermonEditor,
  getEditorMarkdown,
  getActiveSermonMarkdown,
  registerSermonEditor,
  focusSermonEditorAt,
  lintIdentity,
  markdownToEditorDoc,
} from '../markdownTransport';
import type { SermonDocument } from '@/lib/backend/types';

const PROSE_MD = '# The Bread of Life\n\nEvery person has known hunger.\n\n## Exposition\n\nJesus said: I am the bread of life.';

describe('markdown → editor document', () => {
  it('parses Markdown headings and paragraphs into the editor doc', () => {
    const doc = markdownToEditorDoc(PROSE_MD);
    const json = JSON.stringify(doc);
    expect(json).toContain('"heading"');
    expect(json).toContain('The Bread of Life');
    expect(json).toContain('bread of life');
  });

  it('parses canonical Markdown (not HTML) from a string body', () => {
    const editor = createHeadlessSermonEditor(PROSE_MD);
    try {
      // If the body were parsed as HTML, the "#" would survive as literal text.
      expect(getEditorMarkdown(editor)).not.toContain('# Every');
      expect(editor.getText()).toContain('Every person has known hunger.');
    } finally {
      editor.destroy();
    }
  });
});

describe('editor → canonical Markdown', () => {
  it('serializes edits back to Markdown', () => {
    const editor = createHeadlessSermonEditor(PROSE_MD);
    try {
      editor.chain().focus('end').insertContent('\n\nEdited paragraph content.').run();
      const md = getEditorMarkdown(editor);
      expect(md).toContain('Edited paragraph content.');
      expect(md).toContain('# The Bread of Life');
    } finally {
      editor.destroy();
    }
  });

  it('a movement directive fence survives the round trip as text', () => {
    const md = 'Opening prose.\n\n:::movement{title="The Eternal Word"}\nIn the beginning was the Word.\n:::\n\nClosing prose.';
    const editor = createHeadlessSermonEditor(md);
    try {
      editor.commands.insertContent('Final paragraph.');
      const out = getEditorMarkdown(editor);
      expect(out).toContain(':::movement');
      expect(out).toContain('The Eternal Word');
      expect(out).toContain('In the beginning was the Word.');
      expect(out).toContain('Final paragraph.');
    } finally {
      editor.destroy();
    }
  });

  it('an unknown directive fence survives the round trip verbatim in content', () => {
    const md = 'Before.\n\n:::custom-block{foo="bar"}\nUnknown directive body must survive.\n:::\n\nAfter.';
    const editor = createHeadlessSermonEditor(md);
    try {
      const out = getEditorMarkdown(editor);
      expect(out).toContain(':::custom-block');
      expect(out).toContain('Unknown directive body must survive.');
    } finally {
      editor.destroy();
    }
  });
});

describe('editor registry (lint seam + navigation)', () => {
  it('returns live Markdown from the registered editor, null when unregistered', () => {
    expect(getActiveSermonMarkdown()).toBeNull();
    const editor = createHeadlessSermonEditor(PROSE_MD);
    try {
      registerSermonEditor(editor);
      editor.commands.insertContent('Live lint content.');
      // The lint seam sees exactly what a save would persist.
      expect(getActiveSermonMarkdown()).toContain('Live lint content.');
    } finally {
      registerSermonEditor(null);
      editor.destroy();
    }
    expect(getActiveSermonMarkdown()).toBeNull();
  });

  it('focusSermonEditorAt returns false with no editor, true with one', () => {
    expect(focusSermonEditorAt(1, 1)).toBe(false);
    const editor = createHeadlessSermonEditor(PROSE_MD);
    try {
      registerSermonEditor(editor);
      expect(focusSermonEditorAt(3, 5)).toBe(true);
      expect(editor.state.selection.empty).toBe(true);
    } finally {
      registerSermonEditor(null);
      editor.destroy();
    }
  });
});

describe('lintIdentity', () => {
  const baseDoc: SermonDocument = {
    id: 'sermon-001',
    title: 'The Bread of Life',
    scripture: 'John 6:35',
    series: null,
    status: 'in-progress',
    body: PROSE_MD,
    outline: [],
    tags: [],
    createdAt: '2026-09-01T09:00:00Z',
    updatedAt: '2026-09-01T09:00:00Z',
    preachedOn: null,
    version: 1,
    directives: [],
  };

  it('is stable for identical content', () => {
    expect(lintIdentity(baseDoc)).toBe(lintIdentity({ ...baseDoc }));
  });

  it('changes on a same-length edit (the old body.length weakness)', () => {
    // "advent" → "wonder": same length, different content.
    const before = lintIdentity(baseDoc);
    const edited = {
      ...baseDoc,
      body: baseDoc.body.replace('hunger', 'thirst'),
    };
    // Ensure the replacement actually happened and is same-length.
    expect(edited.body).not.toBe(baseDoc.body);
    expect(edited.body.length).toBe(baseDoc.body.length);
    expect(lintIdentity(edited)).not.toBe(before);
  });
});
