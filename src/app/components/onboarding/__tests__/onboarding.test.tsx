/**
 * Track K — First-run onboarding tests.
 *
 * Covers: first run shows; completion persists; subsequent launch skips;
 * replay works; Scripture selection is deterministic; no network usage;
 * the normal app stays reachable underneath.
 */

import React from 'react';
import { render, screen, fireEvent, act } from '@testing-library/react';
import FirstRunOnboarding from '../FirstRunOnboarding';
import {
  isOnboardingCompleted,
  markOnboardingCompleted,
  requestTourReplay,
  resetOnboardingState,
} from '@/lib/onboarding/firstRun';
import { dailyScripture, DAILY_SCRIPTURES } from '@/lib/onboarding/dailyScripture';

beforeEach(() => {
  window.localStorage.clear();
  resetOnboardingState();
});

describe('first-run gate', () => {
  it('shows the welcome on first run', () => {
    render(<FirstRunOnboarding />);
    expect(screen.getByTestId('onboarding-overlay')).toBeTruthy();
    expect(screen.getByText('In honor of forty years of ministry and preaching.')).toBeTruthy();
    // Offline opening Scripture: reference and KJV label, no network.
    expect(screen.getByText(/KJV/)).toBeTruthy();
    expect(screen.getByText('Begin the tour')).toBeTruthy();
  });

  it('skips the overlay on subsequent launches once completed', () => {
    markOnboardingCompleted();
    expect(isOnboardingCompleted()).toBe(true);
    render(<FirstRunOnboarding />);
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();
  });

  it('finishing the tour persists completion (Enter Sermon Studio)', () => {
    render(<FirstRunOnboarding />);
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    // Walk all five steps.
    for (let i = 0; i < 4; i++) {
      fireEvent.click(screen.getByTestId('onboarding-next'));
    }
    expect(screen.getByText('Your Legacy')).toBeTruthy();
    fireEvent.click(screen.getByTestId('onboarding-enter'));
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();
    expect(isOnboardingCompleted()).toBe(true);
    // Remount = subsequent launch: skipped.
    render(<FirstRunOnboarding />);
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();
  });

  it('shows the five tour steps in order', () => {
    render(<FirstRunOnboarding />);
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    const titles = [
      'Your Archive',
      'Your Study Tools',
      'Your Patterns',
      'Your Writing Desk',
      'Your Legacy',
    ];
    titles.forEach((title, i) => {
      expect(screen.getByText(title)).toBeTruthy();
      expect(screen.getByText(`Step ${i + 1} of 5`)).toBeTruthy();
      if (i < titles.length - 1) fireEvent.click(screen.getByTestId('onboarding-next'));
    });
  });

  it('replay: requestTourReplay reopens the tour even after completion', () => {
    markOnboardingCompleted();
    render(<FirstRunOnboarding />);
    expect(screen.queryByTestId('onboarding-overlay')).toBeNull();

    act(() => {
      requestTourReplay();
    });
    expect(screen.getByTestId('onboarding-overlay')).toBeTruthy();
  });

  it('replay request persists across a remount (settings → back to library)', () => {
    markOnboardingCompleted();
    requestTourReplay();
    // Fresh mount, as if navigating back to the home view.
    render(<FirstRunOnboarding />);
    expect(screen.getByTestId('onboarding-overlay')).toBeTruthy();
    // The pending request is consumed: another fresh mount skips again.
    render(<FirstRunOnboarding />);
    expect(screen.queryAllByTestId('onboarding-overlay')).toHaveLength(1);
  });

  it('never mentions AI in onboarding', () => {
    render(<FirstRunOnboarding />);
    fireEvent.click(screen.getByTestId('onboarding-begin'));
    for (let i = 0; i < 4; i++) fireEvent.click(screen.getByTestId('onboarding-next'));
    expect(screen.queryByText(/AI/i)).toBeNull();
  });
});

describe('daily Scripture selection', () => {
  it('is deterministic: same date always yields the same verse', () => {
    const date = new Date(Date.UTC(2026, 5, 14)); // fixed date
    const a = dailyScripture(date);
    const b = dailyScripture(new Date(date.getTime()));
    expect(a).toEqual(b);
    expect(DAILY_SCRIPTURES).toContainEqual(a);
  });

  it('changes across days (no pathological pinning)', () => {
    const day1 = dailyScripture(new Date(Date.UTC(2026, 0, 1)));
    const day2 = dailyScripture(new Date(Date.UTC(2026, 0, 2)));
    expect(day1).not.toEqual(day2);
  });

  it('uses no network: selection is pure local computation', () => {
    const fetchSpy = jest.fn();
    (global as { fetch?: unknown }).fetch = fetchSpy;
    dailyScripture(new Date());
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});
