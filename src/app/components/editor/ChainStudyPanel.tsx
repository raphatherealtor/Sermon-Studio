'use client';

/**
 * Chain Study (Wave 5 / Track N) — restrained Study Rail surface.
 *
 * Presents the deterministic offline engine output from
 * `get_chain_study`: bounded chain, sourced topics, traceable evidence
 * ("why they are connected"), provenance attribution, and a clearly-scoped
 * "From Your Archive" overlay that navigates with the archive's own sermon
 * IDs (same open path as the Archive Rail).
 *
 * Presentation only — no scoring, inference, or network. When canon.db is
 * absent (`canonAvailable === false`) this renders a calm unavailable state.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { GitBranch, Loader2, AlertTriangle, RefreshCw, Archive } from 'lucide-react';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import { openSermon } from '@/lib/store/sermonOpenWorkflow';
import type {
  ChainStudyResult,
  ChainStudyChain,
  ChainStudyArchiveConnection,
} from '@/lib/backend/types';

interface ChainStudyPanelProps {
  /** Shared reference input from the Study Rail (Passage/X-Ref/History). */
  referenceInput: string;
  /**
   * Open a chain reference in the Passage tab — implemented by the Study Rail
   * with its existing shared-reference workflow (no new state, no redesign).
   */
  onOpenReference: (reference: string) => void;
}

const KIND_LABELS: Record<string, string> = {
  'cross-reference': 'X-Ref',
  'rule-edge': 'Rule',
  'sourced-topic': 'Topic',
};

function ProvenanceBadge({ sourceLabel }: { sourceLabel: string }) {
  return (
    <span
      className="text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded border border-border"
      title={`Source: ${sourceLabel}`}
    >
      {sourceLabel}
    </span>
  );
}

