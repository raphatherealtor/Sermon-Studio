/**
 * Track K — Opening Scripture for the first-run welcome.
 *
 * Offline only: a small curated local set of KJV verses (public domain).
 * No network calls, ever. Selection is deterministic per day so the welcome
 * is stable across launches on the same day instead of noisy random picks.
 * Later, canon.db can replace or extend this source behind the same
 * function signature.
 */

export interface DailyScripture {
  reference: string;
  text: string;
}

/**
 * Curated KJV set — study, proclamation, and preservation themed.
 * Sources: King James Version (public domain).
 */
export const DAILY_SCRIPTURES: DailyScripture[] = [
  {
    reference: 'Psalm 90:12',
    text: 'So teach us to number our days, that we may apply our hearts unto wisdom.',
  },
  {
    reference: '2 Timothy 4:2',
    text: 'Preach the word; be instant in season, out of season; reprove, rebuke, exhort with all longsuffering and doctrine.',
  },
  {
    reference: 'Psalm 119:105',
    text: 'Thy word is a lamp unto my feet, and a light unto my path.',
  },
  {
    reference: 'Nehemiah 8:8',
    text: 'So they read in the book in the law of God distinctly, and gave the sense, and caused them to understand the reading.',
  },
  {
    reference: 'Luke 4:4',
    text: 'And Jesus answered him, saying, It is written, That man shall not live by bread alone, but by every word of God.',
  },
  {
    reference: 'Psalm 1:2',
    text: 'But his delight is in the law of the LORD; and in his law doth he meditate day and night.',
  },
  {
    reference: 'Isaiah 55:11',
    text: 'So shall my word be that goeth forth out of my mouth: it shall not return unto me void, but it shall accomplish that which I please.',
  },
  {
    reference: 'Ezra 7:10',
    text: 'For Ezra had prepared his heart to seek the law of the LORD, and to do it, and to teach in Israel statutes and judgments.',
  },
  {
    reference: '1 Timothy 4:13',
    text: 'Till I come, give attendance to reading, to exhortation, to doctrine.',
  },
  {
    reference: '1 Corinthians 15:58',
    text: 'Therefore, my beloved brethren, be ye stedfast, unmoveable, always abounding in the work of the Lord.',
  },
];

/**
 * Deterministic daily selection: the day-of-year (UTC) picks the verse.
 * Same calendar day → same verse, on every launch, with no randomness and
 * no I/O. UTC keeps the selection stable across timezones for a given date.
 */
export function dailyScripture(date: Date): DailyScripture {
  const yearStart = Date.UTC(date.getUTCFullYear(), 0, 0);
  const dayOfYear = Math.floor((date.getTime() - yearStart) / 86400000);
  return DAILY_SCRIPTURES[dayOfYear % DAILY_SCRIPTURES.length];
}
