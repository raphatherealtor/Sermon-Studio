/**
 * Release Track R — Reference Context catalog (V1 fixtures).
 *
 * Static, offline, clearly labeled placeholder entries for a light
 * Pentecostal / COGIC reference layer. The structural shape (topic, scope
 * label, scripture anchors, attribution, see-also) is real and stable; the
 * doctrinal summaries are intentionally scope labels, NOT official wording —
 * no COGIC or other official quotation is fabricated. Populate with
 * authoritative source material later by replacing `summary`/`attribution`
 * per entry.
 *
 * Language discipline: entries reference and compare. They never approve,
 * reject, or instruct. Forbidden wording is asserted by test.
 */

import type { MinistryContext, ReferenceContextEntry } from './types';

/** Every V1 entry carries the same explicit fixture attribution. */
const FIXTURE_ATTRIBUTION =
  'Fixture reference — populate with official source material';

const COGIC_PENTECOSTAL_ENTRIES: ReferenceContextEntry[] = [
  {
    id: 'salvation',
    topic: 'Salvation',
    summary:
      'Reference scope: salvation by grace through faith and the new birth. Fixture summary — replace with official COGIC source wording.',
    scriptures: ['Ephesians 2:8-9', 'John 3:3-7', '2 Corinthians 5:17'],
    attribution: FIXTURE_ATTRIBUTION,
    seeAlso: ['sanctification', 'holy-spirit'],
  },
  {
    id: 'sanctification',
    topic: 'Sanctification',
    summary:
      'Reference scope: sanctification as a definite work of grace following the new birth. Fixture summary — replace with official COGIC source wording.',
    scriptures: ['1 Thessalonians 4:3', '1 Thessalonians 5:23', 'Hebrews 13:12'],
    attribution: FIXTURE_ATTRIBUTION,
    seeAlso: ['holiness', 'salvation'],
  },
  {
    id: 'holy-spirit',
    topic: 'Holy Spirit / Spirit Baptism',
    summary:
      'Reference scope: the baptism of the Holy Spirit with the initial physical evidence of speaking in tongues. Fixture summary — replace with official COGIC source wording.',
    scriptures: ['Acts 1:8', 'Acts 2:1-4', 'Joel 2:28-29'],
    attribution: FIXTURE_ATTRIBUTION,
    seeAlso: ['salvation', 'divine-healing'],
  },
  {
    id: 'divine-healing',
    topic: 'Divine Healing',
    summary:
      'Reference scope: divine healing provided for in the atoning work of Christ. Fixture summary — replace with official COGIC source wording.',
    scriptures: ['Isaiah 53:4-5', 'James 5:14-15', 'Mark 16:17-18'],
    attribution: FIXTURE_ATTRIBUTION,
    seeAlso: ['holy-spirit', 'eschatology'],
  },
  {
    id: 'holiness',
    topic: 'Holiness',
    summary:
      'Reference scope: holiness of life and conduct as the standard for the believer. Fixture summary — replace with official COGIC source wording.',
    scriptures: ['1 Peter 1:15-16', 'Hebrews 12:14', '2 Timothy 2:19'],
    attribution: FIXTURE_ATTRIBUTION,
    seeAlso: ['sanctification'],
  },
  {
    id: 'eschatology',
    topic: 'Eschatology',
    summary:
      'Reference scope: the second coming of Christ and the consummation of the kingdom. Fixture summary — replace with official COGIC source wording.',
    scriptures: ['1 Thessalonians 4:16-17', 'Revelation 19:11-16', 'Matthew 24:36-44'],
    attribution: FIXTURE_ATTRIBUTION,
    seeAlso: ['divine-healing'],
  },
];

/** V1 catalog: only the Pentecostal / COGIC context carries reference
 * entries. Non-denominational and none are valid selections with an empty
 * catalog (the layer stays light and optional). */
export function referenceEntriesFor(
  context: MinistryContext,
): ReferenceContextEntry[] {
  return context === 'cogic-pentecostal' ? COGIC_PENTECOSTAL_ENTRIES : [];
}

/** Look up an entry by id within a context. */
export function referenceEntryById(
  context: MinistryContext,
  id: string,
): ReferenceContextEntry | null {
  return referenceEntriesFor(context).find((e) => e.id === id) ?? null;
}
