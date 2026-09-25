'use client';
import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import { openSermon as openSermonAction, switchToSermon } from '@/lib/store/sermonOpenWorkflow';
import { Search, Plus, X, BookOpen, ChevronRight, Pin, PinOff, Copy, Archive, Trash2, Edit3, AlertTriangle, Loader2, RefreshCw, Filter, FileQuestion, HardDrive, GitMerge, MoreHorizontal,  } from 'lucide-react';
import type { SermonSummary, SearchResult, FilesystemState } from '@/lib/backend/types';

const STATUS_CLASSES: Record<string, string> = {
  draft: 'badge-draft',
  'in-progress': 'badge-progress',
  reviewed: 'badge-reviewed',
  preached: 'badge-preached',
  archived: 'badge-archived',
};

const STATUS_LABELS: Record<string, string> = {
  draft: 'Draft',
  'in-progress': 'WIP',
  reviewed: 'Ready',
  preached: 'Preached',
  archived: 'Archived',
};

function FsStateIcon({ state }: { state?: FilesystemState }) {
  if (!state || state === 'clean') return null;
  if (state === 'local-dirty') return <span title="Unsaved local changes" className="text-warn-amber"><AlertTriangle size={9} /></span>;
  if (state === 'disk-changed' || state === 'both-changed') return <span title="Conflict detected" className="text-alert-red"><GitMerge size={9} /></span>;
  if (state === 'missing') return <span title="File missing on disk" className="text-fg-dim"><FileQuestion size={9} /></span>;
  if (state === 'renamed-externally') return <span title="Renamed externally" className="text-warn-amber"><HardDrive size={9} /></span>;
  return null;
}

type FilterStatus = 'all' | 'draft' | 'in-progress' | 'reviewed' | 'preached' | 'archived';

interface ContextMenuState {
  sermonId: string;
  x: number;
  y: number;
}

interface ArchiveRailProps {
  focusSearch?: boolean;
  onSearchFocused?: () => void;
}

