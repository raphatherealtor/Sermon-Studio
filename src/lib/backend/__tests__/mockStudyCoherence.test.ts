import { MockSermonBackend } from '../MockSermonBackend';

it('does not reuse John 6 browser fixtures for an unrelated sermon or passage', async () => {
  const backend = new MockSermonBackend();
  expect((await backend.getCrossReferences('John 6:35')).length).toBeGreaterThan(0);
  expect(await backend.getCrossReferences('Romans 8:1')).toEqual([]);

  const johnChain = await backend.getChainStudy('John 6:35');
  const romansChain = await backend.getChainStudy('Romans 8:1');
  expect(johnChain.chains.length).toBeGreaterThan(0);
  expect(romansChain.seedReference).toBe('Romans 8:1');
  expect(romansChain.chains).toEqual([]);
  expect(romansChain.archiveConnections).toEqual([]);

  const history = await backend.getPreachedOn('Romans 8:1');
  expect(history.map((sermon) => sermon.sermonTitle)).toEqual(['No Condemnation']);
  expect(await backend.getPreachedOn('John 6:35')).toEqual([]);

  expect((await backend.getSermonInsights('sermon-001')).insights.length).toBeGreaterThan(0);
  const romansInsights = await backend.getSermonInsights('sermon-005');
  expect(romansInsights.subjectReference).toBe('Romans 8:1–11');
  expect(romansInsights.insights).toEqual([]);
});

it('rejects malformed Strong\'s IDs instead of inventing a lexicon entry', async () => {
  const backend = new MockSermonBackend();
  await expect(backend.getStrongs('not-a-strongs-id')).rejects.toThrow('Invalid Strong\'s ID');
  await expect(backend.getStrongs('G740')).resolves.toMatchObject({ id: 'G740', lemma: 'ἄρτος' });
});
