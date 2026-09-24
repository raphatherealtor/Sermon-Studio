'use client';
import React, { useEffect, useState, useMemo } from 'react';
import dynamic from 'next/dynamic';
import Link from 'next/link';
import { useBackend } from '@/lib/backend/BackendContext';
import { Search, X, ChevronUp, ChevronDown, Filter, ExternalLink, Archive, BookOpen, AlertTriangle, BarChart2, TrendingUp, FileText, Activity,  } from 'lucide-react';
import type { SermonSummary, ArchiveStats, IllustrationFatigueResult } from '@/lib/backend/types';
import ArchiveStatsCards from './ArchiveStatsCards';
import IllustrationFatiguePanel from './IllustrationFatiguePanel';
import RkMark from '@/app/components/common/RkMark';

const SermonsByMonthChart = dynamic(() => import('./SermonsByMonthChart'), { ssr: false });

type SortField = 'title' | 'scripture' | 'series' | 'status' | 'wordCount' | 'updatedAt' | 'preachedOn';
type SortDir = 'asc' | 'desc';

const STATUS_CLASSES: Record<string, string> = {
  draft: 'badge-draft',
  'in-progress': 'badge-progress',
  reviewed: 'badge-reviewed',
  preached: 'badge-preached',
  archived: 'badge-archived',
};

const ALL_STATUSES = ['draft', 'in-progress', 'reviewed', 'preached', 'archived'];

const ACTIVITY_ICONS: Record<string, React.ElementType> = {
  created: FileText,
  edited: FileText,
  preached: BookOpen,
  exported: Archive,
  archived: Archive,
};

const ACTIVITY_COLORS: Record<string, string> = {
  created: 'text-accent',
  edited: 'text-fg-dim',
  preached: 'text-ok-green',
  exported: 'text-gold-light',
  archived: 'text-fg-dim',
};

