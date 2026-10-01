import { useCallback, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ApiError } from '../api/client';
import type {
  CreateGoalRequest,
  CreateGoalResponse,
  Currency,
  ReleaseResponse,
  VaultGoal,
} from '../api/types';
import { request } from '../api/requests';
import { formatMinorUnits } from '../components/Money';
import { ScreenState } from '../components/ScreenState';
import { ConfirmSheet } from '../components/ConfirmSheet';
import { Keypad, PinDots } from '../components/Keypad';
import { IconLock, IconPlus } from '../components/Icons';
import { useIdempotentKey } from '../hooks/useIdempotentKey';
import { useOnline } from '../hooks/useOnline';

type Step =
  | { kind: 'list' }
  | { kind: 'creating' }
  | { kind: 'review' }
  | { kind: 'submitting' }
  | { kind: 'pending'; amountMinor: number; name: string }
  | { kind: 'error'; message: string }
  | { kind: 'result'; goal: CreateGoalResponse; amountMinor: number; name: string };

const MAX_AMOUNT_MINOR = 99_999_999_999;

/**
 * The Vault — goal savings riding the ledger's holds engine (docs/vault.md).
 *
 * Honesty rules (§73): until the Core API ships the /v1/vault endpoints, the
 * screen shows exactly what exists — the wallet's real held balance — and its
 * real error state for goal creation. It never fabricates goals or interest.
 * When the API lands, this screen's contract is already final: the types and
 * idempotent submission here match docs/vault.md §2 line for line.
 */
