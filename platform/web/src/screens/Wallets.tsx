import { useCallback, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { ApiError } from '../api/client';
import type { Wallet } from '../api/types';
import { request } from '../api/requests';
import { Money } from '../components/Money';
import { ScreenState } from '../components/ScreenState';
import { useOnline } from '../hooks/useOnline';

type State = { kind: 'loading' } | { kind: 'error'; message: string } | { kind: 'ready'; wallets: Wallet[] };

export function Wallets() {
  const online = useOnline();
  const [state, setState] = useState<State>({ kind: 'loading' });

  const load = useCallback(async () => {
    setState({ kind: 'loading' });
    try {
      const wallets = await request<Wallet[]>('/wallets');
      setState({ kind: 'ready', wallets });
    } catch (err) {
      const message =
        err instanceof ApiError ? err.message : 'Something went wrong loading your wallets.';
      setState({ kind: 'error', message });
    }
  }, [request]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!online) {
    return <ScreenState kind="offline" onRetry={() => void load()} />;
  }
  if (state.kind === 'loading') {
    return <ScreenState kind="loading" />;
  }
  if (state.kind === 'error') {
    return <ScreenState kind="error" message={state.message} onRetry={() => void load()} />;
  }
  if (state.wallets.length === 0) {
    return <ScreenState kind="empty" message="You don't have a wallet yet." />;
  }

  return (
    <div className="px" style={{ paddingTop: 16, paddingBottom: 24 }}>
      <h2 style={{ margin: '0 0 12px' }}>Wallets</h2>
      <p className="muted" style={{ fontSize: '.85rem' }}>
        Balances are updated from the ledger on every load — what you see here is always the
        server's state.
      </p>
      <ul className="wallet-list" style={{ listStyle: 'none', padding: 0, margin: 0 }}>
        {state.wallets.map((w) => (
          <li key={w.id} style={{ marginBottom: 12 }}>
            <Link
              to={`/wallets/${w.id}`}
              className="card"
              style={{ padding: 16, textDecoration: 'none', color: 'inherit' }}
            >
              <span className="wallet-currency muted" style={{ fontSize: '.9rem', fontWeight: 600 }}>
                {w.currency}
              </span>
              <span
                style={{
                  display: 'block',
                  fontSize: '1.6rem',
                  fontWeight: 650,
                  letterSpacing: '-0.02em',
                  fontVariantNumeric: 'tabular-nums',
                }}
              >
                <Money minor={w.available_minor} currency={w.currency} />
              </span>
              <span className="muted" style={{ fontSize: '.8rem' }}>
                <Money minor={w.held_minor} currency={w.currency} /> held ·{' '}
                <Money minor={w.total_minor} currency={w.currency} /> total
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
