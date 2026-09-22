import React from 'react';
import AppLayout from '@/components/AppLayout';
import { BackendProvider } from '@/lib/backend/BackendContext';
import SermonEditorWorkspace from './components/SermonEditorWorkspace';
import FirstRunOnboarding from './components/onboarding/FirstRunOnboarding';

export default function HomePage() {
  return (
    <BackendProvider>
      <AppLayout>
        <SermonEditorWorkspace />
      </AppLayout>
      {/* First-run welcome + tour; renders nothing once completed. */}
      <FirstRunOnboarding />
    </BackendProvider>
  );
}