export default function ArchiveOverviewContent() {
  const backend = useBackend();

  const [sermons, setSermons] = useState<SermonSummary[]>([]);
  const [stats, setStats] = useState<ArchiveStats | null>(null);
  const [fatigue, setFatigue] = useState<IllustrationFatigueResult[]>([]);
  const [loading, setLoading] = useState(true);

  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState<string>('all');
  const [seriesFilter, setSeriesFilter] = useState<string>('all');
  const [sortField, setSortField] = useState<SortField>('updatedAt');
  const [sortDir, setSortDir] = useState<SortDir>('desc');
  const [page, setPage] = useState(1);
  const PAGE_SIZE = 8;

  useEffect(() => {
    let cancelled = false;
    async function load() {
      const [list, s, f] = await Promise.all([
        backend.listSermons(),
        backend.getArchiveStats(),
        backend.getIllustrationFatigue(),
      ]);
      if (!cancelled) {
        setSermons(list);
        setStats(s);
        setFatigue(f);
        setLoading(false);
      }
    }
    load();
    return () => { cancelled = true; };
  }, [backend]);

  const seriesList = useMemo(() => {
    const s = new Set<string>();
    sermons.forEach((sr) => { if (sr.series) s.add(sr.series); });
    return Array.from(s).sort();
  }, [sermons]);

  const filtered = useMemo(() => {
    let list = [...sermons];
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(
        (s) =>
          s.title.toLowerCase().includes(q) ||
          s.scripture.toLowerCase().includes(q) ||
          (s.series?.toLowerCase().includes(q) ?? false) ||
          s.tags.some((t) => t.toLowerCase().includes(q))
      );
    }
    if (statusFilter !== 'all') list = list.filter((s) => s.status === statusFilter);
    if (seriesFilter !== 'all') list = list.filter((s) => s.series === seriesFilter);
    list.sort((a, b) => {
      let av: string | number = '';
      let bv: string | number = '';
      if (sortField === 'wordCount') { av = a.wordCount; bv = b.wordCount; }
      else if (sortField === 'title') { av = a.title; bv = b.title; }
      else if (sortField === 'scripture') { av = a.scripture; bv = b.scripture; }
      else if (sortField === 'series') { av = a.series ?? ''; bv = b.series ?? ''; }
      else if (sortField === 'status') { av = a.status; bv = b.status; }
      else if (sortField === 'updatedAt') { av = a.updatedAt; bv = b.updatedAt; }
      else if (sortField === 'preachedOn') { av = a.preachedOn ?? ''; bv = b.preachedOn ?? ''; }
      if (av < bv) return sortDir === 'asc' ? -1 : 1;
      if (av > bv) return sortDir === 'asc' ? 1 : -1;
      return 0;
    });
    return list;
  }, [sermons, search, statusFilter, seriesFilter, sortField, sortDir]);

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const paginated = filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  const handleSort = (field: SortField) => {
    if (sortField === field) setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    else { setSortField(field); setSortDir('asc'); }
    setPage(1);
  };

  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortField !== field) return <ChevronUp size={9} className="text-fg-dim opacity-30" />;
    return sortDir === 'asc' ? <ChevronUp size={9} className="text-accent" /> : <ChevronDown size={9} className="text-accent" />;
  };

  if (loading) {
    return (
      <div className="p-6 space-y-4">
        <div className="grid grid-cols-4 gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={`skel-stat-${i}`} className="animate-pulse bg-elevated rounded h-24" />
          ))}
        </div>
        <div className="animate-pulse bg-elevated rounded h-64" />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-6 py-4 border-b border-border bg-panel flex-shrink-0">
        <Archive size={15} className="text-accent" />
        <h1 className="text-lg font-600 text-fg">Sermon Archive</h1>
        <span className="text-xs font-mono-data text-fg-dim ml-2">{sermons.length} sermons</span>
        <span className="ml-auto">
          <RkMark />
        </span>
        {stats?.unresolvedLintCount && stats.unresolvedLintCount > 0 && (
          <span className="flex items-center gap-1 text-2xs font-mono-data text-warn-amber bg-warn/8 px-2 py-0.5 rounded border border-warn/20 ml-2">
            <AlertTriangle size={9} /> {stats.unresolvedLintCount} unresolved findings
          </span>
        )}
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-screen-2xl mx-auto px-6 py-6 space-y-6">

          {/* Stats cards */}
          {stats && <ArchiveStatsCards stats={stats} />}

          {/* Charts row */}
          <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
            <div className="xl:col-span-2 card-panel">
              <h2 className="text-sm font-600 text-fg mb-4 flex items-center gap-2">
                <BarChart2 size={13} className="text-accent" /> Sermons by Month
              </h2>
              {stats && <SermonsByMonthChart data={stats.sermonsByMonth} />}
            </div>
            <IllustrationFatiguePanel items={fatigue} />
          </div>

          {/* Second row: Book frequency + Length distribution + Recent activity */}
          <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">

            {/* Scripture book frequency */}
            {stats?.sermonsByBook && stats.sermonsByBook.length > 0 && (
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <BookOpen size={13} className="text-accent" /> Scripture Books
                </h2>
                <div className="space-y-1.5">
                  {stats.sermonsByBook.slice(0, 8).map((item) => (
                    <div key={`book-${item.book}`} className="flex items-center gap-2">
                      <span className="text-xs font-mono-data text-fg-dim w-20 flex-shrink-0">{item.book}</span>
                      <div className="flex-1 h-1.5 bg-elevated rounded-full overflow-hidden">
                        <div
                          className="h-1.5 bg-accent rounded-full"
                          style={{ width: `${(item.count / (stats.sermonsByBook?.[0]?.count || 1)) * 100}%` }}
                        />
                      </div>
                      <span className="text-2xs font-mono-data text-fg-dim w-4 text-right">{item.count}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Length distribution */}
            {stats?.sermonLengthDistribution && (
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <TrendingUp size={13} className="text-accent" /> Length Distribution
                </h2>
                <div className="space-y-1.5">
                  {stats.sermonLengthDistribution.map((item) => {
                    const maxCount = Math.max(...stats.sermonLengthDistribution!.map((d) => d.count));
                    return (
                      <div key={`dist-${item.bucket}`} className="flex items-center gap-2">
                        <span className="text-2xs font-mono-data text-fg-dim w-20 flex-shrink-0">{item.bucket}w</span>
                        <div className="flex-1 h-1.5 bg-elevated rounded-full overflow-hidden">
                          <div
                            className="h-1.5 bg-accent-2 rounded-full"
                            style={{ width: `${maxCount > 0 ? (item.count / maxCount) * 100 : 0}%` }}
                          />
                        </div>
                        <span className="text-2xs font-mono-data text-fg-dim w-4 text-right">{item.count}</span>
                      </div>
                    );
                  })}
                </div>
                {stats.averageWordCount && (
                  <p className="text-2xs font-mono-data text-fg-dim mt-3 pt-2 border-t border-border">
                    Avg: <span className="text-fg">{stats.averageWordCount.toLocaleString()}w</span>
                  </p>
                )}
              </div>
            )}

            {/* Recent activity */}
            {stats?.recentActivity && stats.recentActivity.length > 0 && (
              <div className="card-panel">
                <h2 className="text-sm font-600 text-fg mb-3 flex items-center gap-2">
                  <Activity size={13} className="text-accent" /> Recent Activity
                </h2>
                <div className="space-y-2">
                  {stats.recentActivity.map((item, i) => {
                    const ActivityIcon = ACTIVITY_ICONS[item.action] || FileText;
                    return (
                      <div key={`activity-${i}`} className="flex items-start gap-2">
                        <ActivityIcon size={11} className={`mt-0.5 flex-shrink-0 ${ACTIVITY_COLORS[item.action]}`} />
                        <div className="min-w-0 flex-1">
                          <p className="text-xs text-fg truncate">{item.sermonTitle}</p>
                          <p className="text-2xs font-mono-data text-fg-dim">
                            {item.action} · {item.timestamp.slice(0, 10)}
                          </p>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </div>

          {/* Sermon table */}
          <div className="card-panel">
            {/* Filters */}
            <div className="flex items-center gap-3 mb-4 flex-wrap">
              <div className="relative flex-1 min-w-48">
                <Search size={11} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-dim" />
                <input
                  type="text"
                  value={search}
                  onChange={(e) => { setSearch(e.target.value); setPage(1); }}
                  placeholder="Search title, scripture, tags…"
                  className="input-field pl-7 pr-7 text-xs"
                />
                {search && (
                  <button onClick={() => setSearch('')} className="absolute right-2 top-1/2 -translate-y-1/2 text-fg-dim hover:text-fg">
                    <X size={10} />
                  </button>
                )}
              </div>

              <div className="flex items-center gap-2">
                <Filter size={11} className="text-fg-dim" />
                <select value={statusFilter} onChange={(e) => { setStatusFilter(e.target.value); setPage(1); }} className="input-field text-xs w-36">
                  <option value="all">All statuses</option>
                  {ALL_STATUSES.map((s) => <option key={`status-opt-${s}`} value={s}>{s}</option>)}
                </select>
                <select value={seriesFilter} onChange={(e) => { setSeriesFilter(e.target.value); setPage(1); }} className="input-field text-xs w-48">
                  <option value="all">All series</option>
                  <option value="">Standalone</option>
                  {seriesList.map((s) => <option key={`series-opt-${s}`} value={s}>{s}</option>)}
                </select>
              </div>

              <span className="text-2xs font-mono-data text-fg-dim ml-auto">
                {filtered.length} result{filtered.length !== 1 ? 's' : ''}
              </span>
            </div>

            {/* Table */}
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-border">
                    {([
                      { field: 'title', label: 'Title' },
                      { field: 'scripture', label: 'Scripture' },
                      { field: 'series', label: 'Series' },
                      { field: 'status', label: 'Status' },
                      { field: 'wordCount', label: 'Words' },
                      { field: 'updatedAt', label: 'Updated' },
                      { field: 'preachedOn', label: 'Preached' },
                    ] as { field: SortField; label: string }[]).map((col) => (
                      <th
                        key={`col-${col.field}`}
                        onClick={() => handleSort(col.field)}
                        className="text-left px-3 py-2.5 text-2xs font-mono-data uppercase tracking-wider text-fg-dim cursor-pointer hover:text-fg transition-colors duration-100 whitespace-nowrap select-none"
                      >
                        <span className="flex items-center gap-1">
                          {col.label}
                          <SortIcon field={col.field} />
                        </span>
                      </th>
                    ))}
                    <th className="px-3 py-2.5 text-right text-2xs font-mono-data uppercase tracking-wider text-fg-dim">Open</th>
                  </tr>
                </thead>
                <tbody>
                  {paginated.length === 0 ? (
                    <tr>
                      <td colSpan={8} className="text-center py-12">
                        <Archive size={22} className="text-fg-dim mx-auto mb-2" />
                        <p className="text-xs text-fg-dim">No sermons match your filters</p>
                      </td>
                    </tr>
                  ) : (
                    paginated.map((sermon, rowIdx) => (
                      <tr
                        key={`row-${sermon.id}`}
                        className={`border-b border-border/30 hover:bg-elevated transition-colors duration-100 group ${rowIdx % 2 === 0 ? '' : 'bg-panel/50'}`}
                      >
                        <td className="px-3 py-2.5 font-500 text-fg max-w-48 truncate">
                          {sermon.isPinned && <span className="text-fg-dim mr-1">📌</span>}
                          {sermon.title}
                        </td>
                        <td className="px-3 py-2.5 font-mono-data text-accent whitespace-nowrap">{sermon.scripture}</td>
                        <td className="px-3 py-2.5 text-fg-dim max-w-36 truncate">
                          {sermon.series ?? <span className="text-fg-dim/40 italic">—</span>}
                        </td>
                        <td className="px-3 py-2.5">
                          <span className={`status-badge ${STATUS_CLASSES[sermon.status]}`}>{sermon.status}</span>
                        </td>
                        <td className="px-3 py-2.5 font-mono-data text-fg/70 text-right">{sermon.wordCount.toLocaleString()}</td>
                        <td className="px-3 py-2.5 font-mono-data text-fg-dim whitespace-nowrap">{sermon.updatedAt.slice(0, 10)}</td>
                        <td className="px-3 py-2.5 font-mono-data whitespace-nowrap">
                          {sermon.preachedOn ? (
                            <span className="text-ok-green">{sermon.preachedOn}</span>
                          ) : (
                            <span className="text-fg-dim/40">—</span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 text-right">
                          <Link href={`/?sermonId=${encodeURIComponent(sermon.id)}`} className="opacity-0 group-hover:opacity-100 transition-opacity text-fg-dim hover:text-accent" title={`Open "${sermon.title}"`}>
                            <ExternalLink size={11} />
                          </Link>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>

            {/* Pagination */}
            <div className="flex items-center justify-between mt-4 pt-3 border-t border-border">
              <p className="text-2xs font-mono-data text-fg-dim">
                {Math.min((page - 1) * PAGE_SIZE + 1, filtered.length)}–{Math.min(page * PAGE_SIZE, filtered.length)} of {filtered.length}
              </p>
              <div className="flex items-center gap-1">
                <button onClick={() => setPage((p) => Math.max(1, p - 1))} disabled={page === 1} className="btn-ghost py-1 px-2 text-xs disabled:opacity-30">←</button>
                {Array.from({ length: Math.min(totalPages, 7) }).map((_, i) => (
                  <button
                    key={`page-${i + 1}`}
                    onClick={() => setPage(i + 1)}
                    className={`w-7 h-7 rounded text-xs font-mono-data transition-all duration-100 ${page === i + 1 ? 'bg-accent text-bg' : 'text-fg-dim hover:bg-elevated hover:text-fg'}`}
                  >
                    {i + 1}
                  </button>
                ))}
                <button onClick={() => setPage((p) => Math.min(totalPages, p + 1))} disabled={page === totalPages} className="btn-ghost py-1 px-2 text-xs disabled:opacity-30">→</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
