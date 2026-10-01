/**
 * The transaction state machine (§4.4) made visible (§13.5): every state has
 * one pill color and one user-facing sentence. The UI never renders a
 * money-movement action as instantly successful — an uncertain backend state
 * is shown as pending/confirming, never as success or failure.
 */

const STATES = {
  COMPLETED: { tone: 'completed', label: 'Completed' },
  PENDING: { tone: 'pending', label: "Pending — we'll notify you" },
  PROCESSING: { tone: 'pending', label: 'Processing' },
  HELD_FOR_REVIEW: { tone: 'held', label: 'Under review' },
  FAILED: { tone: 'failed', label: 'Failed' },
  REVERSED: { tone: 'failed', label: 'Reversed' },
  REFUNDED: { tone: 'held', label: 'Refunded' },
  DISPUTED: { tone: 'held', label: 'Disputed' },
  UNKNOWN: { tone: 'pending', label: "We're confirming this with the provider" },
} as const;

type KnownStatus = keyof typeof STATES;

function isKnownStatus(status: string): status is KnownStatus {
  return Object.prototype.hasOwnProperty.call(STATES, status.toUpperCase());
}

export function StatusPill({ status }: { status: string }) {
  const state = isKnownStatus(status)
    ? STATES[status.toUpperCase() as KnownStatus]
    // An unrecognized status is treated as UNKNOWN — never invented into a
    // success or failure the backend did not report (§4.4).
    : STATES.UNKNOWN;

  return (
    <span className={`pill pill-${state.tone}`} data-status={status.toUpperCase()}>
      {state.label}
    </span>
  );
}
