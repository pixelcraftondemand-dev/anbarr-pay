import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { ApiError } from '../api/client';
import type { TransferResponse, TopupResponse } from '../api/types';
import { request } from '../api/requests';
import { Money } from '../components/Money';
import { StatusPill } from '../components/StatusPill';
import { ConfirmSheet } from '../components/ConfirmSheet';
import { ScreenState } from '../components/ScreenState';
import { useOnline } from '../hooks/useOnline';

type Flow = 'transfer' | 'topup';

type State =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; flow: Flow; tx: TransferResponse | TopupResponse };

/**
 * Dispute reasons match the case intake the support/fraud docs describe
 * (docs/ux-flows.md §3.6: reason → explanation → evidence → submit). The API
 * contract for dispute creation is still being finalized, so the submit call
 * is performed against POST /v1/disputes and any contract mismatch surfaces
 * as the server's own error — nothing is faked client-side.
 */
const DISPUTE_REASONS = [
  { value: 'unauthorized', label: "I didn't authorize this payment" },
  { value: 'not_received', label: 'The recipient never got the money' },
  { value: 'wrong_amount', label: 'The amount is wrong' },
  { value: 'merchant_dispute', label: 'Problem with a merchant purchase' },
  { value: 'other', label: 'Something else' },
];

export function TransactionDetail() {
  const { id } = useParams<{ id: string }>();
  const online = useOnline();

  const [state, setState] = useState<State>({ kind: 'loading' });
  const [disputeOpen, setDisputeOpen] = useState(false);
  const [reason, setReason] = useState(DISPUTE_REASONS[0].value);
  const [explanation, setExplanation] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [disputeDone, setDisputeDone] = useState(false);
  const [disputeError, setDisputeError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!id) return;
    setState({ kind: 'loading' });
    try {
      // Transfers and topups are the two user-facing flows with a stable
      // GET-by-id today (docs/api.md §4/§6). Try transfer first; a 404 falls
      // through to topup.
      try {
        const tx = await request<TransferResponse>(`/transfers/${id}`);
        setState({ kind: 'ready', flow: 'transfer', tx });
        return;
      } catch (err) {
        if (!(err instanceof ApiError) || err.status !== 404) throw err;
      }
      const tx = await request<TopupResponse>(`/topups/${id}`);
      setState({ kind: 'ready', flow: 'topup', tx });
    } catch (err) {
      setState({
        kind: 'error',
        message:
          err instanceof ApiError && err.status === 404
            ? 'Transaction not found.'
            : err instanceof ApiError
              ? err.message
              : 'Could not load this transaction.',
      });
    }
  }, [id, request]);

  useEffect(() => {
    if (online) void load();
  }, [load, online]);

  const submitDispute = async () => {
    setSubmitting(true);
    setDisputeError(null);
    try {
      await request('/disputes', {
        method: 'POST',
        idempotencyKey: crypto.randomUUID(),
        body: {
          transaction_id: id,
          reason,
          explanation: explanation.trim() || undefined,
        },
      });
      setDisputeDone(true);
      setDisputeOpen(false);
    } catch (err) {
      setDisputeError(
        err instanceof ApiError ? err.message : 'Could not submit the dispute. Try again.',
      );
    } finally {
      setSubmitting(false);
    }
  };

  if (!online) return <ScreenState kind="offline" onRetry={() => void load()} />;
  if (state.kind === 'loading') return <ScreenState kind="loading" />;
  if (state.kind === 'error') {
    return <ScreenState kind="error" message={state.message} onRetry={() => void load()} />;
  }

  const { flow, tx } = state;
  const currency = ('currency' in tx ? tx.currency : 'SLE') as 'SLE' | 'USD';
  const amountMinor =
    'amount_minor' in tx
      ? (tx.amount_minor as number)
      : ('total_minor' in tx && tx.total_minor !== undefined
          ? tx.total_minor
          : undefined);
  const status = tx.status;
  const disputed = status === 'DISPUTED';
  const canDispute = ['COMPLETED', 'PENDING', 'PROCESSING'].includes(status.toUpperCase());

  return (
    <div className="px" style={{ paddingTop: 16, paddingBottom: 24 }}>
      <button type="button" className="link" onClick={() => history.back()}>
        ← Back
      </button>
      <h2 style={{ margin: '10px 0 12px' }}>
        {flow === 'transfer' ? 'Transfer' : 'Top-up'} receipt
      </h2>

      <div className={`pending-state`} style={{ minHeight: 120, padding: 0 }}>
        <div className="ring" style={{ width: 54, height: 54 }}>
          {status.toUpperCase() === 'COMPLETED' ? (
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--green-700)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M5 12l5 5 9-11" />
            </svg>
          ) : (
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--gold-800)" strokeWidth="2">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 7v5l3 3" />
            </svg>
          )}
        </div>
        <StatusPill status={status} />
      </div>

      <div className="card-flat" style={{ padding: '4px 14px', marginTop: 14 }}>
        <div className="review-row">
          <span className="rv-label muted">Amount</span>
          <span className="rv-value">
            {amountMinor !== undefined ? (
              <Money minor={amountMinor} currency={currency} />
            ) : (
              '—'
            )}
          </span>
        </div>
        {'fee_minor' in tx && tx.fee_minor !== undefined && (
          <div className="review-row">
            <span className="rv-label muted">Fee</span>
            <span className="rv-value">
              <Money minor={tx.fee_minor} currency={currency} />
            </span>
          </div>
        )}
        {'tax_minor' in tx && tx.tax_minor !== undefined && (
          <div className="review-row">
            <span className="rv-label muted">Government tax</span>
            <span className="rv-value">
              <Money minor={tx.tax_minor} currency={currency} />
            </span>
          </div>
        )}
        {'total_minor' in tx && tx.total_minor !== undefined && (
          <div className="review-row" style={{ fontWeight: 700 }}>
            <span>Total</span>
            <span>
              <Money minor={tx.total_minor} currency={currency} />
            </span>
          </div>
        )}
        {'rail_reference' in tx && tx.rail_reference && (
          <div className="review-row">
            <span className="rv-label muted">Provider reference</span>
            <span className="rv-value">{tx.rail_reference}</span>
          </div>
        )}
        <div className="review-row">
          <span className="rv-label muted">Reference</span>
          <span className="rv-value" style={{ fontFamily: 'monospace', fontSize: '.8rem' }}>
            {tx.id}
          </span>
        </div>
        {'journal_id' in tx && tx.journal_id && (
          <div className="review-row">
            <span className="rv-label muted">Ledger journal</span>
            <span className="rv-value" style={{ fontFamily: 'monospace', fontSize: '.8rem' }}>
              {tx.journal_id}
            </span>
          </div>
        )}
      </div>

      <p className="faint" style={{ fontSize: '.72rem', marginTop: 10, lineHeight: 1.5 }}>
        Everything on this receipt comes from AmberPay's ledger and provider records. The original
        financial record is never edited — refunds and reversals are new events.
      </p>

      {disputeDone ? (
        <div className="card" style={{ padding: 14 }}>
          <p style={{ margin: 0, fontSize: '.85rem' }}>
            <strong>Dispute submitted.</strong> We've opened a case for this transaction. You can
            follow its status in <em>Support</em>; the transaction is marked until the case
            resolves.
          </p>
        </div>
      ) : (
        canDispute && (
          <button
            type="button"
            className="button button-ghost"
            style={{ width: '100%', marginTop: 16 }}
            onClick={() => setDisputeOpen(true)}
          >
            Report a problem with this transaction
          </button>
        )
      )}
      {disputed && (
        <p className="muted" style={{ fontSize: '.8rem' }}>
          This transaction is under dispute. Funds may be held until the case resolves.
        </p>
      )}

      <ConfirmSheet
        open={disputeOpen}
        title="Report a problem"
        body="Tell us what went wrong. Opening a dispute never changes the original financial record — our team reviews the case and may place a hold on the funds."
        confirmLabel={submitting ? 'Submitting…' : 'Submit dispute'}
        onConfirm={() => void submitDispute()}
        onCancel={() => setDisputeOpen(false)}
      >
        <label style={{ marginBottom: 10 }}>
          Reason
          <select value={reason} onChange={(e) => setReason(e.target.value)}>
            {DISPUTE_REASONS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
        <label style={{ marginBottom: 4 }}>
          Explanation (optional)
          <textarea
            value={explanation}
            onChange={(e) => setExplanation(e.target.value)}
            rows={3}
            maxLength={500}
            style={{ font: 'inherit', padding: '.6rem .75rem', borderRadius: 'var(--radius-m)', border: '1px solid var(--color-border)', width: '100%' }}
          />
        </label>
        {disputeError && (
          <p className="form-error" role="alert">
            {disputeError}
          </p>
        )}
      </ConfirmSheet>
    </div>
  );
}
