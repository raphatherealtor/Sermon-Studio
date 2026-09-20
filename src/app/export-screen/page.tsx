import React from 'react';
import AppLayout from '@/components/AppLayout';
import { BackendProvider } from '@/lib/backend/BackendContext';
import ExportScreenContent from './components/ExportScreenContent';

export default function ExportPage() {
  return (
    <BackendProvider>
      <AppLayout>
        <ExportScreenContent />
      </AppLayout>
    </BackendProvider>
  );
}