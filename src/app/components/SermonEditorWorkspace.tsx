'use client';
import React, { useEffect, useCallback, useRef, useState } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import ArchiveRail from './editor/ArchiveRail';
import EditorPanel from './editor/EditorPanel';
import StudyRail from './editor/StudyRail';
import type { SermonDocument } from '@/lib/backend/types';

interface SermonEditorWorkspaceProps {
  focusArchiveSearch?: boolean;
  onArchiveSearchFocused?: () => void;
}

export default function SermonEditorWorkspace({ focusArchiveSearch, onArchiveSearchFocused }: SermonEditorWorkspaceProps) {
  const backend = useBackend();
  const { setDocument, activeDocument, setLintFindings, setLinting, setActiveSermon } = useEditorStore();
  const [loading, setLoading] = React.useState(true);

  // Track previous lint key to debounce on meaningful changes
  const lintKeyRef = useRef<string>('');

  useEffect(() => {
    let cancelled = false;
    async function loadInitial() {
      try {
        const doc = await backend.loadSermon('sermon-001');
        if (!cancelled) {
          setDocument(doc);
          setActiveSermon(doc.id);
          setLoading(false);
        }
      } catch {
        if (!cancelled) setLoading(false);
      }
    }
    loadInitial();
    return () => { cancelled = true; };
  }, [backend, setDocument, setActiveSermon]);

  const runLint = useCallback(
    async (doc: SermonDocument) => {
      setLinting(true);
      try {
        // Fix 4: call lintSermon with the CURRENT in-memory SermonDocument
        const findings = await backend.lintSermon(doc);
        setLintFindings(findings);
      } finally {
        setLinting(false);
      }
    },
    [backend, setLinting, setLintFindings]
  );

  // Fix 4: Re-run linting when meaningful sermon content or metadata changes,
  // using a debounce. Track title + scripture + body length + outline length
  // as the lint key — not just sermon ID.
  useEffect(() => {
    if (!activeDocument) return;

    const lintKey = [
      activeDocument.id,
      activeDocument.title,
      activeDocument.scripture,
      activeDocument.body.length,
      activeDocument.outline.length,
      activeDocument.status,
    ].join('|');

    if (lintKey === lintKeyRef.current) return;
    lintKeyRef.current = lintKey;

    const t = setTimeout(() => runLint(activeDocument), 1500);
    return () => clearTimeout(t);
  }, [
    activeDocument,
    activeDocument?.id,
    activeDocument?.title,
    activeDocument?.scripture,
    activeDocument?.body,
    activeDocument?.outline,
    activeDocument?.status,
    runLint,
  ]);

  if (loading) {
    return (
      <div className="flex h-full">
        <div className="w-60 border-r border-border bg-panel p-3 flex flex-col gap-2 flex-shrink-0">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={`skel-archive-${i}`} className="animate-pulse bg-elevated rounded h-14" />
          ))}
        </div>
        <div className="flex-1 p-8 flex flex-col gap-4">
          <div className="animate-pulse bg-elevated rounded h-8 w-1/2" />
          <div className="animate-pulse bg-elevated rounded h-4 w-1/3" />
          <div className="animate-pulse bg-elevated rounded h-4 w-full" />
          <div className="animate-pulse bg-elevated rounded h-4 w-full" />
          <div className="animate-pulse bg-elevated rounded h-4 w-3/4" />
        </div>
        <div className="w-72 border-l border-border bg-panel p-3 flex flex-col gap-2 flex-shrink-0">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={`skel-study-${i}`} className="animate-pulse bg-elevated rounded h-12" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full overflow-hidden">
      <ArchiveRail
        focusSearch={focusArchiveSearch}
        onSearchFocused={onArchiveSearchFocused}
      />
      <EditorPanel />
      <StudyRail />
    </div>
  );
}