'use client';
import React, { useState, useCallback, useEffect } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import { Search, Book, Link2, Clock, Loader2, Hash, AlertTriangle, Copy, Plus, RefreshCw, X, CheckCircle, BarChart2, Lightbulb, FileText,  } from 'lucide-react';
import type {
  PassageResult, StrongsEntry, CrossReference, PreachedResult, IllustrationFatigueResult,
} from '@/lib/backend/types';
import type {
  ResearchAttachment,
  ExtractedPage,
} from '@/lib/backend/contracts/research_packet';
import InsightsPanel from './InsightsPanel';

type StudyTab = 'passage' | 'strongs' | 'xref' | 'history' | 'fatigue' | 'insights' | 'research';

const TAB_CONFIG: { id: StudyTab; label: string; icon: React.ElementType; title: string }[] = [
  { id: 'passage', label: 'Passage', icon: Book, title: 'Scripture Passage' },
  { id: 'strongs', label: "Strong's", icon: Hash, title: "Strong's Lexicon" },
  { id: 'xref', label: 'X-Ref', icon: Link2, title: 'Cross References' },
  { id: 'history', label: 'History', icon: Clock, title: 'Preached On' },
  { id: 'fatigue', label: 'Fatigue', icon: BarChart2, title: 'Illustration Fatigue' },
  { id: 'insights', label: 'Insights', icon: Lightbulb, title: 'Sermon Intelligence' },
  { id: 'research', label: 'Research', icon: FileText, title: 'Research Packet' },
];

function CopyButton({ text, label }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  return (
    <button onClick={handleCopy} className="btn-ghost py-0.5 px-1.5 text-2xs" title={`Copy ${label || 'text'}`}>
      {copied ? <CheckCircle size={10} className="text-ok-green" /> : <Copy size={10} />}
      {label && <span>{copied ? 'Copied' : label}</span>}
    </button>
  );
}

function InsertButton({ text, label }: { text: string; label?: string }) {
  const { updateBody, activeDocument } = useEditorStore();
  const handleInsert = () => {
    if (!activeDocument) return;
    // Body is canonical Markdown: a quoted passage is a Markdown blockquote.
    // (Block-level HTML would be refused by the backend save guard.)
    const quote = text
      .split('\n')
      .map((line) => (line.trim() ? `> ${line}` : '>'))
      .join('\n');
    const insertion = `\n${quote}\n`;
    updateBody(activeDocument.body + insertion);
  };
  return (
    <button onClick={handleInsert} className="btn-ghost py-0.5 px-1.5 text-2xs" title={`Insert ${label || 'into sermon'}`}>
      <Plus size={10} />
      {label && <span>{label}</span>}
    </button>
  );
}

