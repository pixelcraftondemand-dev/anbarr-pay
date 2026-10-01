import { useCallback, useEffect, useState } from 'react';
import { ApiError } from '../api/client';
import type { StatementEntry, StatementPage, Wallet } from '../api/types';
import { request } from '../api/requests';
import { TxRow } from '../components/TxRow';
import { ScreenState } from '../components/ScreenState';
import { useOnline } from '../hooks/useOnline';

type State =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; wallet: Wallet; entries: StatementEntry[]; nextCursor: string };

/**
 * Statement filters. The ledger statement endpoint exposes cursor pagination;
 * journal_type and direction come from the entries themselves, so filtering
 * happens client-side over fetched pages — the filter chips never invent a
 * status the ledger did not report.
 */
type Filter = 'all' | 'credit' | 'debit' | 'p2p' | 'topup';

const FILTERS: { key: Filter; label: string }[] = [
  { key: 'all', label: 'All' },
  { key: 'credit', label: 'Money in' },
  { key: 'debit', label: 'Money out' },
  { key: 'p2p', label: 'Transfers' },
  { key: 'topup', label: 'Top-ups' },
];

const PAGE_LIMIT = 25;

export function Activity() {
  const online = useOnline();
  const [state, setState] = useState<State>({ kind: 'loading' });
  const [filter, setFilter] = useState<Filter>('all');
  const [loadingMore, setLoadingMore] = useState(false);

  const load = useCallback(async () => {
    setState({ kind: 'loading' });
    try {
      const wallets = await request<Wallet[]>('/wallets');
      const primary = wallets[0];
      if (!primary) {
        throw new ApiError(404, 'no_wallet', "You don't have a wallet yet.");
      }
      const page = await request<StatementPage>(
        `/wallets/${primary.id}/transactions?limit=${PAGE_LIMIT}`,
      );
      setState({ kind: 'ready', wallet: primary, entries: page.entries, nextCursor: page.next_page_token });
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof ApiError ? err.message : 'Could not load your activity.',
      });
    }
  }, [request]);

  useEffect(() => {
    if (online) void load();
  }, [load, online]);

  const loadMore = async () => {
    if (state.kind !== 'ready' || !state.nextCursor || loadingMore) return;
    setLoadingMore(true);
    try {
      const page = await request<StatementPage>(
        `/wallets/${state.wallet.id}/transactions?limit=${PAGE_LIMIT}&cursor=${encodeURIComponent(state.nextCursor)}`,
      );
      setState((s) =>
        s.kind === 'ready'
          ? { ...s, entries: [...s.entries, ...page.entries], nextCursor: page.next_page_token }
          : s,
      );
    } catch {
      // Pagination is best-effort; the already-loaded pages remain visible.
    } finally {
      setLoadingMore(false);
    }
  };

  if (!online) return <ScreenState kind="offline" onRetry={() => void load()} />;
  if (state.kind === 'loading') return <ScreenState kind="loading" />;
  if (state.kind === 'error') {
    return <ScreenState kind="error" message={state.message} onRetry={() => void load()} />;
  }

  const { entries } = state;
  const visible = entries.filter((e) => {
    switch (filter) {
      case 'credit':
        return e.direction === 'credit';
      case 'debit':
        return e.direction === 'debit';
      case 'p2p':
        return e.journal_type === 'p2p';
      case 'topup':
        return e.journal_type === 'topup';
      default:
        return true;
    }
  });

  return (
    <div>
      <div className="px" style={{ paddingTop: 18 }}>
        <h2 style={{ margin: '0 0 4px' }}>Activity</h2>
        <p className="faint" style={{ fontSize: '.78rem', margin: '0 0 12px' }}>
          Every entry below is the ledger's own record — nothing is summarized away.
        </p>
        <div className="chiprow" role="tablist" aria-label="Filter activity">
          {FILTERS.map((f) => (
            <button
              key={f.key}
              type="button"
              role="tab"
              aria-selected={filter === f.key}
              className={`chip${filter === f.key ? ' active' : ''}`}
              onClick={() => setFilter(f.key)}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>
      <div className="px" style={{ paddingBottom: 16 }}>
        {visible.length === 0 ? (
          <p className="empty-note">Nothing matches this filter yet.</p>
        ) : (
          visible.map((entry) => (
            <div key={entry.entry_id}>
              <TxRow entry={entry} />
              <hr className="hair" />
            </div>
          ))
        )}
        {state.nextCursor && (
          <button
            type="button"
            className="button button-ghost"
            style={{ width: '100%', marginTop: 12 }}
            disabled={loadingMore}
            onClick={() => void loadMore()}
          >
            {loadingMore ? 'Loading…' : 'Load older entries'}
          </button>
        )}
        <p className="faint" style={{ fontSize: '.72rem', marginTop: 10 }}>
          Dispute any transfer or top-up from its receipt screen — the case attaches to the
          transaction itself, not to a phone queue.
        </p>
      </div>
    </div>
  );
}
