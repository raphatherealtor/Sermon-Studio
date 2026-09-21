/**
 * Canonical Markdown Transport (Track H)
 *
 * Bridges the TipTap editor and the canonical Markdown body contract:
 * `SermonDocument.body` is canonical Markdown (prose + `:::` directive
 * fences), never HTML and never frontmatter (frontmatter is backend-owned).
 *
 * CONTRACT:
 * - Transport only: no sermon parsing, no directive grammar, no lint rules.
 *   Directive fences are plain paragraphs to the editor; the Rust backend
 *   (`crates/core/src/directive.rs`) remains the sole authoritative parser.
 * - `markdownToEditorDoc` / `getEditorMarkdown` use a headless TipTap editor
 *   with the SAME extension set as the React editor, so conversion results
 *   are identical everywhere (editor, tests, lint seam).
 * - The editor registry lets lint/navigation reach the live editor instance
 *   without prop drilling.
 */

import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import CharacterCount from '@tiptap/extension-character-count';
import Highlight from '@tiptap/extension-highlight';
import TextAlign from '@tiptap/extension-text-align';
import Underline from '@tiptap/extension-underline';
import { Markdown } from 'tiptap-markdown';
import type { SermonDocument } from '@/lib/backend/types';

/**
 * The single extension set used by the React editor and by headless
 * conversion helpers. Keep in sync with TipTapEditor.
 */
export function createSermonEditorExtensions() {
  return [
    StarterKit.configure({
      heading: { levels: [1, 2, 3] },
    }),
    Placeholder.configure({
      placeholder: 'Begin writing your sermon…',
    }),
    CharacterCount,
    Highlight.configure({ multicolor: false }),
    TextAlign.configure({ types: ['heading', 'paragraph'] }),
    Underline,
    Markdown.configure({ html: false, tightLists: true, linkify: false }),
  ];
}

/** Create a headless editor for Markdown ⇆ editor-state conversion (jsdom-safe). */
export function createHeadlessSermonEditor(markdown: string): Editor {
  return new Editor({
    extensions: createSermonEditorExtensions(),
    content: markdown,
  });
}

/** Parse canonical Markdown into the editor's ProseMirror document JSON. */
export function markdownToEditorDoc(markdown: string): Record<string, unknown> {
  const editor = createHeadlessSermonEditor(markdown);
  try {
    return editor.getJSON() as Record<string, unknown>;
  } finally {
    editor.destroy();
  }
}

/** Serialize an editor instance's current state to canonical Markdown. */
export function getEditorMarkdown(editor: Editor): string {
  const storage = editor.storage as { markdown?: { getMarkdown?: () => string } };
  return storage.markdown?.getMarkdown?.() ?? editor.getText();
}

/**
 * Serialize an in-memory editor state to the same canonical Markdown that
 * save would persist. This is the lint-seam fix: linting must see exactly
 * what a save would write (the editor's live state), not a stale buffer.
 */
export function getActiveSermonMarkdown(): string | null {
  const editor = getActiveSermonEditor();
  return editor ? getEditorMarkdown(editor) : null;
}

// ---------------------------------------------------------------------------
// Editor registry (lint seam + finding navigation)
// ---------------------------------------------------------------------------

let activeSermonEditor: Editor | null = null;

export function registerSermonEditor(editor: Editor | null): void {
  activeSermonEditor = editor;
}

export function getActiveSermonEditor(): Editor | null {
  return activeSermonEditor;
}

/**
 * Focus the active sermon editor at a 1-based (line, column) source location
 * from a lint finding. Maps the Markdown offset onto the nearest ProseMirror
 * text position and selects it.
 */
export function focusSermonEditorAt(line: number, col: number): boolean {
  const editor = activeSermonEditor;
  if (!editor) return false;

  const markdown = getEditorMarkdown(editor);
  const lines = markdown.split('\n');
  let offset = 0;
  const targetLine = Math.max(1, line);
  for (let i = 0; i < Math.min(targetLine - 1, lines.length); i++) {
    offset += lines[i].length + 1; // +1 for the newline
  }
  offset += Math.max(0, col - 1);

  // Walk text nodes in document order, accumulating text length (block
  // boundaries account for the "\n" the serializer emits), and select the
  // position whose accumulated range contains the offset.
  let acc = 0;
  let targetPos: number | null = null;
  editor.state.doc.descendants((node, pos) => {
    if (targetPos !== null) return false;
    if (node.isText && node.text) {
      if (offset <= acc + node.text.length) {
        targetPos = Math.min(pos + Math.max(0, offset - acc), pos + node.text.length);
        return false;
      }
      acc += node.text.length;
    } else if (node.isBlock) {
      // Block boundary: the Markdown serializer joins blocks with a newline.
      if (acc > 0) acc += 1;
    }
    return true;
  });

  if (targetPos === null) {
    // Past the end: place the selection at the document end.
    targetPos = editor.state.doc.content.size - 2;
  }

  editor.chain().focus().setTextSelection(targetPos).scrollIntoView().run();
  return true;
}

// ---------------------------------------------------------------------------
// Lint identity (content identity, NOT body.length)
// ---------------------------------------------------------------------------

/**
 * Stable content identity for lint invalidation. The previous seam hashed
 * `body.length`, so a same-length edit (e.g. "advent" → "wonder") never
 * re-triggered the debounced lint. This hashes the full content instead.
 */
export function lintIdentity(doc: SermonDocument): string {
  const parts = [
    doc.id,
    doc.title,
    doc.scripture,
    doc.status,
    doc.body,
    doc.outline.map((o) => o.text).join('\n'),
  ].join('');
  let h = 5381;
  for (let i = 0; i < parts.length; i++) {
    h = ((h << 5) + h + parts.charCodeAt(i)) >>> 0;
  }
  return h.toString(36);
}
