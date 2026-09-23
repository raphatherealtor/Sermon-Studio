/**
 * Release Track R — Reference Context preference persistence.
 *
 * The smallest persistence seam, mirroring the first-run pattern: a single
 * localStorage key holding the SELECTED context. The catalog itself is a
 * static frontend module. Nothing here is ever written to the vault, the
 * index, or any backend command — changing the context cannot alter sermon
 * content.
 */

import type { MinistryContext } from './types';

const CONTEXT_KEY = 'sermon-studio.reference-context';

const VALID: MinistryContext[] = ['none', 'cogic-pentecostal', 'nondenominational'];

function store(): Storage | null {
  return typeof window === 'undefined' ? null : window.localStorage;
}

/** The selected ministry context; 'none' when unset or unrecognized. */
export function getMinistryContext(): MinistryContext {
  const raw = store()?.getItem(CONTEXT_KEY);
  return VALID.includes(raw as MinistryContext) ? (raw as MinistryContext) : 'none';
}

/** Persist the selected context. Invalid values fall back to 'none'. */
export function setMinistryContext(context: MinistryContext): void {
  store()?.setItem(CONTEXT_KEY, VALID.includes(context) ? context : 'none');
}