function ChainView({
  chain,
  onOpenReference,
}: {
  chain: ChainStudyChain;
  onOpenReference: (reference: string) => void;
}) {
  return (
    <div className="fade-in space-y-2" data-testid="chain-study-chain">
      {/* Chain header: sourced name (never inferred) + score */}
      <div className="flex items-center justify-between">
        <span className="text-xs font-600 text-accent font-mono-data">
          {chain.name ?? `Chain from ${chain.seedReference}`}
        </span>
        <span
          className="text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded border border-border"
          title={`Chain weight score ${chain.score.toFixed(2)} (deterministic, engine chain-study-1.0)`}
        >
          score {chain.score.toFixed(2)}
        </span>
      </div>
      {chain.name && (
        <p className="text-2xs font-mono-data text-fg-dim">
          Seed: <span className="text-fg">{chain.seedReference}</span>
        </p>
      )}

      {/* Linked verses */}
      <div className="space-y-1.5">
        {chain.references.map((r) => (
          <div
            key={`chainref-${r.reference}`}
            className="border border-border rounded p-2 hover:border-accent/40 transition-colors group"
          >
            <div className="flex items-center justify-between">
              <button
                onClick={() => onOpenReference(r.reference)}
                className="text-xs font-mono-data font-600 text-accent hover:text-gold-light transition-colors"
                title="Open in Passage tab"
              >
                {r.reference}
              </button>
              <div className="flex items-center gap-1.5">
                <span
                  className="text-2xs font-mono-data text-fg-dim"
                  title={`Graph distance ${r.distance} from seed (max 2)`}
                >
                  d{r.distance}
                </span>
                <span
                  className="text-2xs font-mono-data text-fg-dim bg-elevated px-1 py-0.5 rounded"
                  title={`Weight ${r.weight.toFixed(2)} — provenance: ${r.provenance.sourceLabel}`}
                >
                  w {r.weight.toFixed(2)}
                </span>
              </div>
            </div>
            <div className="mt-1">
              <ProvenanceBadge sourceLabel={r.provenance.sourceLabel} />
            </div>
          </div>
        ))}
      </div>

      {/* Why they are connected */}
      {chain.evidence.length > 0 && (
        <div>
          <p className="text-2xs font-mono-data text-fg-dim uppercase tracking-widest mb-1 mt-2">
            Why connected
          </p>
          <div className="space-y-1">
            {chain.evidence.map((e, i) => (
              <div
                key={`chainev-${i}`}
                className="flex items-start gap-1.5 text-2xs text-fg-dim"
                title={`kind=${e.kind} · value=${e.value} · ${e.provenance.sourceLabel}`}
              >
                <span className="font-mono-data text-accent/80 flex-shrink-0">
                  {KIND_LABELS[e.kind] ?? e.kind}
                </span>
                <span className="flex-1">
                  {e.label}
                  {e.kind !== 'sourced-topic' && e.value && (
                    <span className="font-mono-data"> · {e.value}</span>
                  )}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function ArchiveOverlay({
  connections,
  onOpenSermon,
}: {
  connections: ChainStudyArchiveConnection[];
  onOpenSermon: (sermonId: string) => void;
}) {
  if (connections.length === 0) return null;
  return (
    <div className="mt-3 pt-2 border-t border-border" data-testid="chain-study-archive">
      <p className="text-2xs font-mono-data text-fg-dim uppercase tracking-widest mb-1.5 flex items-center gap-1">
        <Archive size={9} /> From Your Archive
      </p>
      <div className="space-y-1.5">
        {connections.map((c) => (
          <div key={c.sermonId} className="border border-border rounded p-2 hover:border-accent/40 transition-colors">
            <button
              onClick={() => onOpenSermon(c.sermonId)}
              className="text-xs font-600 text-fg hover:text-accent transition-colors text-left"
              title={`Open sermon ${c.sermonId}`}
            >
              {c.title}
            </button>
            <div className="text-2xs font-mono-data text-fg-dim mt-0.5">
              {c.primaryPassage}
              {c.matchingReferences.length > 0 && (
                <span> · touches {c.matchingReferences.join(', ')}</span>
              )}
            </div>
            <div className="mt-1">
              <ProvenanceBadge sourceLabel={c.provenance.sourceLabel} />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function ChainStudyPanel({ referenceInput, onOpenReference }: ChainStudyPanelProps) {
  const backend = useBackend();
  const { activeDocument } = useEditorStore();
  const [result, setResult] = useState<ChainStudyResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ranFor, setRanFor] = useState<string | null>(null);

  const seed = (referenceInput || activeDocument?.scripture || '').trim();

  const run = useCallback(async (ref: string) => {
    if (!ref) return;
    setLoading(true);
    setError(null);
    try {
      const r = await backend.getChainStudy(ref);
      setResult(r);
      setRanFor(ref);
    } catch {
      setError('Chain Study is unavailable right now.');
    } finally {
      setLoading(false);
    }
  }, [backend]);

  // Run once when the tab mounts, and re-run when the shared reference changes.
  useEffect(() => {
    if (seed) run(seed);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [seed]);

  const openArchiveSermon = useCallback(async (sermonId: string) => {
    await openSermon(backend, sermonId);
  }, [backend]);

  // Passage navigation stays inside the rail's shared reference workflow —
  // implemented by StudyRail via onOpenReference.

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
        <Loader2 size={14} className="animate-spin-slow" />
        <span className="text-xs">Resolving chain…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-3">
        <div className="py-4 text-center">
          <AlertTriangle size={16} className="text-alert-red mx-auto mb-2" />
          <p className="text-xs text-alert-red mb-2">{error}</p>
          <button onClick={() => run(seed)} className="btn-ghost text-xs">
            <RefreshCw size={11} /> Retry
          </button>
        </div>
      </div>
    );
  }

  // Calm unavailable state — canon.db absent/stale/pre-extension.
  if (result && !result.canonAvailable) {
    return (
      <div className="p-3" data-testid="chain-study-unavailable">
        <div className="py-8 text-center">
          <GitBranch size={20} className="text-fg-dim mx-auto mb-2" />
          <p className="text-xs text-fg-dim">
            Chain Study needs the canon library, which isn&apos;t set up on this
            device yet.
          </p>
          <p className="text-2xs text-fg-dim/60 mt-1">
            Passage, X-Ref and the rest of the Study Rail keep working.
          </p>
        </div>
      </div>
    );
  }

  if (!result) {
    return (
      <div className="py-8 text-center">
        <GitBranch size={20} className="text-fg-dim mx-auto mb-2" />
        <p className="text-xs text-fg-dim">Enter a reference above to trace a chain</p>
      </div>
    );
  }

  const chains = result.chains;
  return (
    <div className="p-3" data-testid="chain-study-panel">
      {chains.length === 0 && (
        <div className="py-8 text-center">
          <GitBranch size={20} className="text-fg-dim mx-auto mb-2" />
          <p className="text-xs text-fg-dim">
            No chain found for <span className="text-accent font-mono-data">{ranFor}</span>
          </p>
        </div>
      )}
      {chains.length > 0 && (
        <>
          <p className="text-2xs font-mono-data text-fg-dim mb-2 flex items-center justify-between">
            <span>
              <span className="text-accent">{result.engineVersion}</span> · deterministic · offline
            </span>
          </p>
          {chains.map((chain) => (
            <ChainView key={chain.id} chain={chain} onOpenReference={onOpenReference} />
          ))}
        </>
      )}

      {/* "From Your Archive" — additive overlay, provenance your-archive */}
      <ArchiveOverlay connections={result.archiveConnections} onOpenSermon={openArchiveSermon} />
    </div>
  );
}
