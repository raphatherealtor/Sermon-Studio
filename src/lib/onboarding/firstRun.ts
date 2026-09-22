/**
 * Track K — First-run persistence seam.
 *
 * The smallest possible persistence: localStorage. Works identically in the
 * browser Mock build and the native Tauri webview (no backend architecture
 * change, no IPC). SSR-safe: every read/write is guarded and the gate
 * component only reads after mount.
 */

const COMPLETED_KEY = 'sermon-studio.onboarding.completedAt';
const REPLAY_KEY = 'sermon-studio.onboarding.replayRequested';

/** Dispatched on the window when the user asks to see the tour again. */
export const TOUR_REPLAY_EVENT = 'sermon-studio:replay-tour';

function store(): Storage | null {
  return typeof window === 'undefined' ? null : window.localStorage;
}

/** True once the user has finished the first-run experience. */
export function isOnboardingCompleted(): boolean {
  return store()?.getItem(COMPLETED_KEY) != null;
}

/** Persist first-run completion locally. */
export function markOnboardingCompleted(): void {
  store()?.setItem(COMPLETED_KEY, new Date().toISOString());
}

/**
 * Ask for the tour to be shown again (Settings → "Show tour again").
 * Records a replay request so the gate reopens even if the user is on
 * another route when they ask, and fires an event for the same-page case.
 */
export function requestTourReplay(): void {
  store()?.setItem(REPLAY_KEY, '1');
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent(TOUR_REPLAY_EVENT));
  }
}

/** Consume a pending replay request; true if one was pending. */
export function consumeTourReplayRequest(): boolean {
  const s = store();
  if (!s) return false;
  const pending = s.getItem(REPLAY_KEY) === '1';
  if (pending) s.removeItem(REPLAY_KEY);
  return pending;
}

/** Test-only helper: reset all first-run state. */
export function resetOnboardingState(): void {
  const s = store();
  if (!s) return;
  s.removeItem(COMPLETED_KEY);
  s.removeItem(REPLAY_KEY);
}
