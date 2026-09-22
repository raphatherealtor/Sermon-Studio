'use client';
import React, { useCallback, useEffect, useState } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import {
  Lightbulb, ChevronDown, ChevronUp, Book, Archive, Loader2, AlertTriangle, RefreshCw,
} from 'lucide-react';
import type { IntelligenceResult, IntelligenceInsight, IntelligenceEvidence, SermonSummary } from '@/lib/backend/types';

// ─────────────────────────────────────────────────────────────────────────────
// Track L — deterministic Sermon Intelligence presentation.
//
// This component RENDERS backend-provided insights; it never computes scores,
// correlations, or evidence. Every insight is gated on having evidence, and a
// score is only ever shown next to that evidence (never as a bare number).
// ─────────────────────────────────────────────────────────────────────────────

const SOURCE_LABEL: Record<string, string> = {
  'your-archive': 'Your Archive',
  'biblical-study': 'Biblical Study',
};

function ProvenanceTag({ evidence }: { evidence: IntelligenceEvidence }) {
  const source = evidence.provenance.class;
  const Icon = source === 'biblical-study' ? Book : Archive;
  return (
    <span className="inline-flex items-center gap-1 text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded border border-border flex-shrink-0">
      <Icon size={9} />
      {SOURCE_LABEL[source]}
    </span>
  );
}

function EvidenceList({ evidence }: { evidence: IntelligenceEvidence[] }) {
  return (
    <ul className="space-y-1.5 border-l-2 border-border/60 pl-2">
      {evidence.map((ev, i) => (
        <li key={`evidence-${i}`} className="flex items-start justify-between gap-2">
          <p className="text-2xs text-fg/80 leading-snug">
            {ev.label}: <span className="text-fg">{ev.value}</span>
          </p>
          <ProvenanceTag evidence={ev} />
        </li>
      ))}
    </ul>
  );
}

function InsightCard({
  insight,
  sermonsById,
  onOpen,
}: {
  insight: IntelligenceInsight;
  sermonsById: Map<string, SermonSummary>;
  onOpen: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="border border-border rounded p-2.5 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-600 text-fg">{insight.title}</p>
          <p className="text-2xs text-fg-dim leading-snug mt-0.5">{insight.summary}</p>
        </div>
        {insight.score > 0 && (
          <span
            className="text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded border border-border flex-shrink-0"
            title="deterministic overlap score — see evidence below"
          >
            {insight.score}
          </span>
        )}
      </div>

      <button
        onClick={() => setOpen((o) => !o)}
        className="btn-ghost py-0.5 px-1.5 text-2xs flex items-center gap-1"
        aria-expanded={open}
      >
        {open ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
        Why this is related ({insight.evidence.length})
      </button>
      {open && <EvidenceList evidence={insight.evidence} />}

      {insight.relatedSermonIds.length > 0 && (
        <div className="space-y-1">
          {insight.relatedSermonIds.map((id) => {
            const sermon = sermonsById.get(id);
            return (
              <button
                key={id}
                onClick={() => onOpen(id)}
                className="w-full text-left border border-border/60 rounded p-1.5 hover:border-accent/40 transition-colors"
                title={`Open ${sermon?.title ?? id}`}
              >
                <p className="text-2xs font-600 text-accent truncate">{sermon?.title ?? id}</p>
                <p className="text-2xs font-mono-data text-fg-dim truncate">
                  {[sermon?.scripture, sermon?.preachedOn, sermon?.series].filter(Boolean).join(' · ')}
                </p>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default function InsightsPanel() {
  const backend = useBackend();
  const { activeDocument, setDocument, setActiveSermon } = useEditorStore();
  const [result, setResult] = useState<IntelligenceResult | null>(null);
  const [sermonsById, setSermonsById] = useState<Map<string, SermonSummary>>(new Map());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!activeDocument) return;
    setLoading(true);
    setError(null);
    try {
      const [insightsResult, sermons] = await Promise.all([
        backend.getSermonInsights(activeDocument.id),
        backend.listSermons(),
      ]);
      setResult(insightsResult);
      setSermonsById(new Map(sermons.map((s) => [s.id, s])));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unable to load intelligence.');
    } finally {
      setLoading(false);
    }
  }, [backend, activeDocument]);

  useEffect(() => {
    load();
  }, [load]);

  const openSermon = useCallback(
    async (id: string) => {
      const doc = await backend.loadSermon(id);
      setDocument(doc);
      setActiveSermon(id);
    },
    [backend, setDocument, setActiveSermon]
  );

  if (!activeDocument) {
    return <p className="text-xs text-fg-dim p-3">Select a sermon to see intelligence.</p>;
  }

  if (loading && !result) {
    return (
      <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
        <Loader2 size={14} className="animate-spin-slow" />
        <span className="text-xs">Gathering insights…</span>
      </div>
    );
  }

  if (error) {
    const notLinked = /unsupported|not linked/i.test(error);
    return (
      <div className="py-6 text-center px-3">
        <AlertTriangle size={16} className="text-warn-amber mx-auto mb-2" />
        <p className="text-xs text-fg-dim mb-2">
          {notLinked
            ? 'Sermon Intelligence is not available in this build yet.'
            : 'Unable to load intelligence.'}
        </p>
        <button onClick={load} className="btn-ghost text-xs">
          <RefreshCw size={11} /> Retry
        </button>
      </div>
    );
  }

  // Enforce the core rule defensively: no insight may render without evidence.
  const insights = (result?.insights ?? []).filter((i) => i.evidence.length > 0);

  return (
    <div className="p-3 space-y-3">
      {result?.biblicalDataAvailable === false && (
        <div className="flex items-start gap-2 p-2.5 rounded bg-warn/8 border border-warn/20">
          <AlertTriangle size={11} className="text-warn-amber flex-shrink-0 mt-0.5" />
          <p className="text-2xs text-fg-dim leading-snug">
            Biblical cross-reference data is unavailable until the study database is configured.
            Archive-derived patterns are shown below.
          </p>
        </div>
      )}

      {insights.length === 0 ? (
        <div className="py-8 text-center">
          <Lightbulb size={20} className="text-fg-dim mx-auto mb-2" />
          <p className="text-xs text-fg-dim">No insights yet — not enough evidence in your archive.</p>
        </div>
      ) : (
        <div className="fade-in space-y-2">
          {insights.map((insight) => (
            <InsightCard key={insight.id} insight={insight} sermonsById={sermonsById} onOpen={openSermon} />
          ))}
        </div>
      )}
    </div>
  );
}