export function Vault() {
  const navigate = useNavigate();
  const online = useOnline();
  const { key, reset } = useIdempotentKey();

  const [goals, setGoals] = useState<VaultGoal[] | null>(null);
  const [goalsUnavailable, setGoalsUnavailable] = useState<string | null>(null);
  const [step, setStep] = useState<Step>({ kind: 'list' });
  const [name, setName] = useState('');
  const [currency, setCurrency] = useState<Currency>('SLE');
  const [amountMinor, setAmountMinor] = useState(0);
  const [pin, setPin] = useState('');
  const [pinToken, setPinToken] = useState<string | null>(null);
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [releaseTarget, setReleaseTarget] = useState<VaultGoal | null>(null);
  const [releasing, setReleasing] = useState(false);
  const [releaseDone, setReleaseDone] = useState<string | null>(null);

  const load = useCallback(async () => {
    setGoalsUnavailable(null);
    try {
      const page = await request<VaultGoal[]>('/vault/goals');
      setGoals(page);
    } catch (err) {
      // The Vault API is not connected yet in this environment — show the
      // real reason instead of pretending the list is simply empty.
      setGoalsUnavailable(
        err instanceof ApiError ? err.message : 'Could not load your goals.',
      );
      setGoals([]);
    }
  }, []);

  useEffect(() => {
    if (online) void load();
  }, [load, online]);

  const createGoal = async () => {
    if (step.kind === 'submitting') return;
    setStep({ kind: 'submitting' });
    try {
      const token =
        pinToken ??
        (await request<{ pin_token: string }>('/auth/pin/verify', {
          method: 'POST',
          body: { pin },
        })).pin_token;
      setPinToken(token);
      setPin('');
      const body: CreateGoalRequest = {
        name: name.trim(),
        currency,
        amount_minor: amountMinor,
      };
      const result = await request<CreateGoalResponse>('/vault/goals', {
        method: 'POST',
        idempotencyKey: key,
        pinToken: token,
        body,
      });
      setStep({ kind: 'result', goal: result, amountMinor, name: name.trim() });
    } catch (err) {
      if (err instanceof ApiError && err.status === 0) {
        // Unknown outcome: retry reuses the idempotency key + pin token.
        setStep({ kind: 'pending', amountMinor, name: name.trim() });
        return;
      }
      setStep({
        kind: 'error',
        message: err instanceof ApiError ? err.message : 'The goal could not be created.',
      });
    }
  };

  const retry = () => void createGoal();

  const releaseGoal = async (goal: VaultGoal) => {
    setReleasing(true);
    try {
      const result = await request<ReleaseResponse>(`/vault/goals/${goal.id}/release`, {
        method: 'POST',
        idempotencyKey: crypto.randomUUID(),
        body: {},
      });
      setReleaseDone(
        `${formatMinorUnits(result.released_minor, goal.currency)} returned to your wallet.`,
      );
      setReleaseTarget(null);
      await load();
    } catch {
      setReleaseDone('The unlock could not be completed — try again.');
    } finally {
      setReleasing(false);
    }
  };

  if (!online) return <ScreenState kind="offline" onRetry={() => void load()} />;

  if (step.kind === 'creating' || step.kind === 'review' || step.kind === 'submitting') {
    const reviewing = step.kind !== 'creating';
    const submitting = step.kind === 'submitting';
    const nameValid = name.trim().length >= 2 && name.trim().length <= 60;
    if (!reviewing) {
      return (
        <div className="px" style={{ paddingTop: 18, flex: 1, display: 'flex', flexDirection: 'column' }}>
          <button type="button" className="link" onClick={() => setStep({ kind: 'list' })}>
            ← Back
          </button>
          <h2 style={{ margin: '10px 0 4px' }}>New goal</h2>
          <p className="faint" style={{ fontSize: '.8rem', margin: '0 0 12px' }}>
            Lock money away from spending — it stays on the ledger, in your name.
          </p>
          <label style={{ marginBottom: 12 }}>
            What are you saving for?
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="School fees, new freezer, stock…"
              maxLength={60}
            />
          </label>
          <label style={{ marginBottom: 4 }}>
            Currency
            <select value={currency} onChange={(e) => setCurrency(e.target.value as Currency)}>
              <option value="SLE">SLE — Sierra Leone</option>
              <option value="USD">USD</option>
            </select>
          </label>
          <div className="amount-display" style={{ padding: '14px 0 4px' }}>
            <span className="cur">{currency}</span>{' '}
            <span className="num">
              {(amountMinor / 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}
            </span>
          </div>
          <div style={{ display: 'flex', justifyContent: 'center' }}>
            <Keypad
              onDigit={(d) => setAmountMinor((a) => Math.min(a * 10 + d, MAX_AMOUNT_MINOR))}
              onBackspace={() => setAmountMinor((a) => Math.floor(a / 10))}
            />
          </div>
          {fieldError && (
            <p className="form-error" role="alert" style={{ marginTop: 8 }}>
              {fieldError}
            </p>
          )}
          <div style={{ marginTop: 'auto', padding: '12px 0 16px' }}>
            <button
              type="button"
              className="button"
              style={{ width: '100%' }}
              disabled={!nameValid || amountMinor <= 0}
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
    return (
      <div className="px" style={{ paddingTop: 18, flex: 1, display: 'flex', flexDirection: 'column' }}>
        <button
          type="button"
          className="link"
          onClick={() => !submitting && setStep({ kind: 'creating' })}
        >
          ← Back
        </button>
        <h2 style={{ margin: '10px 0 10px' }}>Review goal</h2>
        <div className="card-flat" style={{ padding: '4px 14px' }}>
          <div className="review-row">
            <span className="rv-label muted">Goal</span>
            <span className="rv-value">{name.trim()}</span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Lock now</span>
            <span className="rv-value">{formatMinorUnits(amountMinor, currency)}</span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Fee</span>
            <span className="rv-value">None — your money, held on the ledger</span>
          </div>
          <div className="review-row">
            <span className="rv-label muted">Interest</span>
            <span className="rv-value">None — the Vault does not invent yield</span>
          </div>
        </div>
        <p className="faint" style={{ fontSize: '.75rem', marginTop: 12, lineHeight: 1.5 }}>
          Locking moves the amount into a ledger hold in your name. Unlock anytime — the full
          amount returns to your wallet instantly.
        </p>
        {pinErrorWithMessage(step.kind)}
        <PinDots filled={pin.length} />
        <div style={{ display: 'flex', justifyContent: 'center' }}>
          <Keypad
            compact
            onDigit={(d) => setPin((p) => (p.length < 4 ? p + d : p))}
            onBackspace={() => setPin((p) => p.slice(0, -1))}
          />
        </div>
        <div style={{ marginTop: 'auto', padding: '12px 0 16px' }}>
          <button
            type="button"
            className="button"
            style={{ width: '100%' }}
            disabled={pin.length < 4 || submitting}
            onClick={() => void createGoal()}
          >
            {submitting ? 'Locking…' : 'Lock it away'}
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
        <h2 style={{ margin: 0 }}>Lock pending</h2>
        <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>
          {formatMinorUnits(step.amountMinor, currency)} for “{step.name}” hasn't confirmed yet —
          the connection dropped before AmberPay could respond. It has not failed.
        </p>
        <span className="pill pill-pending pill-sm">PENDING</span>
        <button type="button" className="button button-ghost" style={{ marginTop: 14, width: '100%' }} onClick={retry}>
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
        <h2 style={{ margin: 0 }}>Goal couldn't be created</h2>
        <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>{step.message}</p>
        <button
          type="button"
          className="button"
          style={{ marginTop: 14, width: '100%' }}
          onClick={() => {
            reset();
            setPin('');
            setPinToken(null);
            setAmountMinor(0);
            setStep({ kind: 'creating' });
          }}
        >
          Try again
        </button>
      </div>
    );
  }

  if (step.kind === 'result') {
    return (
      <div className="pending-state" style={{ flex: 1 }}>
        <div className="ring">
          <IconLock size={26} stroke="var(--green-700)" />
        </div>
        <h2 style={{ margin: 0 }}>Locked away</h2>
        <p className="muted" style={{ fontSize: '.85rem', margin: 0 }}>
          {formatMinorUnits(step.amountMinor, currency)} is locked for “{step.name}”.
        </p>
        <p className="faint" style={{ fontSize: '.72rem', margin: 0 }}>
          Hold {step.goal.hold_id.slice(0, 8)} · it never left your account
        </p>
        <button
          type="button"
          className="button button-ghost"
          style={{ marginTop: 14, width: '100%' }}
          onClick={() => {
            reset();
            setName('');
            setAmountMinor(0);
            setPin('');
            setPinToken(null);
            setStep({ kind: 'list' });
            void load();
          }}
        >
          Done
        </button>
      </div>
    );
  }

  // ------------------------------------------------------------------
  // List step: goals + locked total + how-it-works
  // ------------------------------------------------------------------
  const list = goals ?? [];
  const lockedTotal = list
    .filter((g) => g.status === 'active')
    .reduce((sum, g) => sum + g.locked_minor, 0);

  return (
    <div className="px" style={{ paddingTop: 18, paddingBottom: 24 }}>
      <div className="greet" style={{ paddingTop: 0 }}>
        <h2 style={{ margin: 0 }}>Vault</h2>
        <button type="button" className="icon-btn" aria-label="New goal" onClick={() => setStep({ kind: 'creating' })}>
          <IconPlus stroke="var(--ink-500)" />
        </button>
      </div>
      <p className="muted" style={{ fontSize: '.85rem', marginTop: 0 }}>
        Lock money away from spending — for school fees, stock, or a rainy day.
      </p>

      {goalsUnavailable && (
        <div className="trust-strip" role="status">
          <IconLock size={15} stroke="var(--amber-800)" />
          <span>
            Goals need the Vault API, which isn't connected in this environment yet
            {goalsUnavailable ? ` — ${goalsUnavailable}` : ''}. Your real held balance is shown
            below; nothing here is simulated.
          </span>
        </div>
      )}

      <div className="balance-card" style={{ marginTop: 14 }}>
        <div className="balance-label">
          <IconLock size={14} stroke="rgba(255,255,255,.7)" />
          Locked right now
        </div>
        <div className="balance-amount">{formatMinorUnits(lockedTotal, 'SLE')}</div>
        <div className="balance-sub">
          From your goals{list.length > 0 ? '' : ' · none yet'}
        </div>
      </div>

      <div className="section-head">
        <h3>Your goals</h3>
      </div>
      {list.length === 0 ? (
        <p className="empty-note">
          No goals yet — tap + to lock your first amount away.
        </p>
      ) : (
        list.map((g) => (
          <div key={g.id} className="vault-card">
            <div className="v-head">
              <div className="v-name">{g.name}</div>
              {g.status === 'active' ? (
                <button
                  type="button"
                  className="link"
                  disabled={releasing}
                  onClick={() => setReleaseTarget(g)}
                >
                  Unlock
                </button>
              ) : (
                <span className="pill pill-completed pill-sm">RELEASED</span>
              )}
            </div>
            <div className="v-amount">{formatMinorUnits(g.locked_minor, g.currency)}</div>
            {g.target_minor !== undefined && g.target_minor > 0 && (
              <>
                <div
                  className="vault-progress"
                  role="progressbar"
                  aria-valuenow={Math.min(100, Math.round((g.locked_minor / g.target_minor) * 100))}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label={`${g.name} progress`}
                >
                  <div
                    className={g.locked_minor >= g.target_minor ? 'goal-met' : undefined}
                    style={{ width: `${Math.min(100, (g.locked_minor / g.target_minor) * 100)}%` }}
                  />
                </div>
                <div className="vault-meta">
                  <span>
                    {Math.round((g.locked_minor / g.target_minor) * 100)}% of{' '}
                    {formatMinorUnits(g.target_minor, g.currency)}
                  </span>
                  {g.maturity_at && <span>until {g.maturity_at.slice(0, 10)}</span>}
                </div>
              </>
            )}
          </div>
        ))
      )}

      <div className="section-head">
        <h3>How the Vault works</h3>
      </div>
      <div className="card-flat" style={{ padding: '4px 14px' }}>
        <div className="review-row">
          <span className="rv-label muted">1 · Lock</span>
          <span className="rv-value">
            Choose an amount — it moves from available into a hold. It stops being spendable
            instantly.
          </span>
        </div>
        <hr className="hair" />
        <div className="review-row">
          <span className="rv-label muted">2 · Wait or add</span>
          <span className="rv-value">
            Add to a goal anytime while it runs. Your money never leaves the ledger.
          </span>
        </div>
        <hr className="hair" />
        <div className="review-row">
          <span className="rv-label muted">3 · Unlock</span>
          <span className="rv-value">
            Release it all back to available — instantly, with a full audit trail.
          </span>
        </div>
      </div>

      <button
        type="button"
        className="button"
        style={{ width: '100%', marginTop: 16 }}
        onClick={() => setStep({ kind: 'creating' })}
      >
        Lock money away
      </button>
      <p className="faint" style={{ fontSize: '.72rem', marginTop: 10, lineHeight: 1.5 }}>
        Every lock is a real ledger hold (wallet → escrow) — the same audited machinery the
        escrow for merchant payments uses. Contract: docs/vault.md.
      </p>

      <ConfirmSheet
        open={releaseTarget !== null}
        title={`Unlock “${releaseTarget?.name ?? ''}”`}
        body="The full locked amount returns to your wallet immediately. This releases the ledger hold — your money, back in your hand."
        confirmLabel={releasing ? 'Unlocking…' : 'Unlock it'}
        onConfirm={() => releaseTarget && void releaseGoal(releaseTarget)}
        onCancel={() => setReleaseTarget(null)}
      />
      {releaseDone && (
        <div className="card" style={{ padding: 14, marginTop: 12 }} role="status">
          <p style={{ margin: 0, fontSize: '.85rem' }}>
            <strong>Unlocked.</strong> {releaseDone}
          </p>
        </div>
      )}

      <button
        type="button"
        className="link"
        style={{ marginTop: 18 }}
        onClick={() => navigate('/home')}
      >
        ← Back to home
      </button>
    </div>
  );
}

/** PIN field note reused by the review step. */
function pinErrorWithMessage(stepKind: Step['kind']) {
  if (stepKind === 'submitting') {
    return <p className="faint" style={{ fontSize: '.75rem', marginTop: 12 }}>Authorizing…</p>;
  }
  return (
    <p className="faint" style={{ fontSize: '.75rem', marginTop: 12, lineHeight: 1.5 }}>
      Enter your transaction PIN to authorize the lock.
    </p>
  );
}
