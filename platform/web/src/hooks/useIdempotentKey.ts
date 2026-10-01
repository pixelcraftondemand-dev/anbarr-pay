import { useCallback, useState } from 'react';
import { v4 as uuidv4 } from 'uuid';

/**
 * One idempotency key per submission ATTEMPT, reused across every retry and
 * refresh of that attempt — so double-clicks and network retries can never
 * double-post (docs/ux-flows.md §4, docs/transaction-state-machine.md §5).
 *
 * Call `reset()` only when the user starts a genuinely NEW attempt (e.g. the
 * form is cleared after a definitive failure).
 */
export function useIdempotentKey(): { key: string; reset: () => void } {
  const [key, setKey] = useState(() => uuidv4());
  const reset = useCallback(() => setKey(uuidv4()), []);
  return { key, reset };
}