'use client';
import React from 'react';
import { Zap, AlertTriangle, Info } from 'lucide-react';
import type { IllustrationFatigueResult } from '@/lib/backend/types';
import Icon from '@/components/ui/AppIcon';


interface IllustrationFatiguePanelProps {
  items: IllustrationFatigueResult[];
}

const SEVERITY_CONFIG = {
  high: { label: 'High', color: 'text-alert-red', bg: 'bg-alert-red/10', border: 'border-alert-red/30', icon: AlertTriangle },
  medium: { label: 'Medium', color: 'text-warn-amber', bg: 'bg-warn-amber/10', border: 'border-warn-amber/30', icon: Zap },
  low: { label: 'Low', color: 'text-ref-blue', bg: 'bg-ref-blue/8', border: 'border-ref-blue/20', icon: Info },
};

export default function IllustrationFatiguePanel({ items }: IllustrationFatiguePanelProps) {
  return (
    <div className="card-panel">
      <div className="flex items-center gap-2 mb-4">
        <Zap size={14} className="text-warn-amber" />
        <h2 className="text-sm font-600 text-foreground">Illustration Fatigue</h2>
        <span className="text-2xs font-mono-data bg-warn-amber/15 text-warn-amber px-2 py-0.5 rounded ml-auto">
          {items.filter((i) => i.severity === 'high').length} high risk
        </span>
      </div>
      <p className="text-xs text-muted-foreground mb-3">
        Illustrations used frequently across recent sermons. Consider fresh material.
      </p>
      <div className="space-y-2">
        {items.map((item) => {
          const cfg = SEVERITY_CONFIG[item.severity];
          const Icon = cfg.icon;
          return (
            <div
              key={`fatigue-${item.illustration.replace(/\s+/g, '-')}`}
              className={`flex items-start gap-3 p-3 rounded border ${cfg.border} ${cfg.bg}`}
            >
              <Icon size={12} className={`${cfg.color} mt-0.5 flex-shrink-0`} />
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-xs font-600 text-foreground italic truncate">
                    &ldquo;{item.illustration}&rdquo;
                  </p>
                  <span className={`text-2xs font-mono-data font-600 ${cfg.color} flex-shrink-0`}>
                    ×{item.useCount}
                  </span>
                </div>
                <p className="text-2xs text-muted-foreground mt-0.5 truncate">
                  Last: {item.lastUsedIn} — {item.lastUsedOn}
                </p>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}