export default function ArchiveRail({ focusSearch, onSearchFocused }: ArchiveRailProps) {
  const backend = useBackend();
  const { activeSermonId, setActiveSermon, isDirty, conflictInfo } = useEditorStore();
  const [sermons, setSermons] = useState<SermonSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<SearchResult[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [creating, setCreating] = useState(false);
  const [statusFilter, setStatusFilter] = useState<FilterStatus>('all');
  const [showFilters, setShowFilters] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Fix 1: Focus archive search when Ctrl/Cmd+K is pressed
  useEffect(() => {
    if (focusSearch) {
      searchInputRef.current?.focus();
      onSearchFocused?.();
    }
  }, [focusSearch, onSearchFocused]);

  const loadSermons = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    else setRefreshing(true);
    try {
      const list = await backend.listSermons();
      setSermons(list);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [backend]);

  useEffect(() => { loadSermons(); }, [loadSermons]);

  useEffect(() => {
    if (!searchQuery.trim()) { setSearchResults(null); return; }
    const t = setTimeout(async () => {
      setSearching(true);
      const results = await backend.searchSermons(searchQuery);
      setSearchResults(results);
      setSearching(false);
    }, 350);
    return () => clearTimeout(t);
  }, [searchQuery, backend]);

  // Close context menu on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
        setContextMenu(null);
      }
    };
    if (contextMenu) document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [contextMenu]);

  const openSermon = useCallback(async (id: string) => {
    await openSermonAction(backend, id);
  }, [backend]);

  const createNew = useCallback(async () => {
    setCreating(true);
    try {
      if (await switchToSermon(backend, () => backend.createSermon({ title: 'Untitled Sermon', scripture: '' }))) {
        await loadSermons(true);
      }
    } finally {
      setCreating(false);
    }
  }, [backend, loadSermons]);

  const handlePin = async (id: string, pinned: boolean) => {
    setContextMenu(null);
    await backend.pinSermon(id, !pinned);
    await loadSermons(true);
  };

  const handleDuplicate = async (id: string) => {
    setContextMenu(null);
    if (await switchToSermon(backend, () => backend.duplicateSermon({ id }))) {
      await loadSermons(true);
    }
  };

  const handleArchive = async (id: string) => {
    setContextMenu(null);
    await backend.archiveSermon(id);
    await loadSermons(true);
  };

  const handleDelete = async (id: string) => {
    setContextMenu(null);
    if (!confirm('Delete this sermon? This cannot be undone.')) return;
    await backend.deleteSermon(id);
    if (activeSermonId === id) setActiveSermon(null);
    await loadSermons(true);
  };

  const startRename = (sermon: SermonSummary) => {
    setContextMenu(null);
    setRenamingId(sermon.id);
    setRenameValue(sermon.title);
  };

  const commitRename = async (id: string) => {
    if (!renameValue.trim()) { setRenamingId(null); return; }
    await backend.renameSermon({ id, newTitle: renameValue.trim() });
    setRenamingId(null);
    await loadSermons(true);
  };

  const openContextMenu = (e: React.MouseEvent, sermon: SermonSummary) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ sermonId: sermon.id, x: e.clientX, y: e.clientY });
  };

  // Filter and display
  let displayList = searchResults
    ? sermons.filter((s) => searchResults.some((r) => r.id === s.id))
    : sermons;

  if (statusFilter !== 'all') {
    displayList = displayList.filter((s) => s.status === statusFilter);
  }

  // Pinned first
  const pinned = displayList.filter((s) => s.isPinned);
  const unpinned = displayList.filter((s) => !s.isPinned);
  const orderedList = [...pinned, ...unpinned];

  const conflictCount = sermons.filter((s) => s.hasConflict).length;

  return (
    <div className="panel-rail w-60 flex-shrink-0 relative">
      {/* Header */}
      <div className="panel-header">
        <div className="flex items-center gap-1.5">
          <span className="text-2xs font-600 text-fg-dim uppercase tracking-widest font-mono-data">Archive</span>
          {refreshing && <Loader2 size={10} className="animate-spin-slow text-fg-dim" />}
          {conflictCount > 0 && (
            <span className="text-2xs font-mono-data text-alert-red bg-alert-red/10 px-1.5 py-0.5 rounded border border-alert-red/20">
              {conflictCount} conflict{conflictCount !== 1 ? 's' : ''}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button onClick={() => loadSermons(true)} className="btn-ghost py-1 px-1.5" title="Refresh">
            <RefreshCw size={11} />
          </button>
          <button onClick={createNew} disabled={creating} className="btn-ghost py-1 px-1.5" title="New sermon (Cmd+N)">
            {creating ? <Loader2 size={11} className="animate-spin-slow" /> : <Plus size={11} />}
          </button>
        </div>
      </div>

      {/* Search */}
      <div className="px-2.5 py-2 border-b border-border flex-shrink-0">
        <div className="relative">
          <Search size={11} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-dim" />
          <input
            ref={searchInputRef}
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search sermons… (⌘K)"
            className="input-field pl-7 pr-7 text-xs"
          />
          {searchQuery && (
            <button onClick={() => setSearchQuery('')} className="absolute right-2 top-1/2 -translate-y-1/2 text-fg-dim hover:text-fg">
              <X size={10} />
            </button>
          )}
        </div>
        {searching && <p className="text-2xs text-fg-dim mt-1 font-mono-data">Searching…</p>}
      </div>

      {/* Filter bar */}
      <div className="px-2.5 py-1.5 border-b border-border flex-shrink-0 flex items-center gap-1.5">
        <button
          onClick={() => setShowFilters((p) => !p)}
          className={`btn-ghost py-0.5 px-1.5 text-2xs ${showFilters ? 'text-accent' : ''}`}
        >
          <Filter size={10} />
          <span>Filter</span>
        </button>
        {statusFilter !== 'all' && (
          <span className="text-2xs font-mono-data text-accent bg-accent/10 px-1.5 py-0.5 rounded flex items-center gap-1">
            {statusFilter}
            <button onClick={() => setStatusFilter('all')}><X size={9} /></button>
          </span>
        )}
        <span className="ml-auto text-2xs font-mono-data text-fg-dim">{orderedList.length}</span>
      </div>

      {showFilters && (
        <div className="px-2.5 py-2 border-b border-border flex-shrink-0 flex flex-wrap gap-1">
          {(['all', 'draft', 'in-progress', 'reviewed', 'preached', 'archived'] as FilterStatus[]).map((s) => (
            <button
              key={`filter-${s}`}
              onClick={() => { setStatusFilter(s); setShowFilters(false); }}
              className={`text-2xs font-mono-data px-2 py-0.5 rounded border transition-colors ${
                statusFilter === s
                  ? 'bg-accent/15 text-accent border-accent/30' :'text-fg-dim border-border hover:border-fg-dim hover:text-fg'
              }`}
            >
              {s === 'all' ? 'All' : STATUS_LABELS[s] || s}
            </button>
          ))}
        </div>
      )}

      {/* List */}
      <div className="flex-1 overflow-y-auto py-0.5">
        {loading ? (
          <div className="p-2.5 flex flex-col gap-1.5">
            {Array.from({ length: 6 }).map((_, i) => (
              <div key={`skel-${i}`} className="animate-pulse bg-elevated rounded h-14" />
            ))}
          </div>
        ) : orderedList.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 p-6 text-center">
            <BookOpen size={22} className="text-fg-dim" />
            <p className="text-xs text-fg-dim">
              {searchQuery ? 'No results found' : 'No sermons yet'}
            </p>
            {!searchQuery && (
              <button onClick={createNew} className="text-xs text-accent hover:text-gold-light transition-colors">
                Create your first sermon
              </button>
            )}
          </div>
        ) : (
          <>
            {pinned.length > 0 && (
              <div className="px-2.5 pt-2 pb-0.5">
                <p className="text-2xs font-mono-data text-fg-dim uppercase tracking-wider flex items-center gap-1">
                  <Pin size={8} /> Pinned
                </p>
              </div>
            )}
            {orderedList.map((sermon, idx) => {
              const isActive = sermon.id === activeSermonId;
              const showPinnedDivider = idx === pinned.length && pinned.length > 0;
              return (
                <React.Fragment key={`archive-${sermon.id}`}>
                  {showPinnedDivider && (
                    <div className="px-2.5 pt-2 pb-0.5">
                      <p className="text-2xs font-mono-data text-fg-dim uppercase tracking-wider">Sermons</p>
                    </div>
                  )}
                  {renamingId === sermon.id ? (
                    <div className="px-2.5 py-2 border-b border-border/40">
                      <input
                        autoFocus
                        type="text"
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') commitRename(sermon.id);
                          if (e.key === 'Escape') setRenamingId(null);
                        }}
                        onBlur={() => commitRename(sermon.id)}
                        className="input-field text-xs w-full"
                      />
                    </div>
                  ) : (
                    <button
                      onClick={() => openSermon(sermon.id)}
                      onContextMenu={(e) => openContextMenu(e, sermon)}
                      className={[
                        'w-full text-left px-2.5 py-2 border-b border-border/30 transition-colors duration-100 group',
                        isActive ? 'bg-accent/8 border-l-2 border-l-accent' : 'hover:bg-elevated',
                      ].join(' ')}
                    >
                      <div className="flex items-start justify-between gap-1 mb-0.5">
                        <span className={`text-xs font-600 leading-tight line-clamp-1 flex-1 ${isActive ? 'text-accent' : 'text-fg'}`}>
                          {sermon.isPinned && <Pin size={8} className="inline mr-1 text-fg-dim" />}
                          {sermon.title}
                        </span>
                        <div className="flex items-center gap-0.5 flex-shrink-0 mt-0.5">
                          <FsStateIcon state={sermon.fsState} />
                          {isActive && <ChevronRight size={9} className="text-accent" />}
                          <div
                            role="button"
                            tabIndex={0}
                            onClick={(e) => { e.stopPropagation(); openContextMenu(e, sermon); }}
                            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); openContextMenu(e as unknown as React.MouseEvent<HTMLDivElement>, sermon); } }}
                            className="opacity-0 group-hover:opacity-100 text-fg-dim hover:text-fg transition-opacity cursor-pointer"
                          >
                            <MoreHorizontal size={11} />
                          </div>
                        </div>
                      </div>
                      <p className="text-2xs font-mono-data text-accent mb-1 line-clamp-1">{sermon.scripture}</p>
                      {sermon.series && (
                        <p className="text-2xs text-fg-dim line-clamp-1 mb-1">{sermon.series}</p>
                      )}
                      <div className="flex items-center justify-between">
                        <span className={`status-badge ${STATUS_CLASSES[sermon.status]}`}>
                          {STATUS_LABELS[sermon.status]}
                        </span>
                        <span className="text-2xs font-mono-data text-fg-dim">
                          {sermon.wordCount.toLocaleString()}w
                        </span>
                      </div>
                    </button>
                  )}
                </React.Fragment>
              );
            })}
          </>
        )}
      </div>

      {/* Footer */}
      <div className="border-t border-border flex-shrink-0">
        {isDirty && (
          <div className="px-2.5 py-1.5 bg-warn/8 flex items-center gap-1.5">
            <AlertTriangle size={10} className="text-warn-amber flex-shrink-0" />
            <p className="text-2xs font-mono-data text-warn-amber">Unsaved changes</p>
          </div>
        )}
        {conflictInfo && (
          <div className="px-2.5 py-1.5 bg-alert-red/8 flex items-center gap-1.5">
            <GitMerge size={10} className="text-alert-red flex-shrink-0" />
            <p className="text-2xs font-mono-data text-alert-red">Conflict pending</p>
          </div>
        )}
      </div>

      {/* Context menu */}
      {contextMenu && (() => {
        const sermon = sermons.find((s) => s.id === contextMenu.sermonId);
        if (!sermon) return null;
        return (
          <div
            ref={contextMenuRef}
            className="fixed z-50 bg-panel border border-border rounded shadow-xl py-1 min-w-44"
            style={{ left: Math.min(contextMenu.x, window.innerWidth - 180), top: Math.min(contextMenu.y, window.innerHeight - 200) }}
          >
            <button onClick={() => openSermon(sermon.id)} className="context-menu-item">
              <BookOpen size={12} /> Open in Editor
            </button>
            <button onClick={() => startRename(sermon)} className="context-menu-item">
              <Edit3 size={12} /> Rename
            </button>
            <button onClick={() => handleDuplicate(sermon.id)} className="context-menu-item">
              <Copy size={12} /> Duplicate
            </button>
            <button onClick={() => handlePin(sermon.id, !!sermon.isPinned)} className="context-menu-item">
              {sermon.isPinned ? <PinOff size={12} /> : <Pin size={12} />}
              {sermon.isPinned ? 'Unpin' : 'Pin'}
            </button>
            <div className="border-t border-border my-1" />
            <button onClick={() => handleArchive(sermon.id)} className="context-menu-item text-fg-dim">
              <Archive size={12} /> Archive
            </button>
            <button onClick={() => handleDelete(sermon.id)} className="context-menu-item text-alert-red">
              <Trash2 size={12} /> Delete
            </button>
          </div>
        );
      })()}
    </div>
  );
}
