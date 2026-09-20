'use client';
import React from 'react';
import { BookOpen, Layers, AlignJustify, Calendar } from 'lucide-react';
import type { ArchiveStats } from '@/lib/backend/types';



interface ArchiveStatsCardsProps {
  stats: ArchiveStats;
}

export default function ArchiveStatsCards({ stats }: ArchiveStatsCardsProps) {
  const cards = [
    {
      id: 'stat-sermons',
      label: 'Total Sermons',
      value: stats.totalSermons.toString(),
      icon: BookOpen,
      sub: `${stats.sermonsByStatus['preached'] ?? 0} preached`,
      color: 'text-accent',
      bg: 'bg-accent/8',
    },
    {
      id: 'stat-series',
      label: 'Active Series',
      value: stats.totalSeries.toString(),
      icon: Layers,
      sub: 'across all sermons',
      color: 'text-accent-2',
      bg: 'bg-accent-2/8',
    },
    {
      id: 'stat-words',
      label: 'Total Words',
      value: stats.totalWords.toLocaleString(),
      icon: AlignJustify,
      sub: `~${Math.round(stats.totalWords / Math.max(1, stats.totalSermons)).toLocaleString()} avg/sermon`,
      color: 'text-gold-light',
      bg: 'bg-warn-amber/8',
    },
    {
      id: 'stat-last',
      label: 'Last Preached',
      value: stats.lastPreachedOn ?? '—',
      icon: Calendar,
      sub: stats.lastPreachedOn ? 'most recent pulpit date' : 'no sermons preached yet',
      color: 'text-ok-green',
      bg: 'bg-ok-green/8',
    },
  ];

  return (
    <div className="grid grid-cols-2 xl:grid-cols-4 gap-4">
      {cards.map((card) => {
        const CardIcon = card.icon;
        return (
          <div key={card.id} className={`card-panel ${card.bg} border-border`}>
            <div className="flex items-center justify-between mb-2">
              <span className="text-2xs font-mono-data uppercase tracking-widest text-fg-dim">
                {card.label}
              </span>
              <CardIcon size={14} className={card.color} />
            </div>
            <p className={`text-3xl font-700 font-mono-data ${card.color} leading-none mb-1`}>
              {card.value}
            </p>
            <p className="text-2xs text-fg-dim">{card.sub}</p>
          </div>
        );
      })}
    </div>
  );
}