export default function StudyRail() {
  const backend = useBackend();
  const { activeDocument } = useEditorStore();
  const [activeTab, setActiveTab] = useState<StudyTab>('passage');
  const [referenceInput, setReferenceInput] = useState('John 6:35');
  const [strongsInput, setStrongsInput] = useState('G740');

  const [passage, setPassage] = useState<PassageResult | null>(null);
  const [passageLoading, setPassageLoading] = useState(false);
  const [passageError, setPassageError] = useState<string | null>(null);

  const [strongs, setStrongs] = useState<StrongsEntry | null>(null);
  const [strongsLoading, setStrongsLoading] = useState(false);
  const [strongsError, setStrongsError] = useState<string | null>(null);

  const [xrefs, setXrefs] = useState<CrossReference[]>([]);
  const [xrefLoading, setXrefLoading] = useState(false);
  const [xrefError, setXrefError] = useState<string | null>(null);

  const [preached, setPreached] = useState<PreachedResult[]>([]);
  const [preachedLoading, setPreachedLoading] = useState(false);
  const [preachedError, setPreachedError] = useState<string | null>(null);

  const [fatigue, setFatigue] = useState<IllustrationFatigueResult[]>([]);
  const [fatigueLoading, setFatigueLoading] = useState(false);
  const [fatigueError, setFatigueError] = useState<string | null>(null);

  // ── Research packet state (Track O) ───────────────────────────────────────
  const [research, setResearch] = useState<ResearchAttachment[]>([]);
  const [researchLoading, setResearchLoading] = useState(false);
  const [researchError, setResearchError] = useState<string | null>(null);
  const [selectedAttId, setSelectedAttId] = useState<string | null>(null);
  const [pages, setPages] = useState<ExtractedPage[]>([]);
  const [pagesLoading, setPagesLoading] = useState(false);
  const [attachBusy, setAttachBusy] = useState(false);
  const fileInputRef = React.useRef<HTMLInputElement>(null);

  // Auto-populate reference from active document
  useEffect(() => {
    if (activeDocument?.scripture) {
      const ref = activeDocument.scripture.split('–')[0].split('—')[0].trim();
      setReferenceInput(ref);
    }
  }, [activeDocument?.id]);

  const lookupPassage = useCallback(async (ref?: string) => {
    const r = ref || referenceInput;
    if (!r.trim()) return;
    setPassageLoading(true);
    setPassageError(null);
    try {
      const result = await backend.getPassage(r.trim());
      setPassage(result);
    } catch {
      setPassageError('Failed to load passage. Check reference format.');
    } finally {
      setPassageLoading(false);
    }
  }, [backend, referenceInput]);

  const lookupStrongs = useCallback(async (id?: string) => {
    const s = id || strongsInput;
    if (!s.trim()) return;
    setStrongsLoading(true);
    setStrongsError(null);
    try {
      const entry = await backend.getStrongs(s.trim());
      setStrongs(entry);
    } catch {
      setStrongsError('Strong\'s ID not found.');
    } finally {
      setStrongsLoading(false);
    }
  }, [backend, strongsInput]);

  const lookupXrefs = useCallback(async (ref?: string) => {
    const r = ref || referenceInput;
    if (!r.trim()) return;
    setXrefLoading(true);
    setXrefError(null);
    try {
      const refs = await backend.getCrossReferences(r.trim());
      setXrefs(refs);
    } catch {
      setXrefError('Failed to load cross references.');
    } finally {
      setXrefLoading(false);
    }
  }, [backend, referenceInput]);

  const lookupHistory = useCallback(async (ref?: string) => {
    const r = ref || referenceInput;
    if (!r.trim()) return;
    setPreachedLoading(true);
    setPreachedError(null);
    try {
      const hist = await backend.getPreachedOn(r.trim());
      setPreached(hist);
    } catch {
      setPreachedError('Failed to load preaching history.');
    } finally {
      setPreachedLoading(false);
    }
  }, [backend, referenceInput]);

  const loadFatigue = useCallback(async () => {
    setFatigueLoading(true);
    setFatigueError(null);
    try {
      const data = await backend.getIllustrationFatigue();
      setFatigue(data);
    } catch {
      setFatigueError('Failed to load illustration fatigue data.');
    } finally {
      setFatigueLoading(false);
    }
  }, [backend]);

  const handleTabChange = (tab: StudyTab) => {
    setActiveTab(tab);
    if (tab === 'passage') lookupPassage();
    if (tab === 'strongs') lookupStrongs();
    if (tab === 'xref') lookupXrefs();
    if (tab === 'history') lookupHistory();
    if (tab === 'fatigue') loadFatigue();
    if (tab === 'research') loadResearch();
  };

  // ── Research packet handlers (Track O) ────────────────────────────────────

  const loadResearch = useCallback(async () => {
    if (!activeDocument?.id) return;
    setResearchLoading(true);
    setResearchError(null);
    try {
      const list = await backend.listResearchAttachments(activeDocument.id);
      setResearch(list);
    } catch {
      setResearchError('Failed to load research packet.');
    } finally {
      setResearchLoading(false);
    }
  }, [backend, activeDocument?.id]);

  // Refresh the packet when the sermon changes or the tab is active on mount.
  useEffect(() => {
    if (activeTab === 'research') {
      setSelectedAttId(null);
      setPages([]);
      loadResearch();
    }
  }, [activeTab, activeDocument?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const selectAttachment = async (att: ResearchAttachment) => {
    if (!activeDocument?.id) return;
    setSelectedAttId(att.id);
    setPages([]);
    setPagesLoading(true);
    try {
      const extracted = await backend.getExtractedPages(activeDocument.id, att.id);
      setPages(extracted);
    } catch {
      setResearchError('Failed to load extracted pages.');
    } finally {
      setPagesLoading(false);
    }
  };

  const handleAttach = async (file: File) => {
    if (!activeDocument?.id) return;
    setAttachBusy(true);
    setResearchError(null);
    try {
      await backend.attachResearchFile({
        sermonId: activeDocument.id,
        // In the browser mock the name is the transport handle; under Tauri the
        // file dialog supplies the absolute path before this call is made.
        sourcePath: (file as File & { path?: string }).path ?? file.name,
        title: file.name,
      });
      await loadResearch();
    } catch {
      setResearchError('Import failed. PDFs up to 50 MB are supported.');
    } finally {
      setAttachBusy(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  const handleRemove = async (att: ResearchAttachment) => {
    if (!activeDocument?.id) return;
    try {
      await backend.removeResearchAttachment(activeDocument.id, att.id);
      if (selectedAttId === att.id) {
        setSelectedAttId(null);
        setPages([]);
      }
      await loadResearch();
    } catch {
      setResearchError('Failed to remove attachment.');
    }
  };

  const handleSaveNotes = async (att: ResearchAttachment, notes: string) => {
    if (!activeDocument?.id) return;
    try {
      const updated = await backend.updateResearchMetadata(activeDocument.id, att.id, {
        userNotes: notes,
      });
      setResearch((prev) => prev.map((a) => (a.id === att.id ? updated : a)));
    } catch {
      setResearchError('Failed to save notes.');
    }
  };

  const handleOpenFile = async (att: ResearchAttachment) => {
    if (!activeDocument?.id) return;
    try {
      await backend.openResearchFile(activeDocument.id, att.id);
    } catch {
      setResearchError('Failed to open the stored file.');
    }
  };

  const handleReferenceSearch = () => {
    if (activeTab === 'passage') lookupPassage();
    else if (activeTab === 'xref') lookupXrefs();
    else if (activeTab === 'history') lookupHistory();
    else lookupPassage();
  };

  const FATIGUE_COLORS: Record<string, string> = {
    high: 'text-alert-red',
    medium: 'text-warn-amber',
    low: 'text-ok-green',
  };

  return (
    <div className="flex flex-col flex-shrink-0 border-l border-border bg-panel overflow-hidden" style={{ width: 280 }}>
      {/* Header */}
      <div className="panel-header">
        <span className="text-2xs font-600 text-fg-dim uppercase tracking-widest font-mono-data">
          {TAB_CONFIG.find((t) => t.id === activeTab)?.title || 'Study Rail'}
        </span>
        <button onClick={handleReferenceSearch} className="btn-ghost py-0.5 px-1.5 text-2xs">
          <RefreshCw size={10} />
        </button>
      </div>

      {/* Reference input (shared for passage/xref/history) */}
      {activeTab !== 'fatigue' && activeTab !== 'strongs' && activeTab !== 'insights' && activeTab !== 'research' && (
        <div className="px-3 py-2 border-b border-border flex-shrink-0">
          <div className="flex gap-1.5">
            <input
              type="text"
              value={referenceInput}
              onChange={(e) => setReferenceInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleReferenceSearch(); }}
              placeholder="e.g. John 6:35"
              className="input-field text-xs font-mono-data flex-1"
            />
            <button onClick={handleReferenceSearch} className="btn-ghost py-1.5 px-2">
              <Search size={11} />
            </button>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="flex border-b border-border flex-shrink-0">
        {TAB_CONFIG.map((tab) => {
          const TabIcon = tab.icon;
          return (
            <button
              key={`study-tab-${tab.id}`}
              onClick={() => handleTabChange(tab.id)}
              className={[
                'flex-1 flex items-center justify-center gap-0.5 py-2 text-2xs font-600 transition-colors duration-100',
                activeTab === tab.id
                  ? 'text-accent border-b-2 border-accent -mb-px' :'text-fg-dim hover:text-fg',
              ].join(' ')}
              title={tab.title}
            >
              <TabIcon size={10} />
              <span className="hidden sm:inline">{tab.label}</span>
            </button>
          );
        })}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto">

        {/* ── Passage tab ── */}
        {activeTab === 'passage' && (
          <div className="p-3">
            {passageLoading && (
              <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
                <Loader2 size={14} className="animate-spin-slow" />
                <span className="text-xs">Loading passage…</span>
              </div>
            )}
            {passageError && (
              <div className="py-4 text-center">
                <AlertTriangle size={16} className="text-alert-red mx-auto mb-2" />
                <p className="text-xs text-alert-red mb-2">{passageError}</p>
                <button onClick={() => lookupPassage()} className="btn-ghost text-xs">
                  <RefreshCw size={11} /> Retry
                </button>
              </div>
            )}
            {passage && !passageLoading && (
              <div className="fade-in">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-600 text-accent font-mono-data">{passage.reference}</span>
                  <div className="flex items-center gap-1">
                    <span className="text-2xs font-mono-data bg-elevated text-fg-dim px-1.5 py-0.5 rounded border border-border">
                      {passage.translation}
                    </span>
                    <CopyButton text={passage.text} label="Ref" />
                  </div>
                </div>
                <div className="space-y-2 mb-3">
                  {passage.verses.map((v) => (
                    <div key={`verse-${passage.reference}-${v.verse}`} className="flex gap-2">
                      <span className="text-2xs font-mono-data text-fg-dim mt-0.5 w-4 flex-shrink-0">{v.verse}</span>
                      <p className="text-sm leading-relaxed text-fg font-editor">{v.text}</p>
                    </div>
                  ))}
                </div>
                <div className="flex items-center gap-1.5 flex-wrap">
                  <CopyButton text={`${passage.reference} — ${passage.text}`} label="Copy" />
                  <InsertButton text={`${passage.reference}: ${passage.text}`} label="Insert" />
                  <button
                    onClick={() => { setActiveTab('xref'); lookupXrefs(); }}
                    className="btn-ghost py-0.5 px-1.5 text-2xs"
                  >
                    <Link2 size={10} /> X-Refs
                  </button>
                </div>
                {passage.osisRef && (
                  <p className="text-2xs font-mono-data text-fg-dim/50 mt-2">OSIS: {passage.osisRef}</p>
                )}
              </div>
            )}
            {!passage && !passageLoading && !passageError && (
              <div className="py-8 text-center">
                <Book size={20} className="text-fg-dim mx-auto mb-2" />
                <p className="text-xs text-fg-dim">Enter a reference above and press Enter</p>
              </div>
            )}
          </div>
        )}

        {/* ── Strong's tab ── */}
        {activeTab === 'strongs' && (
          <div className="p-3">
            <div className="flex gap-1.5 mb-3">
              <input
                type="text"
                value={strongsInput}
                onChange={(e) => setStrongsInput(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') lookupStrongs(); }}
                placeholder="e.g. G740 or H1234"
                className="input-field text-xs font-mono-data flex-1"
              />
              <button onClick={() => lookupStrongs()} className="btn-ghost py-1.5 px-2">
                <Search size={11} />
              </button>
            </div>
            {strongsLoading && (
              <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
                <Loader2 size={14} className="animate-spin-slow" />
                <span className="text-xs">Looking up…</span>
              </div>
            )}
            {strongsError && (
              <div className="py-4 text-center">
                <AlertTriangle size={16} className="text-alert-red mx-auto mb-2" />
                <p className="text-xs text-alert-red">{strongsError}</p>
              </div>
            )}
            {strongs && !strongsLoading && (
              <div className="fade-in space-y-3">
                <div className="flex items-start justify-between">
                  <div>
                    <span className="text-xs font-mono-data font-600 text-accent">{strongs.id}</span>
                    <span className="mx-2 text-fg-dim text-xs">·</span>
                    <span className="text-2xl font-600 text-gold-light font-editor">{strongs.lemma}</span>
                  </div>
                  <CopyButton text={`${strongs.id} ${strongs.lemma} (${strongs.transliteration}) — ${strongs.gloss}`} />
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <p className="text-2xs font-mono-data text-fg-dim mb-0.5">Transliteration</p>
                    <p className="text-sm font-mono-data text-fg italic">{strongs.transliteration}</p>
                  </div>
                  <div>
                    <p className="text-2xs font-mono-data text-fg-dim mb-0.5">Gloss</p>
                    <p className="text-sm font-600 text-fg">{strongs.gloss}</p>
                  </div>
                </div>
                <div>
                  <p className="text-2xs font-mono-data text-fg-dim mb-0.5">Part of Speech</p>
                  <p className="text-xs text-fg/80 font-mono-data">{strongs.partOfSpeech}</p>
                </div>
                <div>
                  <p className="text-2xs font-mono-data text-fg-dim mb-0.5">Definition</p>
                  <p className="text-xs text-fg/80 leading-relaxed">{strongs.definition}</p>
                </div>
                <div className="bg-elevated rounded px-3 py-2 border border-border">
                  <p className="text-2xs font-mono-data text-fg-dim">
                    Occurs <span className="text-fg font-600">{strongs.occurrences}×</span> in NT
                  </p>
                </div>
                {strongs.usageExamples && strongs.usageExamples.length > 0 && (
                  <div>
                    <p className="text-2xs font-mono-data text-fg-dim mb-1.5">Usage Examples</p>
                    <div className="space-y-2">
                      {strongs.usageExamples.map((ex, i) => (
                        <div key={`ex-${i}`} className="border-l-2 border-accent/30 pl-2">
                          <p className="text-2xs font-mono-data text-accent mb-0.5">{ex.reference}</p>
                          <p className="text-xs text-fg/70 italic">{ex.text}</p>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                {strongs.relatedIds && strongs.relatedIds.length > 0 && (
                  <div>
                    <p className="text-2xs font-mono-data text-fg-dim mb-1">Related</p>
                    <div className="flex flex-wrap gap-1">
                      {strongs.relatedIds.map((id) => (
                        <button
                          key={`related-${id}`}
                          onClick={() => { setStrongsInput(id); lookupStrongs(id); }}
                          className="text-2xs font-mono-data text-accent bg-accent/10 px-1.5 py-0.5 rounded border border-accent/20 hover:bg-accent/20 transition-colors"
                        >
                          {id}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                <InsertButton
                  text={`${strongs.id} ${strongs.lemma} (${strongs.transliteration}) — "${strongs.gloss}" — ${strongs.definition}`}
                  label="Insert as note"
                />
              </div>
            )}
            {!strongs && !strongsLoading && !strongsError && (
              <div className="py-8 text-center">
                <Hash size={20} className="text-fg-dim mx-auto mb-2" />
                <p className="text-xs text-fg-dim">Enter a Strong's ID (e.g. G740)</p>
              </div>
            )}
          </div>
        )}

        {/* ── Cross References tab ── */}
        {activeTab === 'xref' && (
          <div className="p-3">
            {xrefLoading && (
              <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
                <Loader2 size={14} className="animate-spin-slow" />
                <span className="text-xs">Loading cross references…</span>
              </div>
            )}
            {xrefError && (
              <div className="py-4 text-center">
                <AlertTriangle size={16} className="text-alert-red mx-auto mb-2" />
                <p className="text-xs text-alert-red mb-2">{xrefError}</p>
                <button onClick={() => lookupXrefs()} className="btn-ghost text-xs">
                  <RefreshCw size={11} /> Retry
                </button>
              </div>
            )}
            {xrefs.length > 0 && !xrefLoading && (
              <div className="fade-in space-y-2">
                <p className="text-2xs font-mono-data text-fg-dim mb-2">
                  {xrefs.length} references for <span className="text-accent">{referenceInput}</span>
                </p>
                {xrefs.map((xref, i) => (
                  <div key={`xref-${i}`} className="border border-border rounded p-2.5 hover:border-accent/40 transition-colors group">
                    <div className="flex items-center justify-between mb-1">
                      <button
                        onClick={() => { setReferenceInput(xref.reference); lookupPassage(xref.reference); setActiveTab('passage'); }}
                        className="text-xs font-mono-data font-600 text-accent hover:text-gold-light transition-colors"
                      >
                        {xref.reference}
                      </button>
                      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                        <CopyButton text={`${xref.reference} — ${xref.snippet}`} />
                        <InsertButton text={`${xref.reference}: ${xref.snippet}`} label="Insert" />
                      </div>
                    </div>
                    <p className="text-xs text-fg/70 italic leading-relaxed">{xref.snippet}</p>
                    <div className="flex items-center justify-between mt-1.5">
                      {xref.category && (
                        <span className="text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded">{xref.category}</span>
                      )}
                      <div className="flex items-center gap-1 ml-auto">
                        <div className="h-1 rounded-full bg-accent/20" style={{ width: 40 }}>
                          <div className="h-1 rounded-full bg-accent" style={{ width: `${xref.relevance * 100}%` }} />
                        </div>
                        <span className="text-2xs font-mono-data text-fg-dim">{Math.round(xref.relevance * 100)}%</span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
            {xrefs.length === 0 && !xrefLoading && !xrefError && (
              <div className="py-8 text-center">
                <Link2 size={20} className="text-fg-dim mx-auto mb-2" />
                <p className="text-xs text-fg-dim">Enter a reference above to find cross references</p>
              </div>
            )}
          </div>
        )}

        {/* ── History tab ── */}
        {activeTab === 'history' && (
          <div className="p-3">
            {preachedLoading && (
              <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
                <Loader2 size={14} className="animate-spin-slow" />
                <span className="text-xs">Loading history…</span>
              </div>
            )}
            {preachedError && (
              <div className="py-4 text-center">
                <AlertTriangle size={16} className="text-alert-red mx-auto mb-2" />
                <p className="text-xs text-alert-red mb-2">{preachedError}</p>
                <button onClick={() => lookupHistory()} className="btn-ghost text-xs">
                  <RefreshCw size={11} /> Retry
                </button>
              </div>
            )}
            {preached.length > 0 && !preachedLoading && (
              <div className="fade-in space-y-2">
                <p className="text-2xs font-mono-data text-fg-dim mb-2">
                  Preached <span className="text-fg">{preached.length}×</span> on <span className="text-accent">{referenceInput}</span>
                </p>
                {preached.map((p, i) => (
                  <div key={`hist-${i}`} className="border border-border rounded p-2.5">
                    <p className="text-xs font-600 text-fg mb-0.5">{p.sermonTitle}</p>
                    <div className="flex items-center gap-2 text-2xs font-mono-data text-fg-dim">
                      <span className="text-accent">{p.preachedOn}</span>
                      {p.series && <span>· {p.series}</span>}
                      {p.wordCount && <span>· {p.wordCount.toLocaleString()}w</span>}
                    </div>
                  </div>
                ))}
              </div>
            )}
            {preached.length === 0 && !preachedLoading && !preachedError && (
              <div className="py-8 text-center">
                <Clock size={20} className="text-fg-dim mx-auto mb-2" />
                <p className="text-xs text-fg-dim">No preaching history found for this reference</p>
              </div>
            )}
          </div>
        )}

        {/* ── Illustration Fatigue tab ── */}
        {activeTab === 'fatigue' && (
          <div className="p-3">
            {fatigueLoading && (
              <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
                <Loader2 size={14} className="animate-spin-slow" />
                <span className="text-xs">Loading fatigue data…</span>
              </div>
            )}
            {fatigueError && (
              <div className="py-4 text-center">
                <AlertTriangle size={16} className="text-alert-red mx-auto mb-2" />
                <p className="text-xs text-alert-red mb-2">{fatigueError}</p>
                <button onClick={loadFatigue} className="btn-ghost text-xs">
                  <RefreshCw size={11} /> Retry
                </button>
              </div>
            )}
            {fatigue.length > 0 && !fatigueLoading && (
              <div className="fade-in space-y-2">
                <p className="text-2xs font-mono-data text-fg-dim mb-2">Illustrations used recently</p>
                {fatigue.map((item, i) => (
                  <div key={`fatigue-${i}`} className="border border-border rounded p-2.5">
                    <div className="flex items-start justify-between mb-1">
                      <p className="text-xs font-600 text-fg capitalize">{item.illustration}</p>
                      <span className={`text-2xs font-mono-data font-600 ${FATIGUE_COLORS[item.severity]}`}>
                        {item.severity.toUpperCase()}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-2xs font-mono-data text-fg-dim mb-1">
                      <span>Used <span className="text-fg">{item.useCount}×</span></span>
                      <span>· Last: {item.lastUsedOn}</span>
                    </div>
                    <p className="text-2xs text-fg-dim">In: {item.lastUsedIn}</p>
                    <div className="mt-1.5 h-1 rounded-full bg-elevated overflow-hidden">
                      <div
                        className={`h-1 rounded-full ${item.severity === 'high' ? 'bg-alert-red' : item.severity === 'medium' ? 'bg-warn-amber' : 'bg-ok-green'}`}
                        style={{ width: `${Math.min(100, item.useCount * 20)}%` }}
                      />
                    </div>
                  </div>
                ))}
              </div>
            )}
            {fatigue.length === 0 && !fatigueLoading && !fatigueError && (
              <div className="py-8 text-center">
                <BarChart2 size={20} className="text-fg-dim mx-auto mb-2" />
                <p className="text-xs text-fg-dim">No illustration fatigue data</p>
                <button onClick={loadFatigue} className="text-xs text-accent hover:text-gold-light mt-2 transition-colors">
                  Load data
                </button>
              </div>
            )}
          </div>
        )}

        {/* ── Research Packet tab (Track O) ── */}
        {activeTab === 'research' && (
          <div className="p-3">
            <input
              ref={fileInputRef}
              type="file"
              accept="application/pdf"
              className="hidden"
              data-testid="research-file-input"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) handleAttach(f);
              }}
            />
            <div className="flex items-center justify-between mb-2">
              <span className="text-2xs font-mono-data uppercase tracking-widest text-fg-dim">
                Research Packet
              </span>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => fileInputRef.current?.click()}
                  disabled={attachBusy || !activeDocument}
                  className="btn-ghost py-0.5 px-1.5 text-2xs"
                  title="Attach a PDF"
                >
                  {attachBusy ? <Loader2 size={10} className="animate-spin-slow" /> : <Plus size={10} />}
                  <span>Attach PDF</span>
                </button>
                <button onClick={loadResearch} className="btn-ghost py-0.5 px-1.5 text-2xs" title="Refresh">
                  <RefreshCw size={10} />
                </button>
              </div>
            </div>

            {!activeDocument && (
              <p className="text-xs text-fg-dim py-6 text-center">
                Open a sermon to manage its research packet.
              </p>
            )}

            {activeDocument && researchLoading && (
              <div className="flex items-center gap-2 text-fg-dim py-6 justify-center">
                <Loader2 size={14} className="animate-spin-slow" />
                <span className="text-xs">Loading packet…</span>
              </div>
            )}

            {activeDocument && researchError && (
              <div className="py-3 text-center">
                <AlertTriangle size={14} className="text-alert-red mx-auto mb-1" />
                <p className="text-2xs text-alert-red mb-1">{researchError}</p>
                <button onClick={loadResearch} className="text-2xs text-accent transition-colors">
                  Retry
                </button>
              </div>
            )}

            {activeDocument && !researchLoading && !researchError && research.length === 0 && (
              <p className="text-xs text-fg-dim py-6 text-center">
                No research attached. PDFs stay in the packet — never in the archive index.
              </p>
            )}

            {activeDocument && research.map((att) => (
              <div key={att.id} className="border border-border rounded-md mb-2 overflow-hidden">
                <div className="px-2 py-1.5 flex items-start justify-between gap-1">
                  <button
                    onClick={() => selectAttachment(att)}
                    className="text-left flex-1 min-w-0"
                    title={att.originalFilename}
                  >
                    <p className="text-xs font-600 text-fg truncate">{att.title || att.originalFilename}</p>
                    <p className="text-2xs font-mono-data text-fg-dim">
                      {att.extraction === 'ok'
                        ? `${att.pageCount ?? '?'}p · extracted`
                        : att.extraction === 'no-text'
                          ? 'no extractable text'
                          : att.extraction === 'failed'
                            ? 'extraction failed'
                            : 'not extracted'}
                    </p>
                  </button>
                  <div className="flex items-center gap-0.5 flex-shrink-0">
                    <button onClick={() => handleOpenFile(att)} className="btn-ghost py-0.5 px-1 text-2xs" title="Open stored PDF">
                      <FileText size={10} />
                    </button>
                    <button onClick={() => handleRemove(att)} className="btn-ghost py-0.5 px-1 text-2xs" title="Remove">
                      <X size={10} />
                    </button>
                  </div>
                </div>
                <div className="px-2 pb-1">
                  <span className="text-2xs font-mono-data uppercase tracking-widest text-accent">
                    research packet
                  </span>
                </div>

                {selectedAttId === att.id && (
                  <div className="border-t border-border px-2 py-2">
                    {pagesLoading && (
                      <div className="flex items-center gap-2 text-fg-dim py-3 justify-center">
                        <Loader2 size={12} className="animate-spin-slow" />
                        <span className="text-2xs">Extracting…</span>
                      </div>
                    )}
                    {!pagesLoading && pages.length === 0 && (
                      <p className="text-2xs text-fg-dim py-2">
                        {att.extraction === 'no-text'
                          ? 'This PDF contains no extractable text.'
                          : 'No extracted pages available.'}
                      </p>
                    )}
                    {!pagesLoading && pages.map((p) => (
                      <div key={p.page} className="mb-2">
                        <p className="text-2xs font-mono-data text-fg-dim mb-0.5">Page {p.page}</p>
                        {/* Inert text rendering: extracted content is never HTML. */}
                        <pre className="text-2xs text-fg whitespace-pre-wrap font-body max-h-40 overflow-y-auto bg-elevated rounded p-1.5">
                          {p.text}
                        </pre>
                        <div className="flex gap-1 mt-0.5">
                          <CopyButton text={p.text} label="Copy" />
                          <InsertButton text={p.text} label="Insert" />
                        </div>
                      </div>
                    ))}
                    <textarea
                      defaultValue={att.userNotes ?? ''}
                      placeholder="Notes about this source…"
                      className="input-field text-2xs w-full mt-1 h-14 resize-y"
                      onBlur={(e) => {
                        if (e.target.value !== (att.userNotes ?? '')) {
                          handleSaveNotes(att, e.target.value);
                        }
                      }}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {/* ── Sermon Intelligence tab (Track L presentation) ── */}
        {activeTab === 'insights' && <InsightsPanel />}
      </div>
    </div>
  );
}