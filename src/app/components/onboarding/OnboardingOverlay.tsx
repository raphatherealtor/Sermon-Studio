/**
 * Track K — First-run overlay: dedication, opening Scripture, and a
 * five-step tour of Sermon Studio.
 *
 * Restrained by design: existing Sermon Studio visual language only
 * (bg-background / bg-panel / fg-dim / accent tokens, btn-primary,
 * font-mono-data), no new visual system. No mention of AI — the product
 * demonstrates value with zero AI.
 */

'use client';
import React, { useMemo, useState } from 'react';
import { ChevronRight, ChevronLeft, BookOpen } from 'lucide-react';
import { dailyScripture } from '@/lib/onboarding/dailyScripture';
import RkMark from '@/app/components/common/RkMark';

const DEDICATION = [
  'In honor of forty years of ministry and preaching.',
  'Sermon Studio was created to preserve a lifetime of study, proclamation, and pastoral work — and to make that body of work easier to revisit, understand, and carry forward.',
];

interface TourStep {
  title: string;
  body: string;
}

const TOUR_STEPS: TourStep[] = [
  {
    title: 'Your Archive',
    body: 'Every sermon has a home here. The archive preserves decades of study — passages, themes, illustrations, and pastoral thought — and keeps it searchable and close at hand.',
  },
  {
    title: 'Your Study Tools',
    body: 'Scripture, original-language notes, and cross-references sit beside your draft as you write, so the text stays open in front of you.',
  },
  {
    title: 'Your Patterns',
    body: 'Across hundreds of sermons, patterns emerge: the passages you return to, the themes you carry, the illustrations you reach for. Sermon Studio makes those patterns inspectable.',
  },
  {
    title: 'Your Writing Desk',
    body: 'A quiet, focused place to draft, revise, and prepare — with structure checks that keep the main thing the main thing.',
  },
  {
    title: 'Your Legacy',
    body: 'Taken together, these sermons are more than files: they are a record of a calling. Sermon Studio exists to preserve that record and make it easier to revisit, understand, and carry forward.',
  },
];

interface OnboardingOverlayProps {
  /** Called when the user presses "Enter Sermon Studio". */
  onFinish: () => void;
}

export default function OnboardingOverlay({ onFinish }: OnboardingOverlayProps) {
  // -1 = welcome (dedication + opening Scripture), 0..4 = tour steps.
  const [screen, setScreen] = useState(-1);
  const scripture = useMemo(() => dailyScripture(new Date()), []);

  const isWelcome = screen === -1;
  const isLastStep = screen === TOUR_STEPS.length - 1;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/97 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label="Welcome to Sermon Studio"
      data-testid="onboarding-overlay"
    >
      <div className="w-full max-w-lg mx-6 card-panel p-8 space-y-6">
        {isWelcome ? (
          <div className="space-y-6" data-testid="onboarding-welcome">
            <div className="space-y-2">
              <RkMark withAttribution />
              <p className="text-2xs font-mono-data uppercase tracking-widest text-accent">
                Sermon Studio
              </p>
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
              onClick={() => setScreen(0)}
              data-testid="onboarding-begin"
            >
              Begin the tour <ChevronRight size={13} />
            </button>
          </div>
        ) : (
          <div className="space-y-6" data-testid={`onboarding-step-${screen}`}>
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <p className="text-2xs font-mono-data uppercase tracking-widest text-accent">
                  Step {screen + 1} of {TOUR_STEPS.length}
                </p>
                <BookOpen size={13} className="text-fg-dim" />
              </div>
              <h2 className="text-lg font-600 text-fg">{TOUR_STEPS[screen].title}</h2>
              <p className="text-sm text-fg-dim leading-relaxed">{TOUR_STEPS[screen].body}</p>
            </div>

            {/* Step indicator */}
            <div className="flex items-center gap-1.5">
              {TOUR_STEPS.map((_, i) => (
                <span
                  key={`dot-${i}`}
                  className={`h-1 rounded-full transition-all ${
                    i === screen ? 'w-6 bg-accent' : 'w-2 bg-elevated'
                  }`}
                />
              ))}
            </div>

            <div className="flex items-center justify-between gap-3">
              <button
                className="btn-ghost"
                onClick={() => setScreen((s) => (s <= 0 ? -1 : s - 1))}
                data-testid="onboarding-back"
              >
                <ChevronLeft size={13} /> Back
              </button>
              {isLastStep ? (
                <button
                  className="btn-primary"
                  onClick={onFinish}
                  data-testid="onboarding-enter"
                >
                  Enter Sermon Studio <ChevronRight size={13} />
                </button>
              ) : (
                <button
                  className="btn-primary"
                  onClick={() => setScreen((s) => s + 1)}
                  data-testid="onboarding-next"
                >
                  Next <ChevronRight size={13} />
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
