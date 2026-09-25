/**
 * Track K — First-run gate.
 *
 * Mounted once at the app root, above the workspace. On first launch (or a
 * replay request) it renders the onboarding overlay while the normal app
 * stays mounted underneath, so nothing is unreachable. After completion the
 * gate renders nothing until the user asks to see the tour again.
 */

'use client';
import React, { useEffect, useState } from 'react';
import OnboardingOverlay from './OnboardingOverlay';
import {
  isOnboardingCompleted,
  markOnboardingCompleted,
  consumeTourReplayRequest,
  TOUR_REPLAY_EVENT,
} from '@/lib/onboarding/firstRun';

export default function FirstRunOnboarding() {
  // null until mounted: avoids SSR/localStorage hydration mismatch.
  const [visible, setVisible] = useState<boolean | null>(null);
  // -1 = welcome (first run); 0 = first tour step (replay restarts at step 1).
  const [startAtStep, setStartAtStep] = useState(-1);

  useEffect(() => {
    const replayRequested = consumeTourReplayRequest();
    if (!isOnboardingCompleted() || replayRequested) {
      setStartAtStep(replayRequested ? 0 : -1);
      setVisible(true);
      return;
    }
    setVisible(false);

    const onReplay = () => {
      setStartAtStep(0);
      setVisible(true);
    };
    window.addEventListener(TOUR_REPLAY_EVENT, onReplay);
    return () => window.removeEventListener(TOUR_REPLAY_EVENT, onReplay);
  }, []);

  if (visible !== true) return null;

  const finish = () => {
    markOnboardingCompleted();
    setVisible(false);
  };

  return <OnboardingOverlay onFinish={finish} initialScreen={startAtStep} />;
}
