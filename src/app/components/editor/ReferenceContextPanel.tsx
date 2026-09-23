/**
 * Release Track R — Reference Context panel (Study Rail surface).
 *
 * A light, optional, collapsible reference layer: the pastor picks the
 * ministry context he is working within and compares study material against
 * clearly labeled reference entries. Informational language only — Reference,
 * Compare, Related, See also. Never approval, correction, or instruction.
 *
 * Isolation: this panel reads a static frontend catalog and a localStorage
 * preference. It never writes to the vault, never calls a save/index
 * command, and never feeds Sermon Intelligence.
 */
'use client';
import React, { useEffect, useState } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { BookOpen, Loader2, Copy, CheckCircle } from 'lucide-react';
import {
  MINISTRY_CONTEXTS,
  type MinistryContext,
  type ReferenceContextEntry,
} from '@/lib/reference-context/types';
import { referenceEntriesFor } from '@/lib/reference-context/data';
import { getMinistryContext, setMinistryContext } from '@/lib/reference-context/storage';
import type { PreachedResult } from '@/lib/backend/types';

function ScriptureRow({ reference }: { reference: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = () => {
    navigator.clipboard.writeText(reference).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  return (
    <span className="inline-flex items-center gap-1">
      <span className="font-mono-data">{reference}</span>
      <button onClick={handleCopy} className="btn-ghost py-0 px-0.5 text-2xs" title={`Copy ${reference}`}>
        {copied ? <CheckCircle size={9} className="text-ok-green" /> : <Copy size={9} />}
      </button>
    </span>
  );
}

function ReferenceEntryCard({ entry, onSeeAlso }: { entry: ReferenceContextEntry; onSeeAlso: (id: string) => void }) {
  const backend = useBackend();
  const [related, setRelated] = useState<PreachedResult[] | null>(null);
  const [relatedLoading, setRelatedLoading] = useState(false);

  // Optional related sermons from the pastor's own archive, keyed by the
  // entry's first scripture anchor.
  const loadRelated = async () => {
    if (related !== null || relatedLoading) return;
    setRelatedLoading(true);
    try {
      const results = await backend.getPreachedOn(entry.scriptures[0] ?? '');
      setRelated(results);
    } catch {
      setRelated([]);
    } finally {
      setRelatedLoading(false);
    }
  };

  return (
    <details className="border border-border rounded-md" data-testid={`ref-ctx-${entry.id}`}>
      <summary className="px-2 py-1.5 cursor-pointer text-xs font-600 text-fg select-none">
        {entry.topic}
      </summary>
      <div className="px-2 pb-2 space-y-1.5 border-t border-border/50 pt-1.5">
        <p className="text-2xs text-fg-dim leading-relaxed">{entry.summary}</p>

        <div className="text-2xs text-fg-dim">
          <span className="font-mono-data uppercase tracking-widest">Reference · </span>
          {entry.scriptures.map((s, i) => (
            <React.Fragment key={`${entry.id}-scripture-${i}`}>
              {i > 0 && <span> · </span>}
              <ScriptureRow reference={s} />
            </React.Fragment>
          ))}
        </div>

        <p className="text-2xs font-mono-data text-fg-dim/80">{entry.attribution}</p>

        {entry.seeAlso.length > 0 && (
          <div className="text-2xs text-fg-dim flex flex-wrap items-center gap-1">
            <span className="font-mono-data uppercase tracking-widest">See also · </span>
            {entry.seeAlso.map((id) => (
              <button
                key={`${entry.id}-see-${id}`}
                onClick={() => onSeeAlso(id)}
                className="text-accent hover:text-gold-light transition-colors"
              >
                {id.replace(/-/g, ' ')}
              </button>
            ))}
          </div>
        )}

        <div>
          <button
            onClick={loadRelated}
            disabled={relatedLoading}
            className="btn-ghost py-0.5 px-1.5 text-2xs"
            data-testid={`ref-related-${entry.id}`}
          >
            {relatedLoading ? <Loader2 size={9} className="animate-spin-slow" /> : <BookOpen size={9} />}
            <span>Related sermons</span>
          </button>
          {related !== null && (
            <p className="text-2xs text-fg-dim mt-0.5">
              {related.length === 0
                ? 'No sermons on this passage yet.'
                : `Preached on ${entry.scriptures[0]}: ${related.map((r) => r.sermonTitle ?? r.sermonId).join(', ')}`}
            </p>
          )}
        </div>
      </div>
    </details>
  );
}

export default function ReferenceContextPanel() {
  const [context, setContext] = useState<MinistryContext>('none');
  const [mounted, setMounted] = useState(false);

  // Read the persisted preference only after mount (SSR-safe, mirrors the
  // first-run gate pattern).
  useEffect(() => {
    setContext(getMinistryContext());
    setMounted(true);
  }, []);

  const entries = referenceEntriesFor(context);

  const handleChange = (next: MinistryContext) => {
    setContext(next);
    setMinistryContext(next);
    // Intentionally no backend call: changing context never modifies sermon
    // content, the index, or any corpus.
  };

  return (
    <div className="p-3 space-y-2" data-testid="reference-context-panel">
      <div>
        <label className="block text-2xs font-mono-data uppercase tracking-widest text-fg-dim mb-1">
          Ministry context
        </label>
        <select
          value={context}
          onChange={(e) => handleChange(e.target.value as MinistryContext)}
          className="input-field text-xs font-mono-data w-full"
          data-testid="ministry-context-select"
        >
          {MINISTRY_CONTEXTS.map((c) => (
            <option key={`ctx-${c.id}`} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
      </div>

      <p className="text-2xs text-fg-dim leading-relaxed">
        Reference only — compare study material against the context you are
        working within. This layer never changes your sermon and never speaks
        for your own convictions.
      </p>

      {mounted && context === 'none' && (
        <p className="text-xs text-fg-dim py-4 text-center">
          No reference context selected.
        </p>
      )}

      {mounted && context === 'nondenominational' && (
        <p className="text-xs text-fg-dim py-4 text-center">
          No reference entries configured for this context yet.
        </p>
      )}

      {entries.map((entry) => (
        <ReferenceEntryCard
          key={`ref-entry-${entry.id}`}
          entry={entry}
          onSeeAlso={(id) => {
            // Open the related entry in place (native <details>) and bring
            // it into view — See also stays a lightweight in-panel link.
            const target = document.querySelector(
              `[data-testid="ref-ctx-${id}"]`,
            ) as HTMLDetailsElement | null;
            if (target) {
              target.open = true;
              target.scrollIntoView({ block: 'nearest' });
            }
          }}
        />
      ))}

      {entries.length > 0 && (
        <p className="text-2xs font-mono-data text-fg-dim/70 pt-1">
          Fixture references — populate with official source material.
        </p>
      )}
    </div>
  );
}
