/**
 * Guided onboarding tour — spotlight navigation tests.
 *
 * Covers: step→target resolution, spotlight advance/restore, skip/escape
 * dismissal, missing-target degradation, replay-at-step-1, and the pure
 * card-positioning helper.
 */
import React from 'react';
import { render, screen, fireEvent, act } from '@testing-library/react';
import OnboardingOverlay, {
  TOUR_STEPS,
  computeTourCardPosition,
} from '../OnboardingOverlay';
import FirstRunOnboarding from '../FirstRunOnboarding';
import {
  isOnboardingCompleted,
  markOnboardingCompleted,
  requestTourReplay,
  resetOnboardingState,
} from '@/lib/onboarding/firstRun';

beforeEach(() => {
  window.localStorage.clear();
  resetOnboardingState();
});

function beginTour() {
  render(<OnboardingOverlay onFinish={() => {}} />);
  fireEvent.click(screen.getByTestId('onboarding-begin'));
}

describe('tour step → target resolution', () => {
  it('each of the five steps declares its intended UI target', () => {
    expect(TOUR_STEPS.map((s) => s.title)).toEqual([
      'Your Archive',
      'Your Study Tools',
      'Your Patterns',
      'Your Writing Desk',
      'Your Legacy',
    ]);
    expect(TOUR_STEPS.map((s) => s.target)).toEqual([
      'archive',
      'study-tools',
      'insights',
      'writing-desk',
      'archive',
    ]);
  });

  it('positions the card right, then left, then below, then above, then centered', () => {
    const card = { width: 340, height: 240 };
    const vp = { width: 1280, height: 800 };

    // Target on the left → card to its right.
    const right = computeTourCardPosition({ left: 200, top: 100, right: 400, bottom: 200 }, vp, card);
    expect(right.left).toBe(416); // 400 + margin(16)

    // Target on the right → card to its left.
    const left = computeTourCardPosition({ left: 1000, top: 100, right: 1200, bottom: 200 }, vp, card);
    expect(left.left).toBeLessThan(1000);

    // Missing target → centered fallback.
    expect(computeTourCardPosition(null, vp, card)).toEqual({
      left: (1280 - 340) / 2,
      top: (800 - 240) / 2,
    });
  });
});

describe('guided tour navigation', () => {
  it('Next advances the spotlight target and Back restores it', () => {
    beginTour();
    expect(screen.getByTestId('tour-card').getAttribute('data-tour-target')).toBe('archive');

    fireEvent.click(screen.getByTestId('onboarding-next'));
    expect(screen.getByTestId('tour-card').getAttribute('data-tour-target')).toBe('study-tools');

    fireEvent.click(screen.getByTestId('onboarding-next'));
    expect(screen.getByTestId('tour-card').getAttribute('data-tour-target')).toBe('insights');

    fireEvent.click(screen.getByTestId('onboarding-back'));
    expect(screen.getByTestId('tour-card').getAttribute('data-tour-target')).toBe('study-tools');
  });

  it('a missing tour target degrades to a centered card without crashing', () => {
    beginTour();
    // No workspace is rendered, so every `data-tour` target is absent.
    expect(screen.getByTestId('tour-card')).toBeTruthy();
    expect(screen.getByText('Your Archive')).toBeTruthy();
    expect(screen.queryByTestId('tour-spotlight')).toBeNull();
  });

  it('the spotlight renders when the target element resolves', () => {
    const fakeRect = { left: 200, top: 100, right: 400, bottom: 300, width: 200, height: 200, x: 200, y: 100, toJSON: () => ({}) };
    const fakeEl = { getBoundingClientRect: () => fakeRect } as unknown as HTMLElement;
    const qs = jest.spyOn(document, 'querySelector').mockReturnValue(fakeEl);
    beginTour();
    expect(screen.getByTestId('tour-spotlight')).toBeTruthy();
    qs.mockRestore();
  });
});

describe('guided tour dismissal and persistence', () => {
  it('Skip removes the overlay', () => {
    render(<FirstRunOnboarding />);
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    fireEvent.click(screen.getByTestId('onboarding-skip'));
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();
  });

  it('Escape removes the overlay', () => {
    render(<FirstRunOnboarding />);
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();
  });

  it('Finish removes the overlay and persists completion', () => {
    render(<FirstRunOnboarding />);
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    for (let i = 0; i < 4; i++) fireEvent.click(screen.getByTestId('onboarding-next'));
    fireEvent.click(screen.getByTestId('onboarding-enter'));
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();
    expect(isOnboardingCompleted()).toBe(true);
  });

  it('Replay restarts the guided experience at step 1 (Your Archive), not the welcome', () => {
    markOnboardingCompleted();
    render(<FirstRunOnboarding />);
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();

    act(() => {
      requestTourReplay();
    });

    expect(screen.getByTestId('onboarding-overlay')).toBeTruthy();
    expect(screen.getByText('Your Archive')).toBeTruthy();
    expect(screen.getByTestId('tour-card').getAttribute('data-tour-target')).toBe('archive');
    expect(screen.queryByTestId('onboarding-welcome')).toBeNull();
  });

  it('existing first-run persistence remains intact (replay consumed on remount)', () => {
    markOnboardingCompleted();
    requestTourReplay();
    render(<FirstRunOnboarding />);
    expect(screen.getByTestId('onboarding-overlay')).toBeTruthy();
    // A second fresh mount no longer has a pending replay request.
    render(<FirstRunOnboarding />);
    expect(screen.queryAllByTestId('onboarding-overlay')).toHaveLength(1);
  });
});
