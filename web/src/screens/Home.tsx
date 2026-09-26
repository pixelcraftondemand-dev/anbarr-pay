import { useCallback, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ApiError } from '../api/client';
import type { StatementEntry, StatementPage, Wallet } from '../api/types';
import { request } from '../api/requests';
import { IconEye, IconLock, IconShieldNote } from '../components/Icons';
import { TxRow } from '../components/TxRow';
import { ScreenState } from '../components/ScreenState';
import { formatMinorUnits } from '../components/Money';
import { VaultMiniCard } from '../components/VaultMiniCard';
import { useOnline } from '../hooks/useOnline';

type State =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; wallets: Wallet[]; recent: StatementEntry[] };

const RECENT_LIMIT = 20;

type ServiceKey =
  | 'send'
  | 'topup'
  | 'request'
  | 'bill'
  | 'vault'
  | 'withdraw'
  | 'beneficiaries'
  | 'merchant';

const SERVICES: {
  key: ServiceKey;
  label: string;
  to?: string;
  trust?: boolean;
  soon?: string;
}[] = [
  { key: 'send', label: 'Send', to: '/send' },
  { key: 'topup', label: 'Add money', to: '/topup' },
  { key: 'request', label: 'Request', soon: 'Requests need the payments API, not connected yet.' },
  { key: 'bill', label: 'Bills', soon: 'Billers are coming with the merchant rails.' },
  { key: 'vault', label: 'Vault', to: '/vault', trust: true },
  { key: 'withdraw', label: 'Withdraw', to: '/withdraw' },
  { key: 'beneficiaries', label: 'People', to: '/beneficiaries' },
  { key: 'merchant', label: 'Merchant', to: '/merchant' },
];

export function Home() {
  const navigate = useNavigate();
  const online = useOnline();
  const [state, setState] = useState<State>({ kind: 'loading' });
  const [balanceVisible, setBalanceVisible] = useState(true);

  const load = useCallback(async () => {
    setState({ kind: 'loading' });
    try {
      const wallets = await request<Wallet[]>('/wallets');
      // Recent activity = the primary wallet's real statement (the ledger's
      // ListEntries). No demo rows are ever rendered.
      let recent: StatementEntry[] = [];
      const primary = wallets[0];
      if (primary) {
        const page = await request<StatementPage>(
          `/wallets/${primary.id}/transactions?limit=${RECENT_LIMIT}`,
        );
        recent = page.entries;
      }
      setState({ kind: 'ready', wallets, recent });
    } catch (err) {
      const message =
        err instanceof ApiError ? err.message : 'Something went wrong loading your wallet.';
      setState({ kind: 'error', message });
    }
  }, []);

  useEffect(() => {
    if (online) void load();
  }, [load, online]);

  if (!online) {
    return <ScreenState kind="offline" onRetry={() => void load()} />;
  }
  if (state.kind === 'loading') return <ScreenState kind="loading" />;
  if (state.kind === 'error') {
    return <ScreenState kind="error" message={state.message} onRetry={() => void load()} />;
  }

  const { wallets, recent } = state;
  const primary = wallets[0];

  return (
    <div className="px" style={{ paddingBottom: 8 }}>
      <div className="greet">
        <div>
          <div className="faint" style={{ fontSize: '.75rem' }}>{greetingNow()}</div>
          <h2>AmberPay</h2>
        </div>
        <button
          type="button"
          className="icon-btn"
          aria-label="Security"
          onClick={() => navigate('/security')}
        >
          <IconLock stroke="var(--ink-500)" />
        </button>
      </div>

      {primary ? (
        <div className="balance-card">
          <div className="balance-label">
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="rgba(255,255,255,.7)"
              strokeWidth="1.8"
            >
              <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
            Available balance
            <button
              type="button"
              className="balance-eye"
              aria-label={balanceVisible ? 'Hide balance' : 'Show balance'}
              onClick={() => setBalanceVisible((v) => !v)}
            >
              <IconEye stroke="#fff" />
            </button>
          </div>
          <div className="balance-amount">
            {balanceVisible ? (
              formatMinorUnits(primary.available_minor, primary.currency)
            ) : (
              <span className="masked">•• ••• ••</span>
            )}
          </div>
          <div className="balance-sub">
            {formatMinorUnits(primary.held_minor, primary.currency)} held ·{' '}
            {formatMinorUnits(primary.total_minor, primary.currency)} total · {primary.currency}
          </div>
        </div>
      ) : (
        <div className="balance-card">
          <div className="balance-label">No wallet yet</div>
          <div className="balance-sub">
            Your wallet appears here once the data layer is connected.
          </div>
        </div>
      )}

      {/* Services grid — the OPay lesson: every destination one tap away. */}
      <div className="services" role="navigation" aria-label="Services">
        {SERVICES.map((svc) => {
          const to = svc.to;
          const disabled = !to || !online;
          return (
            <button
              key={svc.key}
              type="button"
              className="svc"
              aria-disabled={disabled || undefined}
              title={svc.soon}
              onClick={() => {
                if (!to || !online) return;
                navigate(to);
              }}
            >
              <span className={`circ${svc.trust ? ' trust' : ''}`}>
                <ServiceIcon service={svc.key} />
              </span>
              <span className="lbl">{svc.label}</span>
            </button>
          );
        })}
      </div>

      {/* The Monime lesson: savings with intent. Locked goals ride the same
          holds machinery as escrow — real entries, no stored balances. */}
      <VaultMiniCard onOpenAll={() => navigate('/vault')} />

      <div className="trust-strip">
        <IconShieldNote size={15} stroke="var(--amber-800)" />
        <span>
          Every balance and entry here is served from the AmberPay ledger — the same immutable
          record our reconciliation audits. No cached numbers.
        </span>
      </div>

      <div className="section-head">
        <h3>Recent activity</h3>
        <button type="button" className="link" onClick={() => navigate('/activity')}>
          View all
        </button>
      </div>
      {recent.length === 0 ? (
        <p className="empty-note">
          No transactions yet — send your first payment and it will appear here.
        </p>
      ) : (
        recent.map((entry) => (
          <div key={entry.entry_id}>
            <TxRow entry={entry} />
            <hr className="hair" />
          </div>
        ))
      )}
      {primary && (
        <p className="faint" style={{ fontSize: '.72rem', marginTop: 8 }}>
          Balances and entries come straight from the ledger on every load.
        </p>
      )}
    </div>
  );
}

