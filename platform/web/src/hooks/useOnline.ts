import { useEffect, useState } from 'react';

/** Tracks navigator.onLine. Money-moving actions are disabled while offline
 *  (docs/ux-flows.md §2 — nothing is queued silently client-side). */
export function useOnline(): boolean {
  const [online, setOnline] = useState<boolean>(() => navigator.onLine);
  useEffect(() => {
    const up = () => setOnline(true);
    const down = () => setOnline(false);
    window.addEventListener('online', up);
    window.addEventListener('offline', down);
    return () => {
      window.removeEventListener('online', up);
      window.removeEventListener('offline', down);
    };
  }, []);
  return online;
}