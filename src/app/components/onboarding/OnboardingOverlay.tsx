/**
 * Track K — First-run overlay: dedication, opening Scripture, and a
 * five-step GUIDED tour of Sermon Studio.
 *
 * The tour is a real product walkthrough, not a slideshow: each step
 * spotlights the actual UI region it describes (via stable `data-tour`
 * targets), dims the rest of the interface, and positions the explanatory
 * card beside the highlighted target. Missing targets degrade to a centered
 * card rather than crashing.
 *
 * Restrained by design: existing Sermon Studio visual language only, no new
 * visual system, no mention of AI.
 */

'use client';
import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ChevronRight, ChevronLeft, BookOpen } from 'lucide-react';
import { dailyScripture } from '@/lib/onboarding/dailyScripture';
import RkMark from '@/app/components/common/RkMark';

const DEDICATION = [
  'In honor of forty years of ministry and preaching.',
  'Sermon Studio was created to preserve a lifetime of study, proclamation, and pastoral work — and to make that body of work easier to revisit, understand, and carry forward.',
];

export interface TourStep {
  title: string;
  /** `data-tour` target id highlighted for this step. */
  target: string;
  body: string;
}

export const TOUR_STEPS: TourStep[] = [
  {
    title: 'Your Archive',
    target: 'archive',
    body: 'This is where your sermons are preserved, searched, reopened, and connected over time.',
  },
  {
    title: 'Your Study Tools',
    target: 'study-tools',
    body: 'Scripture, lexical study, cross-references, Chain Study, and related study material live here.',
  },
  {
    title: 'Your Patterns',
    target: 'insights',
    body: 'Sermon Studio can reveal documented connections across your own ministry history.',
  },
  {
    title: 'Your Writing Desk',
    target: 'writing-desk',
    body: 'This remains your primary writing and sermon-development workspace.',
  },
  {
    title: 'Your Legacy',
    target: 'archive',
    body: 'Your sermons remain your files, accumulating into a searchable ministry archive you can revisit, understand, and carry forward.',
  },
];

/** Rough card dimensions used for side-preference math (not exact layout). */
const CARD_SIZE = { width: 340, height: 240 };
const CARD_MARGIN = 16;

export interface RectLike {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * Pick a position for the tour card beside `rect` without covering it:
 * right when space permits, then left, then below, then above. A `null`
 * rect (missing target) falls back to the viewport center.
 */
export function computeTourCardPosition(
  rect: RectLike | null,
  viewport: { width: number; height: number },
  card: { width: number; height: number } = CARD_SIZE,
): { left: number; top: number } {
  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));
  const centerX = Math.max(0, (viewport.width - card.width) / 2);
  const centerY = Math.max(0, (viewport.height - card.height) / 2);
  if (!rect) {
    return { left: centerX, top: centerY };
  }
  if (rect.right + CARD_MARGIN + card.width <= viewport.width) {
    return {
      left: rect.right + CARD_MARGIN,
      top: clamp(rect.top, CARD_MARGIN, viewport.height - card.height - CARD_MARGIN),
    };
  }
  if (rect.left - CARD_MARGIN - card.width >= 0) {
    return {
      left: rect.left - CARD_MARGIN - card.width,
      top: clamp(rect.top, CARD_MARGIN, viewport.height - card.height - CARD_MARGIN),
    };
  }
  if (rect.bottom + CARD_MARGIN + card.height <= viewport.height) {
    return {
      left: clamp(rect.left, CARD_MARGIN, viewport.width - card.width - CARD_MARGIN),
      top: rect.bottom + CARD_MARGIN,
    };
  }
  return {
    left: clamp(rect.left, CARD_MARGIN, viewport.width - card.width - CARD_MARGIN),
    top: Math.max(CARD_MARGIN, rect.top - CARD_MARGIN - card.height),
  };
}

function viewportSize(): { width: number; height: number } {
  if (typeof window === 'undefined') return { width: 1024, height: 768 };
  return { width: window.innerWidth, height: window.innerHeight };
}

interface OnboardingOverlayProps {
  /** Called for both "Enter Sermon Studio" (finish) and "Skip". */
  onFinish: () => void;
  /** Screen to start on: -1 = welcome, 0..4 = tour step. */
  initialScreen?: number;
}

export default function OnboardingOverlay({ onFinish, initialScreen = -1 }: OnboardingOverlayProps) {
  const [screen, setScreen] = useState(initialScreen);
  const scripture = useMemo(() => dailyScripture(new Date()), []);
  const isWelcome = screen === -1;
  const isLastStep = screen === TOUR_STEPS.length - 1;

  // Escape always cleanly dismisses the overlay (no permanently dimmed UI).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onFinish();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onFinish]);

  return (
    <div
      className="fixed inset-0 z-50"
      role="dialog"
      aria-modal="true"
      aria-label="Welcome to Sermon Studio"
      data-testid="onboarding-overlay"
    >
      {isWelcome ? (
        <WelcomeCard
          scripture={scripture}
          onBegin={() => setScreen(0)}
          onSkip={onFinish}
        />
      ) : (
        <GuidedStep
          step={screen}
          isLast={isLastStep}
          onNext={() => setScreen((s) => s + 1)}
          onBack={() => setScreen((s) => (s <= 0 ? -1 : s - 1))}
          onSkip={onFinish}
          onFinish={onFinish}
        />
      )}
    </div>
  );
}

