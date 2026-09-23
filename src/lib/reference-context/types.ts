/**
 * Release Track R — Reference Context types.
 *
 * A LIGHT OPTIONAL REFERENCE LAYER, not a doctrine engine: it helps the
 * pastor compare study material against the ministry context he is working
 * within. It never approves, rejects, or grades doctrine, and it never
 * infers what the pastor personally believes.
 *
 * Corpus boundary (hard rules):
 * - Reference Context content is static contextual metadata. It is NOT
 *   pastor-authored content, so it never enters the lifetime corpus, the
 *   pastor FTS index, Sermon Intelligence scoring, or Research Packet
 *   provenance.
 * - It is stored only in the frontend bundle and a localStorage preference
 *   (which context is selected). It is never written into the vault and no
 *   backend command carries it.
 */

/** The ministry contexts the pastor can compare against. V1: light focus on
 * Pentecostal / COGIC; the layer stays optional and 'none' is the default. */
export type MinistryContext = 'none' | 'cogic-pentecostal' | 'nondenominational';

export const MINISTRY_CONTEXTS: { id: MinistryContext; label: string }[] = [
  { id: 'none', label: 'None' },
  { id: 'cogic-pentecostal', label: 'Pentecostal / COGIC' },
  { id: 'nondenominational', label: 'Non-denominational' },
];

/** A single reference entry: study anchors plus clearly labeled placeholder
 * scope text. NO official quotations are fabricated anywhere in this module;
 * entries are structural and are meant to be populated from authoritative
 * sources later. */
export interface ReferenceContextEntry {
  /** Stable slug, also used for See also links. */
  id: string;
  /** Display topic. */
  topic: string;
  /** Short scope summary, explicitly labeled as a placeholder. */
  summary: string;
  /** Supporting scripture references (study anchors). */
  scriptures: string[];
  /** Source/attribution line. Every V1 entry is a labeled fixture. */
  attribution: string;
  /** Related topic ids ("See also"). */
  seeAlso: string[];
}

/**
 * Isolation sentinel: a distinctive string that isolation tests scan the
 * entire backend and corpus surface for. It must never appear in the vault,
 * in any backend module, in the index, or in Intelligence output.
 */
export const REFERENCE_CONTEXT_SENTINEL = 'REFCTX-ISOLATION-5D1B';
