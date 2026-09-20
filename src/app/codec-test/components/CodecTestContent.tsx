'use client';
import React, { useState, useCallback } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import {
  CheckCircle, XCircle, Loader2, Code2, RefreshCw, ChevronRight,
  AlertTriangle, Info, Copy,
} from 'lucide-react';
import type { CodecRoundTripResult, ParsedDirective } from '@/lib/backend/types';

const KNOWN_DIRECTIVE_SAMPLE = `:::big-idea{text="Christ is the eternal Word who became flesh to dwell among us"}
The central claim of the Johannine prologue is the incarnation of the eternal Logos.
:::

:::application{point="1"}
Believe that Jesus is the Christ, the Son of God, and that by believing you may have life in his name.
:::

:::illustration{title="Augustine's Restless Heart"}
Augustine wrote: "Thou madest us for Thyself, and our heart is restless, until it repose in Thee."
:::`;

const UNKNOWN_DIRECTIVE_SAMPLE = `:::custom-block{foo="bar"}
This is an unknown directive body. It must survive round-trip without modification.
:::`;

const MIXED_SAMPLE = `:::big-idea{text="Justification is by faith alone, not works of the law"}
The central thesis of Romans 3:21–31 is the righteousness of God revealed through faith in Christ.
:::

:::custom-block{foo="bar"}
This is an unknown directive body. It must survive round-trip without modification.
:::

:::application{point="1"}
Trust in Christ alone for your standing before God — not in your moral performance.
:::

:::unknown-directive{x="1" y="2"}
Another unknown directive with multiple attributes.
:::

:::note{author="Calvin"}
"Faith alone justifies, but the faith that justifies is never alone."
:::`;

function DirectiveCard({ directive }: { directive: ParsedDirective }) {
  const isUnknown = directive.kind === 'unknown';
  return (
    <div className={`border rounded p-3 ${isUnknown ? 'border-fg-dim/30 bg-fg-dim/5' : 'border-accent/30 bg-accent/5'}`}>
      <div className="flex items-center gap-2 mb-2">
        <span className={`text-2xs font-mono-data font-600 px-1.5 py-0.5 rounded ${isUnknown ? 'bg-fg-dim/15 text-fg-dim' : 'bg-accent/15 text-accent'}`}>
          {isUnknown ? 'UNKNOWN' : 'KNOWN'}
        </span>
        <span className="text-xs font-mono-data font-600 text-fg">:::{directive.name}</span>
        {Object.entries(directive.attributes).map(([k, v]) => (
          <span key={k} className="text-2xs font-mono-data text-fg-dim">{k}=&quot;{v}&quot;</span>
        ))}
      </div>
      {directive.body && (
        <p className="text-xs text-fg/70 font-editor leading-relaxed border-l-2 border-border pl-2">
          {directive.body}
        </p>
      )}
    </div>
  );
}

