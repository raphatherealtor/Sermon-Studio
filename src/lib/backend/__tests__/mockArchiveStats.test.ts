import { MockSermonBackend } from '../MockSermonBackend';

it('derives preached archive count from the same collection as the filtered rows', async () => {
  const backend = new MockSermonBackend();
  const preached = (await backend.listSermons()).filter((sermon) => sermon.status === 'preached');
  expect((await backend.getArchiveStats()).sermonsByStatus.preached).toBe(preached.length);

  await backend.archiveSermon(preached[0].id);
  const remaining = (await backend.listSermons()).filter((sermon) => sermon.status === 'preached');
  const stats = await backend.getArchiveStats();
  expect(stats.sermonsByStatus.preached).toBe(remaining.length);
  expect(stats.totalSermons).toBe((await backend.listSermons()).length);
});
