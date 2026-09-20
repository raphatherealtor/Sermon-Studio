'use client';
import { create } from 'zustand';
import type { SermonDocument, LintFinding, ConflictInfo } from '../backend/types';

interface EditorStore {
  activeSermonId: string | null;
  activeDocument: SermonDocument | null;
  isDirty: boolean;
  isSaving: boolean;
  lastSaved: string | null;
  saveError: string | null;
  lintFindings: LintFinding[];
  isLinting: boolean;
  conflictInfo: ConflictInfo | null;
  showMergeDrawer: boolean;
  wordCount: number;
  estimatedMinutes: number;

  setActiveSermon: (id: string | null) => void;
  setDocument: (doc: SermonDocument) => void;
  markDirty: () => void;
  markClean: () => void;
  setSaving: (saving: boolean) => void;
  setLastSaved: (ts: string) => void;
  setSaveError: (err: string | null) => void;
  setLintFindings: (findings: LintFinding[]) => void;
  setLinting: (linting: boolean) => void;
  setConflict: (info: ConflictInfo | null) => void;
  setShowMergeDrawer: (show: boolean) => void;
  updateBody: (html: string) => void;
  updateTitle: (title: string) => void;
  updateScripture: (scripture: string) => void;
  dismissLintFinding: (id: string) => void;
}

function computeWordCount(html: string): number {
  return html.replace(/<[^>]+>/g, '').split(/\s+/).filter(Boolean).length;
}

export const useEditorStore = create<EditorStore>((set) => ({
  activeSermonId: null,
  activeDocument: null,
  isDirty: false,
  isSaving: false,
  lastSaved: null,
  saveError: null,
  lintFindings: [],
  isLinting: false,
  conflictInfo: null,
  showMergeDrawer: false,
  wordCount: 0,
  estimatedMinutes: 0,

  setActiveSermon: (id) => set({ activeSermonId: id }),
  setDocument: (doc) => {
    const wc = computeWordCount(doc.body);
    set({
      activeDocument: doc,
      isDirty: false,
      wordCount: wc,
      estimatedMinutes: Math.round(wc / 130),
      conflictInfo: null,
      showMergeDrawer: false,
    });
  },
  markDirty: () => set({ isDirty: true }),
  markClean: () => set({ isDirty: false }),
  setSaving: (saving) => set({ isSaving: saving }),
  setLastSaved: (ts) => set({ lastSaved: ts }),
  setSaveError: (err) => set({ saveError: err }),
  setLintFindings: (findings) => set({ lintFindings: findings }),
  setLinting: (linting) => set({ isLinting: linting }),
  setConflict: (info) => set({ conflictInfo: info }),
  setShowMergeDrawer: (show) => set({ showMergeDrawer: show }),
  updateBody: (html) =>
    set((state) => {
      const wc = computeWordCount(html);
      return {
        activeDocument: state.activeDocument
          ? { ...state.activeDocument, body: html, updatedAt: new Date().toISOString() }
          : null,
        isDirty: true,
        wordCount: wc,
        estimatedMinutes: Math.round(wc / 130),
      };
    }),
  updateTitle: (title) =>
    set((state) => ({
      activeDocument: state.activeDocument
        ? { ...state.activeDocument, title }
        : null,
      isDirty: true,
    })),
  updateScripture: (scripture) =>
    set((state) => ({
      activeDocument: state.activeDocument
        ? { ...state.activeDocument, scripture }
        : null,
      isDirty: true,
    })),
  dismissLintFinding: (id) =>
    set((state) => ({
      lintFindings: state.lintFindings.map((f) =>
        f.id === id ? { ...f, dismissed: true } : f
      ),
    })),
}));