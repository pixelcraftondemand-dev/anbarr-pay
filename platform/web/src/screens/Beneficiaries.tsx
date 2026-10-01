import { useCallback, useEffect, useState } from 'react';
import { ApiError } from '../api/client';
import type { Beneficiary } from '../api/types';
import { request } from '../api/requests';
import { ScreenState } from '../components/ScreenState';
import { useOnline } from '../hooks/useOnline';

type State =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; people: Beneficiary[] };

/**
 * Beneficiaries list (docs/ux-flows.md §3.4). Adding/editing a beneficiary
 * is a sensitive action (strong auth, cooling-off, audit) and ships with the
 * beneficiaries API; until then this screen reads the list if the endpoint
 * exists and reports honestly when it doesn't. Send-money picks recipients
 * from the same source via BeneficiaryPicker.
 */
export function Beneficiaries() {
  const online = useOnline();
  const [state, setState] = useState<State>({ kind: 'loading' });

  const load = useCallback(async () => {
    setState({ kind: 'loading' });
    try {
      const people = await request<Beneficiary[]>('/beneficiaries');
      setState({ kind: 'ready', people });
    } catch (err) {
      setState({
        kind: 'error',
        message:
          err instanceof ApiError
            ? err.message
            : 'Could not load your beneficiaries.',
      });
    }
  }, []);

  useEffect(() => {
    if (online) void load();
  }, [load, online]);

  if (!online) return <ScreenState kind="offline" onRetry={() => void load()} />;
  if (state.kind === 'loading') return <ScreenState kind="loading" />;
  if (state.kind === 'error') {
    // The endpoint being absent is an honest "not built yet", not a crash —
    // show the server's message with context instead of an empty list.
    return (
      <div className="px" style={{ paddingTop: 18 }}>
        <h2 style={{ margin: '0 0 4px' }}>Beneficiaries</h2>
        <div className="trust-strip" role="status">
          <span>
            The beneficiaries service isn't connected in this environment yet
            {state.message ? ` — ${state.message}` : ''}. Recipients are added automatically
            as you pay them once the API ships; nothing here is simulated.
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="px" style={{ paddingTop: 18, paddingBottom: 24 }}>
      <h2 style={{ margin: '0 0 4px' }}>Beneficiaries</h2>
      <p className="muted" style={{ fontSize: '.85rem', marginTop: 0 }}>
        People you pay, saved by AmberPay as you send.
      </p>
      {state.people.length === 0 ? (
        <p className="empty-note">
          No recipients yet — send your first payment and they'll appear here.
        </p>
      ) : (
        <div className="card-flat" style={{ padding: '4px 14px', marginTop: 10 }}>
          {state.people.map((p, i) => (
            <div key={p.id}>
              {i > 0 && <hr className="hair" />}
              <div className="set-row">
                <div className="left">
                  <span className="ic" aria-hidden>
                    {p.name.slice(0, 1).toUpperCase()}
                  </span>
                  <div>
                    <div className="lbl">{p.name}</div>
                    <div className="sub">{p.email_or_phone}</div>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
