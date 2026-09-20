'use client';
import React from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  BookOpen, Archive, FileOutput, Settings,
  ChevronLeft, ChevronRight, BookMarked, Code2,
} from 'lucide-react';
import AppLogo from './ui/AppLogo';

interface NavItem {
  href: string;
  label: string;
  icon: React.ElementType;
  badge?: number;
  devOnly?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { href: '/', label: 'Editor', icon: BookOpen },
  { href: '/archive-overview', label: 'Archive', icon: Archive },
  { href: '/export-screen', label: 'Export', icon: FileOutput },
  { href: '/settings-index-management', label: 'Settings', icon: Settings },
];

const DEV_ITEMS: NavItem[] = [
  { href: '/codec-test', label: 'Codec Test', icon: Code2, devOnly: true },
];

interface SidebarProps {
  collapsed: boolean;
  onToggleCollapse: () => void;
}

export default function Sidebar({ collapsed, onToggleCollapse }: SidebarProps) {
  const pathname = usePathname();

  const renderNavItem = (item: NavItem) => {
    const ItemIcon = item.icon;
    const isActive = pathname === item.href;
    return (
      <Link
        key={`nav-${item.href}`}
        href={item.href}
        title={collapsed ? item.label : undefined}
        className={[
          'flex items-center gap-2.5 rounded transition-all duration-150 relative group',
          collapsed ? 'justify-center px-0 py-2.5' : 'px-2.5 py-2',
          isActive
            ? 'bg-accent/10 text-accent' :'text-fg-dim hover:bg-elevated hover:text-fg',
        ].join(' ')}
      >
        <ItemIcon size={15} className="flex-shrink-0" />
        {!collapsed && (
          <span className="text-sm font-500 whitespace-nowrap">{item.label}</span>
        )}
        {item.badge && item.badge > 0 && (
          <span className={[
            'flex-shrink-0 text-2xs font-mono-data font-600 rounded-full flex items-center justify-center',
            collapsed
              ? 'absolute top-1 right-1 w-3.5 h-3.5 bg-warn-amber text-bg' :'ml-auto w-4 h-4 bg-warn-amber text-bg',
          ].join(' ')}>
            {item.badge}
          </span>
        )}
        {collapsed && (
          <span className="pointer-events-none absolute left-full ml-2 px-2 py-1 rounded bg-elevated text-fg text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-150 z-50 border border-border">
            {item.label}
          </span>
        )}
      </Link>
    );
  };

  return (
    <aside
      className="flex flex-col flex-shrink-0 border-r border-border bg-panel transition-all duration-300 ease-in-out"
      style={{ width: collapsed ? 48 : 192 }}
    >
      {/* Logo */}
      <div
        className="flex items-center border-b border-border flex-shrink-0 overflow-hidden"
        style={{ height: 48, padding: collapsed ? '0 10px' : '0 12px' }}
      >
        <div className="flex items-center gap-2 min-w-0">
          <AppLogo size={26} />
          {!collapsed && (
            <span className="font-sans text-sm font-700 text-accent whitespace-nowrap tracking-tight overflow-hidden">
              SermonStudio
            </span>
          )}
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 py-2 flex flex-col gap-0.5 px-1.5 overflow-y-auto overflow-x-hidden">
        {!collapsed && (
          <p className="text-2xs font-mono-data uppercase tracking-widest text-fg-dim px-2 pb-1 pt-1">
            Workspace
          </p>
        )}
        {NAV_ITEMS.map(renderNavItem)}

        {/* Dev tools section */}
        {!collapsed && (
          <p className="text-2xs font-mono-data uppercase tracking-widest text-fg-dim px-2 pb-1 pt-3">
            Dev Tools
          </p>
        )}
        {collapsed && <div className="border-t border-border/50 my-1 mx-1" />}
        {DEV_ITEMS.map(renderNavItem)}
      </nav>

      {/* Bottom: collapse toggle + version */}
      <div className="border-t border-border p-1.5 flex flex-col gap-1">
        <button
          onClick={onToggleCollapse}
          className="flex items-center justify-center w-full py-2 rounded text-fg-dim hover:bg-elevated hover:text-fg transition-all duration-150"
          title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {collapsed ? <ChevronRight size={13} /> : (
            <span className="flex items-center gap-2 text-xs text-fg-dim">
              <ChevronLeft size={13} />
              <span>Collapse</span>
            </span>
          )}
        </button>
        {!collapsed && (
          <div className="flex items-center gap-1.5 px-2 py-1">
            <BookMarked size={9} className="text-fg-dim" />
            <span className="text-2xs font-mono-data text-fg-dim">v0.9.0-dev</span>
          </div>
        )}
      </div>
    </aside>
  );
}