/**
 * Release Track R — the understated RK monogram.
 *
 * A small bordered "RK" square in existing tokens, with an optional
 * attribution caption. Used on the onboarding dedication, the Settings →
 * About inscription, and as a subtle app mark. Never a redesign element:
 * quiet, dignified, secondary to the forty-year ministry framing.
 */
import React from 'react';
import { RK_MONOGRAM, RK_ATTRIBUTION } from '@/lib/identity/rk';

interface RkMarkProps {
  /** Show the "Prepared for Raphael Knox · Reverend Knox" caption. */
  withAttribution?: boolean;
  /** Monogram square size in pixels (default 28). */
  size?: number;
}

export default function RkMark({ withAttribution = false, size = 28 }: RkMarkProps) {
  return (
    <div className="flex items-center gap-2" data-testid="rk-mark">
      <span
        className="inline-flex items-center justify-center border border-border rounded-sm text-fg-dim font-editor font-600 select-none"
        style={{ width: size, height: size, fontSize: size * 0.42, letterSpacing: '0.05em' }}
        aria-label={RK_MONOGRAM}
      >
        {RK_MONOGRAM}
      </span>
      {withAttribution && (
        <span className="text-2xs font-mono-data text-fg-dim">{RK_ATTRIBUTION}</span>
      )}
    </div>
  );
}
