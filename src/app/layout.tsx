import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'Sermon Studio',
  description: 'Local-first sermon research and writing studio',
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
