// Standard screen states per docs/ux-flows.md §53. Every screen must render
// all of these — never only the happy path.

export type ScreenStateKind = 'loading' | 'empty' | 'error' | 'offline' | 'maintenance';

export function ScreenState({
  kind,
  message,
  onRetry,
}: {
  kind: ScreenStateKind;
  message?: string;
  onRetry?: () => void;
}) {
  const defaults: Record<ScreenStateKind, { title: string; body: string }> = {
    loading: { title: 'Loading…', body: 'Please wait.' },
    empty: { title: 'Nothing here yet', body: 'There is nothing to show right now.' },
    error: { title: 'Something went wrong', body: 'Please try again. If it persists, contact support and mention the request ID.' },
    offline: {
      title: "You're offline",
      body: 'Balances and transactions may be out of date. Money-moving actions are disabled until you reconnect.',
    },
    maintenance: {
      title: 'AmberPay is undergoing maintenance',
      body: 'Money movement is temporarily disabled. We will be back shortly.',
    },
  };

  const copy = { ...defaults[kind], ...(message ? { body: message } : {}) };

  return (
    <div className="screen-state" role={kind === 'error' ? 'alert' : 'status'} aria-live="polite">
      <h2>{copy.title}</h2>
      <p>{copy.body}</p>
      {onRetry && kind !== 'loading' && (
        <button type="button" onClick={onRetry} className="button">
          Retry
        </button>
      )}
    </div>
  );
}