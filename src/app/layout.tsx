import React from 'react';
import type { Metadata, Viewport } from 'next';
import { Geist, IBM_Plex_Mono } from 'next/font/google';
import '../styles/tailwind.css';

const geist = Geist({
  subsets: ['latin'],
  weight: ['400', '500', '600', '700'],
  variable: '--font-geist',
  display: 'swap',
});

const ibmPlexMono = IBM_Plex_Mono({
  subsets: ['latin'],
  weight: ['400', '500', '600'],
  variable: '--font-ibm-plex-mono',
  display: 'swap',
});

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
};

export const metadata: Metadata = {
  title: 'SermonStudio — Expository Sermon Writing Environment',
  description:
    'A portable, offline-first sermon writing environment for expository preachers. Integrated Bible study, linter, archive, and export tools.',
  icons: {
    icon: [{ url: '/favicon.ico', type: 'image/x-icon' }],
  },
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={`${geist.variable} ${ibmPlexMono.variable} dark`}>
      <body className={geist.className}>{children}
</body>
    </html>
  );
}