import React from 'react';
import AppLayout from '@/components/AppLayout';
import { BackendProvider } from '@/lib/backend/BackendContext';
import SettingsContent from './components/SettingsContent';

export default function SettingsPage() {
  return (
    <BackendProvider>
      <AppLayout>
        <SettingsContent />
      </AppLayout>
    </BackendProvider>
  );
}