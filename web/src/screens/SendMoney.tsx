import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { Currency, TransferRequest } from '../api/types';
import { verifyPin } from '../api/pin';
import { Money, formatMinorUnits } from '../components/Money';
import { StatusPill } from '../components/StatusPill';
import { Keypad, PinDots } from '../components/Keypad';
import { ScreenState } from '../components/ScreenState';
import { BeneficiaryPicker } from '../components/BeneficiaryPicker';
import { useTransferSubmit } from '../hooks/useTransferSubmit';
import { useOnline } from '../hooks/useOnline';

type Step =
  | { kind: 'recipient' }
  | { kind: 'amount' }
  | { kind: 'review' }
  | { kind: 'sent' };

const MAX_AMOUNT_MINOR = 99_999_999_999;

export function SendMoney() {
  const navigate = useNavigate();
  const online = useOnline();
  // PIN authorization goes through the Core API; while it's unreachable the
  // flow lands in its honest pending/error states.
  const { state: submitState, submit, retry, startOver, pinToken } = useTransferSubmit(
    null,
    verifyPin,
  );

  const [step, setStep] = useState<Step>({ kind: 'recipient' });
  const [recipient, setRecipient] = useState('');
  const [currency, setCurrency] = useState<Currency>('SLE');
  const [amountMinor, setAmountMinor] = useState(0);
  const [note, setNote] = useState('');
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [pin, setPin] = useState('');
  const [pinError, setPinError] = useState<string | null>(null);

  if (!online) {
    return <ScreenState kind="offline" />;
  }

  const buildRequest = (): TransferRequest => ({
    recipient_email_or_phone: recipient.trim(),
    amount_minor: amountMinor,
    currency,
    note: note.trim() || undefined,
  });

  // ----------------------------------------------------------------
  // Step 1: recipient
  // ----------------------------------------------------------------
  if (step.kind === 'recipient') {
    const valid = /^([^\s@]+@[^\s@]+\.[^\s@]+|\+?[0-9]{7,15})$/.test(recipient.trim());
    return (
      <div className="px" style={{ paddingTop: 16, flex: 1, display: 'flex', flexDirection: 'column' }}>
        <h2 style={{ margin: '0 0 4px' }}>Send money</h2>
        <div className="faint" style={{ fontSize: '.8rem', marginBottom: 12 }}>
          Who are you paying?
        </div>
        {/* Saved recipients come from the server's beneficiary list (§3.4).
            It hides itself when the API isn't available — the input below
            always works. */}
        <BeneficiaryPicker onSelect={setRecipient} />
        <label style={{ marginBottom: 12 }}>
          Recipient phone or email
          <input
            value={recipient}
            onChange={(e) => setRecipient(e.target.value)}
            placeholder="+232 76 000000 or name@example.com"
            autoComplete="off"
            inputMode="email"
          />
        </label>
        <label style={{ marginBottom: 4 }}>
          Currency
          <select value={currency} onChange={(e) => setCurrency(e.target.value as Currency)}>
            <option value="SLE">SLE — Sierra Leone</option>
            <option value="USD">USD</option>
          </select>
        </label>
        {fieldError && (
          <p className="form-error" role="alert" style={{ marginTop: 8 }}>
            {fieldError}
          </p>
        )}
        <p className="faint" style={{ fontSize: '.75rem', marginTop: 10 }}>
          The recipient must be a registered AmberPay account.
        </p>
        <div style={{ marginTop: 'auto', paddingBottom: 16, paddingTop: 12 }}>
          <button
            type="button"
            className="button"
            style={{ width: '100%' }}
            disabled={!valid}
            onClick={() => {
              if (!valid) return setFieldError('Enter a valid phone number or email.');
              setFieldError(null);
              setStep({ kind: 'amount' });
            }}
          >
            Continue
          </button>
        </div>
      </div>
    );
  }

  // ----------------------------------------------------------------
  // Step 2: amount keypad
  // ----------------------------------------------------------------
  if (step.kind === 'amount') {
    const enabled = amountMinor > 0;
    return (
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
        <div className="px" style={{ paddingTop: 16 }}>
          <button type="button" className="link" onClick={() => setStep({ kind: 'recipient' })}>
            ← Back
          </button>
          <div className="amount-display">
            <span className="cur">{currency}</span>{' '}
            <span className="num">
              {(amountMinor / 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}
            </span>
            <div className="faint" style={{ fontSize: '.8rem', marginTop: 6 }}>
              To {recipient}
            </div>
          </div>
        </div>
        <div className="px" style={{ margin: '0 20px' }}>
          <div className="card-flat">
            <div className="breakdown-row">
              <span className="muted">You send</span>
              <span>{formatMinorUnits(amountMinor, currency)}</span>
            </div>
            <div className="breakdown-row">
              <span className="muted">Fee &amp; tax</span>
              <span className="muted">Confirmed by AmberPay when you authorize</span>
            </div>
          </div>
          <label style={{ margin: '12px 0 0' }}>
            Note (optional)
            <input value={note} onChange={(e) => setNote(e.target.value)} maxLength={140} />
          </label>
        </div>
        <div className="px" style={{ marginTop: 14 }}>
          <Keypad
            onDigit={(d) => setAmountMinor((a) => Math.min(a * 10 + d, MAX_AMOUNT_MINOR))}
            onBackspace={() => setAmountMinor((a) => Math.floor(a / 10))}
          />
        </div>
        <div style={{ marginTop: 'auto', paddingBottom: 16, paddingTop: 12 }}>
          <button
            type="button"
            className="button"
            style={{ width: '100%' }}
            disabled={!enabled}
            onClick={() => setStep({ kind: 'review' })}
          >
            Review transfer
          </button>
        </div>
      </div>
    );
  }

  // ----------------------------------------------------------------
  // Submission states owned by the hook (review UI reads them).
  // ----------------------------------------------------------------
  const submitting = submitState.kind === 'submitting';

  if (step.kind === 'review' || step.kind === 'sent') {
    // Terminal/unknown states take over the screen.
    if (submitState.kind === 'pending') {
      return (
        <div className="pending-state" style={{ flex: 1 }}>
          <div className="ring">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--gold-800)" strokeWidth="2">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 7v5l3 3" />
            </svg>
          </div>
          <h2 style={{ margin: 0 }}>Transfer pending</h2>
          <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>
            {formatMinorUnits(submitState.amountMinor, currency)} to {submitState.recipient} is on
            its way. The connection dropped before AmberPay could confirm it — it has not failed.
          </p>
          <span className="pill pill-pending pill-sm">PENDING</span>
          <button
            type="button"
            className="button button-ghost"
            style={{ marginTop: 14, width: '100%' }}
            onClick={() => void retry()}
          >
            Check status again
          </button>
          <button type="button" className="link" onClick={() => navigate('/activity')}>
            View activity instead
          </button>
        </div>
      );
    }

    if (submitState.kind === 'error') {
      return (
        <div className="pending-state" style={{ flex: 1 }}>
          <div className="ring ring-danger">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--red-700)" strokeWidth="2" strokeLinecap="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </div>
          <h2 style={{ margin: 0 }}>Transfer couldn't be completed</h2>
          <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>{submitState.message}</p>
          {pinError && (
            <p className="form-error" role="alert">
              {pinError}
            </p>
          )}
          <button
            type="button"
            className="button"
            style={{ marginTop: 14, width: '100%' }}
            onClick={() => {
              setPin('');
              setStep({ kind: 'review' });
            }}
          >
            Enter PIN again
          </button>
          <button
            type="button"
            className="button button-ghost"
            style={{ width: '100%' }}
            onClick={() => {
              startOver();
              setAmountMinor(0);
              setStep({ kind: 'amount' });
            }}
          >
            Start over
          </button>
        </div>
      );
    }

    if (submitState.kind === 'result') {
      const { result, request: req } = submitState;
      const completed = result.status === 'COMPLETED';
      return (
        <div className="pending-state" style={{ flex: 1 }}>
          <div className="ring">
            {completed ? (
              <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--green-700)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M5 12l5 5 9-11" />
              </svg>
            ) : (
              <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--gold-800)" strokeWidth="2">
                <circle cx="12" cy="12" r="9" />
                <path d="M12 7v5l3 3" />
              </svg>
            )}
          </div>
          <h2 style={{ margin: 0 }}>
            {completed ? 'Transfer confirmed' : "We're confirming your transfer"}
          </h2>
          <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>
            {formatMinorUnits(req.amount_minor, req.currency)} to{' '}
            {req.recipient_email_or_phone}
            {result.total_minor !== undefined && (
              <>
                {' '}· total with fees <Money minor={result.total_minor} currency={req.currency} />
              </>
            )}
          </p>
          <StatusPill status={result.status} />
          <p className="faint" style={{ fontSize: '.72rem', margin: 0 }}>
            Reference {result.id}
            {result.journal_id ? ` · journal ${result.journal_id.slice(0, 8)}` : ''}
          </p>
          <button
            type="button"
            className="button button-ghost"
            style={{ marginTop: 14, width: '100%' }}
            onClick={() => {
              startOver();
              setRecipient('');
              setAmountMinor(0);
              setNote('');
              setPin('');
              setStep({ kind: 'recipient' });
            }}
          >
            Done
          </button>
        </div>
      );
    }

    // ----------------------------------------------------------------
    // Step 3: review + PIN (idle or submitting).
    // ----------------------------------------------------------------
    return (
      <div className="px" style={{ paddingTop: 16, flex: 1, display: 'flex', flexDirection: 'column' }}>
        <button
          type="button"
          className="link"
          onClick={() => !submitting && setStep({ kind: 'amount' })}
        >
          ← Back
        </button>
        <h2 style={{ margin: '14px 0 10px' }}>Review transfer</h2>
        <div className="card-flat" style={{ padding: '4px 14px' }}>
          <div className="review-row">
            <span className="rv-label muted">To</span>
            <span className="rv-value">{recipient}</span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Amount</span>
            <span className="rv-value">
              <Money minor={amountMinor} currency={currency} />
            </span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Fee</span>
            <span className="rv-value muted">Calculated by AmberPay at authorization</span>
          </div>
          {note.trim() && (
            <div className="review-row">
              <span className="rv-label muted">Note</span>
              <span className="rv-value">{note}</span>
            </div>
          )}
        </div>
        <p className="faint" style={{ fontSize: '.75rem', marginTop: 12, lineHeight: 1.5 }}>
          Enter your transaction PIN to authorize. This confirms the transfer only — it doesn't
          guarantee the recipient's bank or mobile-money provider has completed it yet.
        </p>
        {pinError && (
          <p className="form-error" role="alert">
            {pinError}
          </p>
        )}
        <PinDots filled={pin.length} />
        <div style={{ maxWidth: 220, margin: '0 auto' }}>
          <Keypad
            compact
            onDigit={(d) => setPin((p) => (p.length < 4 ? p + d : p))}
            onBackspace={() => setPin((p) => p.slice(0, -1))}
          />
        </div>
        <div style={{ marginTop: 'auto', paddingBottom: 16, paddingTop: 12 }}>
          <button
            type="button"
            className="button"
            style={{ width: '100%' }}
            disabled={pin.length < 4 || submitting}
            onClick={() => {
              setPinError(null);
              setStep({ kind: 'sent' });
              void submit({ pin, pinToken: pinToken.current, transfer: buildRequest() });
            }}
          >
            {submitting ? 'Sending…' : 'Confirm transfer'}
          </button>
        </div>
      </div>
    );
  }

  return null;
}
