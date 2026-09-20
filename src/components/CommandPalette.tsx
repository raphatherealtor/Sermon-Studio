'use client';
import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useRouter } from 'next/navigation';
import { useBackend } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import {
  Search, Plus, Save, FileOutput, AlertTriangle, Archive,
  Settings, Code2, BookOpen, Hash, Link2, X, Command,
  Keyboard, RefreshCw, PanelLeft, PanelRight, ChevronRight,
} from 'lucide-react';

interface CommandItem {
  id: string;
  label: string;
  description?: string;
  shortcut?: string;
  icon: React.ElementType;
  category: string;
  action: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
}

export default function CommandPalette({ open, onClose }: CommandPaletteProps) {
  const router = useRouter();
  const backend = useBackend();
  const { activeDocument, setDocument, setActiveSermon, setLintFindings, setLinting } = useEditorStore();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const runLint = useCallback(async () => {
    if (!activeDocument) return;
    onClose();
    setLinting(true);
    try {
      const findings = await backend.lintSermon(activeDocument);
      setLintFindings(findings);
    } finally {
      setLinting(false);
    }
  }, [backend, activeDocument, onClose, setLinting, setLintFindings]);

  const createNewSermon = useCallback(async () => {
    onClose();
    const doc = await backend.createSermon({ title: 'Untitled Sermon' });
    setDocument(doc);
    setActiveSermon(doc.id);
    router.push('/');
  }, [backend, onClose, setDocument, setActiveSermon, router]);

  const saveSermon = useCallback(async () => {
    if (!activeDocument) return;
    onClose();
    await backend.saveSermon(activeDocument);
  }, [backend, activeDocument, onClose]);

  const COMMANDS: CommandItem[] = [
    // Sermon
    { id: 'new-sermon', label: 'New Sermon', description: 'Create a new sermon document', shortcut: '⌘N', icon: Plus, category: 'Sermon', action: createNewSermon },
    { id: 'save', label: 'Save', description: 'Save the current sermon', shortcut: '⌘S', icon: Save, category: 'Sermon', action: saveSermon },
    { id: 'save-as', label: 'Save As…', description: 'Save sermon to a new location', shortcut: '⌘⇧S', icon: Save, category: 'Sermon', action: () => { onClose(); } },
    { id: 'run-lint', label: 'Run Linter', description: 'Analyze current sermon for structural issues', shortcut: '⌘⇧L', icon: AlertTriangle, category: 'Sermon', action: runLint },
    // Navigation
    { id: 'go-editor', label: 'Go to Editor', description: 'Open the Preacher\'s Desk workspace', icon: BookOpen, category: 'Navigation', action: () => { onClose(); router.push('/'); } },
    { id: 'go-archive', label: 'Search Archive', description: 'Focus archive search (⌘K)', shortcut: '⌘K', icon: Archive, category: 'Navigation', action: () => { onClose(); router.push('/archive-overview'); } },
    { id: 'go-export', label: 'Export Sermon', description: 'Open the export configurator', shortcut: '⌘⇧E', icon: FileOutput, category: 'Navigation', action: () => { onClose(); router.push('/export-screen'); } },
    { id: 'go-settings', label: 'Settings', description: 'Open application settings', shortcut: '⌘,', icon: Settings, category: 'Navigation', action: () => { onClose(); router.push('/settings-index-management'); } },
    { id: 'go-codec', label: 'Codec Test', description: 'Open directive transport codec test', icon: Code2, category: 'Navigation', action: () => { onClose(); router.push('/codec-test'); } },
    // Study
    { id: 'go-passage', label: 'Go to Passage', description: 'Look up a scripture passage in the Study Rail', icon: BookOpen, category: 'Study', action: () => { onClose(); router.push('/'); } },
    { id: 'show-strongs', label: "Show Strong's", description: 'Look up a Strong\'s lexicon entry', icon: Hash, category: 'Study', action: () => { onClose(); router.push('/'); } },
    { id: 'show-xref', label: 'Show Cross References', description: 'Find cross references for current passage', icon: Link2, category: 'Study', action: () => { onClose(); router.push('/'); } },
    // Index
    { id: 'rebuild-index', label: 'Rebuild Index', description: 'Rebuild the sermon search index from scratch', icon: RefreshCw, category: 'Index', action: async () => { onClose(); await backend.rebuildIndex(); } },
    { id: 'rescan-library', label: 'Rescan Library', description: 'Scan library directory for new/changed files', icon: RefreshCw, category: 'Index', action: async () => { onClose(); await backend.rescanLibrary(); } },
    // View
    { id: 'toggle-archive-rail', label: 'Toggle Archive Rail', description: 'Show or hide the left archive panel', shortcut: '⌘[', icon: PanelLeft, category: 'View', action: () => { onClose(); } },
    { id: 'toggle-study-rail', label: 'Toggle Study Rail', description: 'Show or hide the right study panel', shortcut: '⌘]', icon: PanelRight, category: 'View', action: () => { onClose(); } },
    // Help
    { id: 'keyboard-shortcuts', label: 'Keyboard Shortcuts', description: 'View all keyboard shortcuts', shortcut: '⌘?', icon: Keyboard, category: 'Help', action: () => { onClose(); router.push('/settings-index-management'); } },
  ];

  const filtered = query.trim()
    ? COMMANDS.filter(
        (c) =>
          c.label.toLowerCase().includes(query.toLowerCase()) ||
          (c.description?.toLowerCase().includes(query.toLowerCase()) ?? false) ||
          c.category.toLowerCase().includes(query.toLowerCase())
      )
    : COMMANDS;

  // Group by category
  const grouped = filtered.reduce<Record<string, CommandItem[]>>((acc, cmd) => {
    if (!acc[cmd.category]) acc[cmd.category] = [];
    acc[cmd.category].push(cmd);
    return acc;
  }, {});

  const flatFiltered = filtered;

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, flatFiltered.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (flatFiltered[selectedIndex]) {
        flatFiltered[selectedIndex].action();
      }
    } else if (e.key === 'Escape') {
      onClose();
    }
  };

  if (!open) return null;

  let globalIndex = 0;

  return (
    <div className="command-overlay" onClick={onClose}>
      <div className="command-palette" onClick={(e) => e.stopPropagation()}>
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border">
          <Search size={15} className="text-fg-dim flex-shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a command or search…"
            className="flex-1 bg-transparent text-fg text-sm outline-none placeholder:text-fg-dim"
          />
          <div className="flex items-center gap-1.5">
            <kbd className="text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded border border-border">ESC</kbd>
            <button onClick={onClose} className="text-fg-dim hover:text-fg">
              <X size={14} />
            </button>
          </div>
        </div>

        {/* Results */}
        <div ref={listRef} className="max-h-96 overflow-y-auto py-1">
          {flatFiltered.length === 0 ? (
            <div className="px-4 py-8 text-center">
              <p className="text-sm text-fg-dim">No commands found for &quot;{query}&quot;</p>
            </div>
          ) : (
            Object.entries(grouped).map(([category, items]) => (
              <div key={`cat-${category}`}>
                <div className="px-4 py-1.5">
                  <span className="text-2xs font-mono-data uppercase tracking-wider text-fg-dim">{category}</span>
                </div>
                {items.map((cmd) => {
                  const isSelected = globalIndex === selectedIndex;
                  const currentIndex = globalIndex++;
                  const CmdIcon = cmd.icon;
                  return (
                    <button
                      key={`cmd-${cmd.id}`}
                      onClick={cmd.action}
                      onMouseEnter={() => setSelectedIndex(currentIndex)}
                      className={[
                        'w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors duration-75',
                        isSelected ? 'bg-accent/10' : 'hover:bg-elevated',
                      ].join(' ')}
                    >
                      <CmdIcon size={14} className={isSelected ? 'text-accent' : 'text-fg-dim'} />
                      <div className="flex-1 min-w-0">
                        <p className={`text-sm font-500 ${isSelected ? 'text-accent' : 'text-fg'}`}>{cmd.label}</p>
                        {cmd.description && (
                          <p className="text-xs text-fg-dim truncate">{cmd.description}</p>
                        )}
                      </div>
                      {cmd.shortcut && (
                        <kbd className="text-2xs font-mono-data text-fg-dim bg-elevated px-1.5 py-0.5 rounded border border-border flex-shrink-0">
                          {cmd.shortcut}
                        </kbd>
                      )}
                      {isSelected && <ChevronRight size={12} className="text-accent flex-shrink-0" />}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-border px-4 py-2 flex items-center gap-3 text-2xs font-mono-data text-fg-dim">
          <span className="flex items-center gap-1"><kbd className="bg-elevated px-1 py-0.5 rounded border border-border">↑↓</kbd> navigate</span>
          <span className="flex items-center gap-1"><kbd className="bg-elevated px-1 py-0.5 rounded border border-border">↵</kbd> select</span>
          <span className="flex items-center gap-1"><kbd className="bg-elevated px-1 py-0.5 rounded border border-border">ESC</kbd> close</span>
          <span className="ml-auto flex items-center gap-1">
            <Command size={10} />
            <span>⌘P to open</span>
          </span>
        </div>
      </div>
    </div>
  );
}
