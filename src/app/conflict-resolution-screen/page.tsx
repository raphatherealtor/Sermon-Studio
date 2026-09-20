'use client';
import { useEffect } from 'react';
import { useRouter } from 'next/navigation';

// Conflict resolution is now handled inline in the editor workspace.
// This route redirects to the editor where the conflict banner appears.
export default function ConflictResolutionRedirect() {
  const router = useRouter();
  useEffect(() => {
    router?.replace('/');
  }, [router]);
  return null;
}