export default function CodecTestContent() {
  const backend = useBackend();
  const [input, setInput] = useState(MIXED_SAMPLE);
  const [result, setResult] = useState<CodecRoundTripResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runTest = useCallback(async () => {
    if (!input.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const r = await backend.testDirectiveCodec(input);
      setResult(r);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Codec test failed');
    } finally {
      setLoading(false);
    }
  }, [backend, input]);

  const loadSample = (sample: string) => {
    setInput(sample);
    setResult(null);
  };

  const copyText = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-6 py-4 border-b border-border bg-panel flex-shrink-0">
        <Code2 size={16} className="text-accent" />
        <h1 className="text-lg font-600 text-fg">Directive Transport Codec Test</h1>
        <ChevronRight size={14} className="text-fg-dim" />
        <span className="text-sm text-fg-dim font-mono-data">Developer Tools</span>
        <div className="ml-auto flex items-center gap-2">
          <span className="text-2xs font-mono-data text-fg-dim bg-elevated px-2 py-1 rounded border border-border">
            Settings → Developer Tools → Codec Test
          </span>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-screen-xl mx-auto px-6 py-6 space-y-6">

          {/* Architecture note */}
          <div className="flex items-start gap-3 p-4 rounded bg-accent/5 border border-accent/20">
            <Info size={14} className="text-accent flex-shrink-0 mt-0.5" />
            <div className="text-xs text-fg-dim leading-relaxed">
              <strong className="text-fg">Directive Transport Contract:</strong> The frontend codec parses directive syntax for display purposes only.
              Unknown directives are preserved as opaque transport data — their name, attributes, body, and ordering must survive
              the full round-trip (load → editor state → save) without semantic loss.
              The Rust backend owns canonical AST validation; TypeScript must not recreate it.
            </div>
          </div>

          {/* Sample loaders */}
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-xs text-fg-dim font-mono-data">Load sample:</span>
            <button onClick={() => loadSample(KNOWN_DIRECTIVE_SAMPLE)} className="btn-ghost text-xs">
              Known Directives Only
            </button>
            <button onClick={() => loadSample(UNKNOWN_DIRECTIVE_SAMPLE)} className="btn-ghost text-xs">
              Unknown Directive Sample
            </button>
            <button onClick={() => loadSample(MIXED_SAMPLE)} className="btn-ghost text-xs">
              Mixed Known + Unknown
            </button>
          </div>

          <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
            {/* Input */}
            <div className="card-panel">
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-sm font-600 text-fg">Raw Input</h2>
                <div className="flex items-center gap-2">
                  <button onClick={() => copyText(input)} className="btn-ghost text-xs">
                    <Copy size={11} /> Copy
                  </button>
                  <button
                    onClick={runTest}
                    disabled={loading || !input.trim()}
                    className="btn-primary text-xs"
                  >
                    {loading ? (
                      <><Loader2 size={11} className="animate-spin-slow" /> Running…</>
                    ) : (
                      <><RefreshCw size={11} /> Run Test</>
                    )}
                  </button>
                </div>
              </div>
              <textarea
                value={input}
                onChange={(e) => { setInput(e.target.value); setResult(null); }}
                className="w-full h-64 font-mono-data text-xs bg-bg border border-border rounded p-3 text-fg resize-none focus:border-accent focus:outline-none"
                placeholder="Enter directive markdown here…"
                spellCheck={false}
              />
              <p className="text-2xs font-mono-data text-fg-dim mt-1">
                {input.split('\n').length} lines · {input.length} chars
              </p>
            </div>

            {/* Serialized output */}
            <div className="card-panel">
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-sm font-600 text-fg">Serialized Output</h2>
                {result && (
                  <button onClick={() => copyText(result.serialized)} className="btn-ghost text-xs">
                    <Copy size={11} /> Copy
                  </button>
                )}
              </div>
              {result ? (
                <pre className="w-full h-64 font-mono-data text-xs bg-bg border border-border rounded p-3 text-fg overflow-auto whitespace-pre-wrap">
                  {result.serialized || '(empty output)'}
                </pre>
              ) : (
                <div className="h-64 bg-bg border border-border rounded flex items-center justify-center">
                  <p className="text-xs text-fg-dim font-mono-data">Run the test to see serialized output</p>
                </div>
              )}
            </div>
          </div>

          {/* Round-trip result */}
          {error && (
            <div className="flex items-center gap-3 p-4 rounded bg-alert-red/10 border border-alert-red/30">
              <XCircle size={16} className="text-alert-red flex-shrink-0" />
              <p className="text-sm text-alert-red">{error}</p>
            </div>
          )}

          {result && (
            <div className={`flex items-center gap-3 p-4 rounded ${result.pass ? 'codec-pass' : 'codec-fail'}`}>
              {result.pass ? (
                <CheckCircle size={18} className="text-ok-green flex-shrink-0" />
              ) : (
                <XCircle size={18} className="text-alert-red flex-shrink-0" />
              )}
              <div>
                <p className={`text-sm font-600 ${result.pass ? 'text-ok-green' : 'text-alert-red'}`}>
                  Round-trip: {result.pass ? 'PASS' : 'FAIL'}
                </p>
                <p className="text-xs text-fg-dim mt-0.5">
                  {result.pass
                    ? `All ${result.parsed.length} directive(s) survived the round-trip intact. Unknown directives preserved verbatim.`
                    : 'One or more directives were lost or modified during serialization. Check the diff above.'}
                </p>
              </div>
            </div>
          )}

          {/* Parsed directives */}
          {result && result.parsed.length > 0 && (
            <div className="card-panel">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-sm font-600 text-fg">Parsed Directives</h2>
                <div className="flex items-center gap-3 text-2xs font-mono-data text-fg-dim">
                  <span className="text-accent">{result.parsed.filter((d) => d.kind === 'known').length} known</span>
                  <span className="text-fg-dim">{result.parsed.filter((d) => d.kind === 'unknown').length} unknown</span>
                </div>
              </div>
              <div className="space-y-2">
                {result.parsed.map((directive, i) => (
                  <DirectiveCard key={`dir-${i}`} directive={directive} />
                ))}
              </div>
            </div>
          )}

          {result && result.parsed.length === 0 && (
            <div className="card-panel text-center py-8">
              <AlertTriangle size={20} className="text-warn-amber mx-auto mb-2" />
              <p className="text-sm text-fg-dim">No directives found in input.</p>
              <p className="text-xs text-fg-dim mt-1">
                Directives use the syntax: <code className="font-mono-data text-accent bg-elevated px-1.5 py-0.5 rounded">:::name{'{'}attrs{'}'}</code>
              </p>
            </div>
          )}

          {/* Codec contract reference */}
          <div className="card-panel">
            <h2 className="text-sm font-600 text-fg mb-3">Codec Contract Reference</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-xs text-fg-dim">
              <div>
                <p className="font-600 text-fg mb-1">Known Directives (specialized rendering)</p>
                <ul className="space-y-0.5 font-mono-data">
                  {['big-idea', 'application', 'illustration', 'note', 'scripture', 'movement', 'warrant'].map((d) => (
                    <li key={d} className="text-accent">:::{d}</li>
                  ))}
                </ul>
              </div>
              <div>
                <p className="font-600 text-fg mb-1">Unknown Directive Requirements</p>
                <ul className="space-y-1">
                  <li>✓ Load successfully without error</li>
                  <li>✓ Remain editable as opaque blocks</li>
                  <li>✓ Preserve directive name verbatim</li>
                  <li>✓ Preserve all attributes verbatim</li>
                  <li>✓ Preserve body content verbatim</li>
                  <li>✓ Preserve ordering relative to other content</li>
                  <li>✓ Survive load → editor state → save</li>
                  <li>✗ Must NOT be silently normalized away</li>
                  <li>✗ Must NOT trigger Rust AST validation in TS</li>
                </ul>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
