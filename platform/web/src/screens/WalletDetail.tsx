import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { ApiError } from '../api/client';
import type { StatementEntry, StatementPage, Wallet } from '../api/types';
import { request } from '../api/requests';
import { Money } from '../components/Money';
import { TxRow } from '../components/TxRow';
import { ScreenState } from '../components/ScreenState';

type State =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; wallet: Wallet; entries: StatementEntry[] };

export function WalletDetail() {
  const { id } = useParams<{ id: string }>();
  const [state, setState] = useState<State>({ kind: 'loading' });

  const load = useCallback(async () => {
    if (!id) return;
    setState({ kind: 'loading' });
    try {
      const wallet = await request<Wallet>(`/wallets/${id}`);
      let entries: StatementEntry[] = [];
      try {
        const page = await request<StatementPage>(`/wallets/${id}/transactions?limit=25`);
        entries = page.entries;
      } catch {
        // Statement endpoint may not be provisioned for every wallet yet —
        // balances still render, the section degrades honestly.
        entries = [];
      }
      setState({ kind: 'ready', wallet, entries });
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof ApiError ? err.message : 'Could not load this wallet.',
      });
    }
  }, [id, request]);

  useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === 'loading') return <ScreenState kind="loading" />;
  if (state.kind === 'error') {
    return <ScreenState kind="error" message={state.message} onRetry={() => void load()} />;
  }

  const { wallet, entries } = state;
  return (
    <div className="px" style={{ paddingTop: 16, paddingBottom: 24 }}>
      <button type="button" className="link" onClick={() => history.back()}>
        ← Back
      </button>
      <h2 style={{ margin: '10px 0 12px' }}>{wallet.currency} wallet</h2>
      <div className="balance-card">
        <div className="balance-label">Available</div>
        <div className="balance-amount">
          <Money minor={wallet.available_minor} currency={wallet.currency} />
        </div>
        <div className="balance-sub">
          <Money minor={wallet.held_minor} currency={wallet.currency} /> held ·{' '}
          <Money minor={wallet.total_minor} currency={wallet.currency} /> total · status{' '}
          {wallet.status}
        </div>
      </div>
      <div className="section-head">
        <h3>Statement</h3>
      </div>
      {entries.length === 0 ? (
        <p className="empty-note">No entries on this wallet yet.</p>
      ) : (
        entries.map((entry) => (
          <div key={entry.entry_id}>
            <TxRow entry={entry} />
            <hr className="hair" />
          </div>
        ))
      )}
    </div>
  );
}
