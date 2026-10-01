import { useEffect, useState } from 'react';
import type { Wallet } from '../api/types';
import { request } from '../api/requests';
import { formatMinorUnits } from './Money';

/**
 * Home widget for the Vault (locked savings). The amount shown is the
 * primary wallet's *held* balance — the ledger's open holds are exactly what
 * a savings lock is (wallet → escrow entries), so this reads the real rails
 * instead of inventing a savings product (checklist §73: no fake data).
 * Loading/error degrade to hiding the card; an empty hold list hides it too,
 * and Home's layout flows on without it.
 */
export function VaultMiniCard({ onOpenAll }: { onOpenAll: () => void }) {
  const [held, setHeld] = useState<number | null>(null);
  const [currency, setCurrency] = useState<'SLE' | 'USD'>('SLE');

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const wallets = await request<Wallet[]>('/wallets');
        const primary = wallets[0];
        if (!cancelled && primary && primary.held_minor > 0) {
          setHeld(primary.held_minor);
          setCurrency(primary.currency);
        }
      } catch {
        // Statement/wallet hiccup: the card simply stays hidden. Home's own
        // state machine already surfaces load failures.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (held === null) return null;

  return (
    <button
      type="button"
      className="vault-card"
      style={{
        textAlign: 'left',
        cursor: 'pointer',
        font: 'inherit',
        color: 'inherit',
        width: '100%',
      }}
      onClick={onOpenAll}
      aria-label="Open your Vault"
    >
      <div className="v-head">
        <div className="v-name">🔒 Vault — locked savings</div>
        <span className="link">Open</span>
      </div>
      <div className="v-amount">{formatMinorUnits(held, currency)}</div>
      <div className="vault-meta">
        <span>Held under your savings locks</span>
        <span>ledger-backed</span>
      </div>
    </button>
  );
}
