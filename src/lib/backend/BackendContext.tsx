'use client';
import React, { createContext, useContext, ReactNode } from 'react';
import type { SermonBackend } from './SermonBackend';
import { MockSermonBackend } from './MockSermonBackend';

// BACKEND INTEGRATION POINT:
// At application startup, inject either MockSermonBackend or TauriSermonBackend.
// Components never instantiate either class directly.

const defaultBackend: SermonBackend = new MockSermonBackend();

const BackendContext = createContext<SermonBackend>(defaultBackend);

export function BackendProvider({
  backend,
  children,
}: {
  backend?: SermonBackend;
  children: ReactNode;
}) {
  return (
    <BackendContext.Provider value={backend ?? defaultBackend}>
      {children}
    </BackendContext.Provider>
  );
}

export function useBackend(): SermonBackend {
  return useContext(BackendContext);
}