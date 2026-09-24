import React from 'react';
import { render, screen } from '@testing-library/react';
import CodecTestContent from '../CodecTestContent';
import { BackendProvider } from '@/lib/backend/BackendContext';
import { MockSermonBackend } from '@/lib/backend/MockSermonBackend';
import { KNOWN_DIRECTIVES, parseDirectives, serializeDirectives } from '@/editor/codec/directiveCodec';

it('shows exactly the codec-known directives and preserves unknown input verbatim', () => {
  render(<BackendProvider backend={new MockSermonBackend()}><CodecTestContent /></BackendProvider>);
  const reference = screen.getByText('Known Directives (specialized rendering)').parentElement;
  const shown = Array.from(reference?.querySelectorAll('li') ?? []).map((item) => item.textContent?.replace(':::', ''));
  expect(shown).toEqual(Array.from(KNOWN_DIRECTIVES));
  for (const name of shown) {
    expect(parseDirectives(`:::${name}\nbody\n:::`)[0].kind).toBe('known');
  }

  const unknown = ':::big-idea{text="keep spacing"}\n  Keep this body exactly.  \n:::';
  const parsed = parseDirectives(unknown);
  expect(parsed[0].kind).toBe('unknown');
  expect(serializeDirectives(parsed)).toBe(unknown);
});