function WelcomeCard({
  scripture,
  onBegin,
  onSkip,
}: {
  scripture: { text: string; reference: string };
  onBegin: () => void;
  onSkip: () => void;
}) {
  return (
    <div className="fixed inset-0 flex items-center justify-center bg-background/97 backdrop-blur-sm">
      <div className="w-full max-w-lg mx-6 card-panel p-8 space-y-6">
        <div className="space-y-6" data-testid="onboarding-welcome">
          <div className="space-y-2">
            <RkMark withAttribution />
            <p className="text-2xs font-mono-data uppercase tracking-widest text-accent">Sermon Studio</p>
            <h1 className="text-xl font-600 text-fg leading-snug">{DEDICATION[0]}</h1>
            <p className="text-sm text-fg-dim leading-relaxed">{DEDICATION[1]}</p>
          </div>

          <figure className="border-l-2 border-accent/40 pl-4 space-y-1.5">
            <blockquote className="text-sm text-fg/90 leading-relaxed font-editor">
              “{scripture.text}”
            </blockquote>
            <figcaption className="text-2xs font-mono-data text-fg-dim">
              {scripture.reference} · KJV
            </figcaption>
          </figure>

          <button
            className="btn-primary w-full justify-center"
            onClick={onBegin}
            data-testid="onboarding-begin"
          >
            Begin the tour <ChevronRight size={13} />
          </button>

          <button onClick={onSkip} className="text-2xs font-mono-data text-fg-dim hover:text-fg transition-colors" data-testid="onboarding-skip">
            Skip tour
          </button>
        </div>
      </div>
    </div>
  );
}

function GuidedStep({
  step,
  isLast,
  onNext,
  onBack,
  onSkip,
  onFinish,
}: {
  step: number;
  isLast: boolean;
  onNext: () => void;
  onBack: () => void;
  onSkip: () => void;
  onFinish: () => void;
}) {
  const def = TOUR_STEPS[step];
  const [rect, setRect] = useState<DOMRect | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  // Resolve + track the target region; re-measure on step change, resize, and
  // scroll so the spotlight follows the real element.
  useLayoutEffect(() => {
    const measure = () => {
      const el = document.querySelector<HTMLElement>(`[data-tour="${def.target}"]`);
      setRect(el ? el.getBoundingClientRect() : null);
    };
    measure();
    window.addEventListener('resize', measure);
    window.addEventListener('scroll', measure, true);
    return () => {
      window.removeEventListener('resize', measure);
      window.removeEventListener('scroll', measure, true);
    };
  }, [def.target]);

  // Keyboard: arrows navigate, Tab stays within the card. Escape is handled
  // at the overlay root (window listener) so it works regardless of focus.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      if (isLast) onFinish();
      else onNext();
      return;
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      onBack();
      return;
    }
    if (e.key === 'Tab') {
      const focusables = cardRef.current
        ? Array.from(cardRef.current.querySelectorAll<HTMLElement>('button'))
        : [];
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };

  const pos = computeTourCardPosition(rect, viewportSize());

  return (
    <>
      {/* Spotlight: a transparent window over the target with a dimming
          box-shadow and a modest accent ring. The target stays fully visible
          and readable; nothing else is interactive while the tour is active. */}
      {rect && (
        <div
          className="fixed z-40 pointer-events-none rounded-lg transition-all duration-300 ease-out"
          style={{
            left: rect.left - 8,
            top: rect.top - 8,
            width: Math.max(0, rect.width + 16),
            height: Math.max(0, rect.height + 16),
            boxShadow: '0 0 0 9999px rgba(8, 10, 14, 0.62)',
          }}
          data-testid="tour-spotlight"
        >
          <div className="absolute inset-0 rounded-lg ring-2 ring-accent" aria-hidden="true" />
        </div>
      )}

      {/* Explanatory card, positioned beside the target. */}
      <div
        ref={cardRef}
        className="fixed z-50 w-80 card-panel p-5 space-y-4 shadow-xl"
        style={{ left: pos.left, top: pos.top }}
        onKeyDown={onKeyDown}
        data-testid="tour-card"
        data-tour-target={def.target}
      >
        <div className="space-y-2" data-testid={`onboarding-step-${step}`}>
          <div className="flex items-center justify-between">
            <p className="text-2xs font-mono-data uppercase tracking-widest text-accent">
              Step {step + 1} of {TOUR_STEPS.length}
            </p>
            <BookOpen size={13} className="text-fg-dim" />
          </div>
          <h2 className="text-lg font-600 text-fg">{def.title}</h2>
          <p className="text-sm text-fg-dim leading-relaxed">{def.body}</p>
        </div>

        {/* Step indicator */}
        <div className="flex items-center gap-1.5" data-testid="tour-progress">
          {TOUR_STEPS.map((_, i) => (
            <span
              key={`dot-${i}`}
              className={`h-1 rounded-full transition-all ${i === step ? 'w-6 bg-accent' : 'w-2 bg-elevated'}`}
            />
          ))}
        </div>

        <div className="flex items-center justify-between gap-3">
          <button className="btn-ghost" onClick={onBack} data-testid="onboarding-back">
            <ChevronLeft size={13} /> Back
          </button>
          <button onClick={onSkip} className="btn-ghost text-fg-dim" data-testid="onboarding-skip">
            Skip
          </button>
          {isLast ? (
            <button className="btn-primary" onClick={onFinish} data-testid="onboarding-enter">
              Enter Sermon Studio <ChevronRight size={13} />
            </button>
          ) : (
            <button className="btn-primary" onClick={onNext} data-testid="onboarding-next">
              Next <ChevronRight size={13} />
            </button>
          )}
        </div>
      </div>
    </>
  );
}