function ServiceIcon({ service }: { service: ServiceKey }) {
  const stroke = 'var(--navy-900)';
  const c = { width: 20, height: 20, viewBox: '0 0 24 24', fill: 'none' } as const;
  switch (service) {
    case 'send':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 11.5L20 3.5 12.5 20.5 10.2 12.8 3 11.5z" />
          <path d="M10.2 12.8L20 3.5" />
        </svg>
      );
    case 'topup':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3.5 8.2A2.2 2.2 0 0 1 5.7 6h11.6a2.2 2.2 0 0 1 2.2 2.2v.8h1a1.5 1.5 0 0 1 1.5 1.5v6a2.2 2.2 0 0 1-2.2 2.2H5.7a2.2 2.2 0 0 1-2.2-2.2z" />
          <path d="M15.2 13.3h4.3M17.35 11.15v4.3" />
        </svg>
      );
    case 'request':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 4.2v10.3" />
          <path d="M7.6 10.8L12 15.2l4.4-4.4" />
          <path d="M4.2 15.6v3a2 2 0 0 0 2 2h11.6a2 2 0 0 0 2-2v-3" />
        </svg>
      );
    case 'bill':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M6 3h12v13.2l-1.5 1.4L15 16.2l-1.5 1.4L12 16.2l-1.5 1.4L9 16.2l-1.5 1.4L6 16.2z" />
          <path d="M9 7.4h6M9 10.4h6M9 13.4h3.2" />
        </svg>
      );
    case 'vault':
      return (
        <svg {...c} stroke="var(--amber-600)" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3.5" y="5" width="17" height="15" rx="2.5" />
          <circle cx="12" cy="12.5" r="3.6" />
          <path d="M12 10.5v-1M12 15.5v-1M14 12.5h-1M11 12.5h-1" />
        </svg>
      );
    case 'withdraw':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 19V6.5" />
          <path d="M6.5 13.5L12 19l5.5-5.5" />
          <path d="M4.5 20.5h15" />
        </svg>
      );
    case 'beneficiaries':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="9" cy="8.5" r="3.2" />
          <path d="M3.5 19.5c0-3 2.5-5 5.5-5s5.5 2 5.5 5" />
          <circle cx="16.5" cy="9.5" r="2.6" />
          <path d="M16.5 14.5c2.5 0 4.5 1.8 4.5 4.5" />
        </svg>
      );
    case 'merchant':
      return (
        <svg {...c} stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 9.5L6 4h12l2 5.5" />
          <path d="M4 9.5h16V19a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 19z" />
          <path d="M9.5 20.5v-6h5v6" />
        </svg>
      );
  }
}

function greetingNow(): string {
  const h = new Date().getHours();
  if (h < 12) return 'Good morning';
  if (h < 18) return 'Good afternoon';
  return 'Good evening';
}
