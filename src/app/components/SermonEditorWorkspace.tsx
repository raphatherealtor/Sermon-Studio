'use client';
import React, { useEffect, useCallback, useRef, useState } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import ArchiveRail from './editor/ArchiveRail';
import EditorPanel from './editor/EditorPanel';
import StudyRail from './editor/StudyRail';
import type { SermonDocument } from '@/lib/backend/types';
import { getActiveSermonMarkdown, lintIdentity } from '@/editor/transport/markdownTransport';
import { saveActiveSermon } from '@/lib/store/saveWorkflow';

interface SermonEditorWorkspaceProps {
  focusArchiveSearch?: boolean;
  onArchiveSearchFocused?: () => void;
}

export default function SermonEditorWorkspace({ focusArchiveSearch, onArchiveSearchFocused }: SermonEditorWorkspaceProps) {
  const backend = useBackend();
  const { setDocument, activeDocument, setLintFindings, setLinting, setActiveSermon } = useEditorStore();
  const [loading, setLoading] = React.useState(true);

  // Track previous lint identity to debounce on meaningful changes
  const lintKeyRef = useRef<string>('');

  useEffect(() => {
    let cancelled = false;
    async function loadInitial() {
      try {
        const requestedId = new URLSearchParams(window.location.search).get('sermonId');
        if (requestedId) {
          const current = useEditorStore.getState();
          if (current.activeSermonId === requestedId && current.activeDocument) {
            setLoading(false);
            return;
          }
          if (current.isDirty && !(await saveActiveSermon(backend))) {
            if (!cancelled) setLoading(false);
            return;
          }
        }
        // No hardcoded sermon id: pick the first sermon the backend knows
        // about. An empty vault is a valid state, not an error.
        const sermons = await backend.listSermons();
        if (cancelled) return;
        const target = requestedId ? sermons.find((sermon) => sermon.id === requestedId) : sermons[0];
        if (!target) {
          setLoading(false);
          return;
        }
        const doc = await backend.loadSermon(target.id);
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
        // Lint exactly what a save would persist: convert the live editor
        // state to canonical Markdown (the store buffer can lag the editor
        // by a debounce tick) before handing the document to the backend.
        const liveMarkdown = getActiveSermonMarkdown();
        const lintDoc = liveMarkdown !== null ? { ...doc, body: liveMarkdown } : doc;
        const findings = await backend.lintSermon(lintDoc);
        setLintFindings(findings);
      } finally {
        setLinting(false);
      }
    },
    [backend, setLinting, setLintFindings]
  );

  // Re-run linting when meaningful sermon content or metadata changes,
  // using a debounce. The lint identity is a content hash (NOT body.length,
  // which misses same-length edits like "advent" → "wonder").
  useEffect(() => {
    if (!activeDocument) return;

    const lintKey = lintIdentity(activeDocument);

    if (lintKey === lintKeyRef.current) return;
    lintKeyRef.current = lintKey;

    const t = setTimeout(() => runLint(activeDocument), 1500);
    return () => clearTimeout(t);
  }, [
    activeDocument,
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
