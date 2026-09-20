import React from 'react';
import AppLayout from '@/components/AppLayout';
import { BackendProvider } from '@/lib/backend/BackendContext';
import ArchiveOverviewContent from './components/ArchiveOverviewContent';

export default function ArchiveOverviewPage() {
  return (
    <BackendProvider>
      <AppLayout>
        <ArchiveOverviewContent />
      </AppLayout>
    </BackendProvider>
  );
}