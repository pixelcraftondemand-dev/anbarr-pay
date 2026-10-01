import { useState } from 'react';
import { ApiError } from '../api/client';
import type { Currency, TopupResponse } from '../api/types';
import { request } from '../api/requests';
import { pinVerificationUnavailable } from '../hooks/useTransferSubmit';
import { Money, formatMinorUnits } from '../components/Money';
import { StatusPill } from '../components/StatusPill';
import { Keypad, PinDots } from '../components/Keypad';
import { ScreenState } from '../components/ScreenState';
import { useIdempotentKey } from '../hooks/useIdempotentKey';
import { useOnline } from '../hooks/useOnline';

type Step =
  | { kind: 'form' }
  | { kind: 'review' }
  | { kind: 'submitting' }
  | { kind: 'pending'; amountMinor: number; phone: string }
  | { kind: 'error'; message: string }
  | { kind: 'result'; result: TopupResponse; amountMinor: number; currency: Currency };

/** Mobile-money rails the API documents (docs/api.md §6). The list is the
 *  adapter set — which rails are live for a given account is the server's
 *  decision at submission time. */
const RAILS = [
  { value: 'orange_money', label: 'Orange Money' },
  { value: 'afrimoney', label: 'Afrimoney' },
];

const MAX_AMOUNT_MINOR = 99_999_999_999;

