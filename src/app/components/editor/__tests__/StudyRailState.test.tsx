import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import StudyRail from '../StudyRail';
import { BackendProvider } from '@/lib/backend/BackendContext';
import { useEditorStore } from '@/lib/store/editorStore';
import type { SermonBackend } from '@/lib/backend/SermonBackend';
import type { PassageResult, SermonDocument } from '@/lib/backend/types';

function documentFor(id: string, scripture: string): SermonDocument {
  return {
    id, title: id, scripture, series: null, status: 'draft', body: '# Sermon',
    outline: [], tags: [], createdAt: '2026-09-24T00:00:00Z',
    updatedAt: '2026-09-24T00:00:00Z', preachedOn: null, version: 1, directives: [],
  };
}

function passage(reference: string, text: string): PassageResult {
  return { reference, translation: 'KJV', text, verses: [{ verse: 1, text }] };
}

it('refreshes study passage on sermon switch and ignores the old late response', async () => {
  const pending = new Map<string, (value: PassageResult) => void>();
  const getPassage = jest.fn((reference: string) => new Promise<PassageResult>((resolve) => {
    pending.set(reference, resolve);
  }));
  useEditorStore.setState({ activeDocument: documentFor('sermon-a', 'John 6:35'), activeSermonId: 'sermon-a' });
  render(<BackendProvider backend={{ getPassage } as unknown as SermonBackend}><StudyRail /></BackendProvider>);
  fireEvent.keyDown(screen.getByPlaceholderText('e.g. John 6:35'), { key: 'Enter' });
  await waitFor(() => expect(getPassage).toHaveBeenCalledWith('John 6:35'));

  act(() => useEditorStore.getState().setDocument(documentFor('sermon-b', 'Romans 8:1')));
  await waitFor(() => expect(getPassage).toHaveBeenCalledWith('Romans 8:1'));
  expect(screen.getByPlaceholderText('e.g. John 6:35')).toHaveProperty('value', 'Romans 8:1');

  await act(async () => pending.get('Romans 8:1')?.(passage('Romans 8:1', 'No condemnation.')));
  expect(screen.getByText('No condemnation.')).toBeTruthy();
  await act(async () => pending.get('John 6:35')?.(passage('John 6:35', 'Bread of life.')));
  expect(screen.queryByText('Bread of life.')).toBeNull();
  expect(screen.getByText('No condemnation.')).toBeTruthy();

  fireEvent.click(screen.getByTitle('Insert Insert'));
  expect(useEditorStore.getState().activeDocument?.body).toContain('> Romans 8:1: No condemnation.');
});
