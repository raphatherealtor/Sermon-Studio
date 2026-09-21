'use client';
import React, { createContext, useContext, ReactNode, useMemo } from 'react';
import type { SermonBackend } from './SermonBackend';
import { MockSermonBackend } from './MockSermonBackend';
import { TauriSermonBackend } from './TauriSermonBackend';
import { isTauriRuntime } from './runtime';

// ─────────────────────────────────────────────────────────────────────────────
// Composition root: the single place that decides which concrete backend the
// whole React tree uses.
//
//   * Native Tauri webview   -> TauriSermonBackend (real IPC to the Rust core)
//   * Browser / static preview -> MockSermonBackend (fixtures only)
//
// Components consume the abstraction exclusively via `useBackend()` and must
// never import TauriSermonBackend, MockSermonBackend, or @tauri-apps/api
// directly. This module is the only composition point.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Build the backend for the current runtime. Pure of component lifecycle and
 * testable in isolation.
 *
 * A detected native runtime always resolves to the Tauri backend — failures
 * there are real backend failures that must surface, never silently replaced
 * with mock success.
 */
export function createRuntimeBackend(): SermonBackend {
  return isTauriRuntime() ? new TauriSermonBackend() : new MockSermonBackend();
}

const BackendContext = createContext<SermonBackend | null>(null);

export function BackendProvider({
  backend,
  children,
}: {
  /** Optional explicit override (tests / composition). Defaults to runtime detection. */
  backend?: SermonBackend;
  children: ReactNode;
}) {
  const resolved = useMemo(() => backend ?? createRuntimeBackend(), [backend]);
  return <BackendContext.Provider value={resolved}>{children}</BackendContext.Provider>;
}

export function useBackend(): SermonBackend {
  const backend = useContext(BackendContext);
  if (backend === null) {
    throw new Error('useBackend must be used within a BackendProvider');
  }
  return backend;
}