export function Topup() {
  const online = useOnline();
  const { key, reset } = useIdempotentKey();

  const [step, setStep] = useState<Step>({ kind: 'form' });
  const [rail, setRail] = useState(RAILS[0].value);
  const [phone, setPhone] = useState('');
  const [currency, setCurrency] = useState<Currency>('SLE');
  const [amountMinor, setAmountMinor] = useState(0);
  const [pin, setPin] = useState('');
  const [pinToken, setPinToken] = useState<string | null>(null);
  const [fieldError, setFieldError] = useState<string | null>(null);

  if (!online) {
    return <ScreenState kind="offline" />;
  }

  const submit = async () => {
    if (step.kind === 'submitting') return;
    setStep({ kind: 'submitting' });
    try {
      const token = pinToken ?? (await pinVerificationUnavailable());
      setPinToken(token);
      setPin('');
      const result = await request<TopupResponse>('/topups', {
        method: 'POST',
        idempotencyKey: key,
        pinToken: token,
        body: {
          rail,
          destination_phone: phone.trim(),
          amount_minor: amountMinor,
          currency,
        },
      });
      setStep({ kind: 'result', result, amountMinor, currency });
    } catch (err) {
      if (err instanceof ApiError && err.status === 0) {
        // Unknown outcome — stay pending, retry reuses key + pin_token.
        setStep({ kind: 'pending', amountMinor, phone: phone.trim() });
        return;
      }
      if (err instanceof ApiError && err.status === 401) {
        // No session layer right now — the error surfaces on this screen.
      }
      setStep({
        kind: 'error',
        message: err instanceof ApiError ? err.message : 'Top-up could not be completed.',
      });
    }
  };

  if (step.kind === 'form') {
    const phoneValid = /^\+?[0-9]{7,15}$/.test(phone.trim());
    return (
      <div className="px" style={{ paddingTop: 16, flex: 1, display: 'flex', flexDirection: 'column' }}>
        <h2 style={{ margin: '0 0 4px' }}>Add money</h2>
        <div className="faint" style={{ fontSize: '.8rem', marginBottom: 12 }}>
          Top up from your mobile-money account
        </div>
        <label style={{ marginBottom: 12 }}>
          Provider
          <select value={rail} onChange={(e) => setRail(e.target.value)}>
            {RAILS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
        <label style={{ marginBottom: 12 }}>
          Your mobile-money number
          <input
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            placeholder="+232 76 000000"
            type="tel"
            autoComplete="tel"
          />
        </label>
        <label style={{ marginBottom: 4 }}>
          Currency
          <select value={currency} onChange={(e) => setCurrency(e.target.value as Currency)}>
            <option value="SLE">SLE — Sierra Leone</option>
            <option value="USD">USD</option>
          </select>
        </label>
        <div style={{ margin: '12px 0 0' }}>
          <div className="amount-display" style={{ padding: '10px 0 6px' }}>
            <span className="cur">{currency}</span>{' '}
            <span className="num">{(amountMinor / 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}</span>
          </div>
          <Keypad
            onDigit={(d) => setAmountMinor((a) => Math.min(a * 10 + d, MAX_AMOUNT_MINOR))}
            onBackspace={() => setAmountMinor((a) => Math.floor(a / 10))}
          />
        </div>
        {fieldError && (
          <p className="form-error" role="alert">
            {fieldError}
          </p>
        )}
        <div style={{ marginTop: 'auto', paddingBottom: 16, paddingTop: 12 }}>
          <button
            type="button"
            className="button"
            style={{ width: '100%' }}
            disabled={!phoneValid || amountMinor <= 0}
            onClick={() => {
              setFieldError(null);
              setStep({ kind: 'review' });
            }}
          >
            Continue
          </button>
        </div>
      </div>
    );
  }

  if (step.kind === 'review' || step.kind === 'submitting') {
    const submitting = step.kind === 'submitting';
    return (
      <div className="px" style={{ paddingTop: 16, flex: 1, display: 'flex', flexDirection: 'column' }}>
        <button type="button" className="link" onClick={() => !submitting && setStep({ kind: 'form' })}>
          ← Back
        </button>
        <h2 style={{ margin: '14px 0 10px' }}>Review top-up</h2>
        <div className="card-flat" style={{ padding: '4px 14px' }}>
          <div className="review-row">
            <span className="rv-label muted">Provider</span>
            <span className="rv-value">{RAILS.find((r) => r.value === rail)?.label ?? rail}</span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">From number</span>
            <span className="rv-value">{phone}</span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Amount</span>
            <span className="rv-value">
              <Money minor={amountMinor} currency={currency} />
            </span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Fee</span>
            <span className="rv-value muted">Confirmed by AmberPay at authorization</span>
          </div>
        </div>
        <p className="faint" style={{ fontSize: '.75rem', marginTop: 12, lineHeight: 1.5 }}>
          Enter your transaction PIN to authorize this top-up.
        </p>
        <PinDots filled={pin.length} />
        <div style={{ maxWidth: 220, margin: '0 auto' }}>
          <Keypad compact onDigit={(d) => setPin((p) => (p.length < 4 ? p + d : p))} onBackspace={() => setPin((p) => p.slice(0, -1))} />
        </div>
        <div style={{ marginTop: 'auto', paddingBottom: 16, paddingTop: 12 }}>
          <button
            type="button"
            className="button"
            style={{ width: '100%' }}
            disabled={pin.length < 4 || submitting}
            onClick={() => void submit()}
          >
            {submitting ? 'Sending…' : 'Confirm top-up'}
          </button>
        </div>
      </div>
    );
  }

  if (step.kind === 'pending') {
    return (
      <div className="pending-state" style={{ flex: 1 }}>
        <div className="ring">
          <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--gold-800)" strokeWidth="2">
            <circle cx="12" cy="12" r="9" />
            <path d="M12 7v5l3 3" />
          </svg>
        </div>
        <h2 style={{ margin: 0 }}>Top-up pending</h2>
        <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>
          {formatMinorUnits(step.amountMinor, currency)} from {step.phone} hasn't confirmed yet —
          the connection dropped before the provider responded. It has not failed.
        </p>
        <span className="pill pill-pending pill-sm">PENDING</span>
        <button
          type="button"
          className="button button-ghost"
          style={{ marginTop: 14, width: '100%' }}
          onClick={() => void submit()}
        >
          Check status again
        </button>
      </div>
    );
  }

  if (step.kind === 'error') {
    return (
      <div className="pending-state" style={{ flex: 1 }}>
        <div className="ring ring-danger">
          <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--red-700)" strokeWidth="2" strokeLinecap="round">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </div>
        <h2 style={{ margin: 0 }}>Top-up couldn't be completed</h2>
        <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>{step.message}</p>
        <button
          type="button"
          className="button"
          style={{ marginTop: 14, width: '100%' }}
          onClick={() => {
            reset();
            setAmountMinor(0);
            setPin('');
            setPinToken(null);
            setStep({ kind: 'form' });
          }}
        >
          Try again
        </button>
      </div>
    );
  }

  const { result } = step;
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
      <h2 style={{ margin: 0 }}>{completed ? 'Top-up confirmed' : "We're confirming your top-up"}</h2>
      <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>
        {formatMinorUnits(step.amountMinor, step.currency)} via{' '}
        {RAILS.find((r) => r.value === rail)?.label ?? rail}
      </p>
      <StatusPill status={result.status} />
      <p className="faint" style={{ fontSize: '.72rem', margin: 0 }}>
        Reference {result.id}
        {result.rail_reference ? ` · rail ref ${result.rail_reference}` : ''}
      </p>
      <button
        type="button"
        className="button button-ghost"
        style={{ marginTop: 14, width: '100%' }}
        onClick={() => {
          reset();
          setPhone('');
          setAmountMinor(0);
          setPin('');
          setPinToken(null);
          setStep({ kind: 'form' });
        }}
      >
        Done
      </button>
    </div>
  );
}
