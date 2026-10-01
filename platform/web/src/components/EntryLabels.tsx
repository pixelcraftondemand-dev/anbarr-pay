import type { StatementEntry } from '../api/types';

/**
 * Display-only labels for ledger statement entries. The journal_type and
 * direction come straight from the ledger (gRPC ListEntries — see
 * ledger/proto/ledger.proto); nothing here invents a transaction the ledger
 * did not record. Unknown journal types render as their raw ledger value —
 * honest, not guessed.
 */

const JOURNAL_LABELS: Record<string, string> = {
  p2p: 'Transfer',
  topup: 'Top-up',
  cash_in: 'Cash in',
  cash_out: 'Cash out',
  checkout: 'Checkout',
  hold: 'Hold',
  capture: 'Capture',
  release: 'Hold released',
  fee: 'Fee',
  commission: 'Commission',
  reversal: 'Reversal',
  refund: 'Refund',
  adjustment: 'Adjustment',
};

export function journalLabel(type: string): string {
  return JOURNAL_LABELS[type] ?? type;
}

/** Debits leave the wallet; credits arrive. Fee legs are debits too. */
export function isCredit(entry: StatementEntry): boolean {
  return entry.direction === 'credit';
}

/** Initials for the row avatar — the counterparty is not in the statement
 *  (privacy: the ledger records accounts, not names), so rows show the
 *  journal type, not a person. */
export function entryAvatar(entry: StatementEntry): string {
  const label = journalLabel(entry.journal_type);
  const words = label.split(/[\s_]+/).filter(Boolean);
  if (words.length >= 2) return (words[0][0] + words[1][0]).toUpperCase();
  return label.slice(0, 2).toUpperCase();
}

export function formatEntryTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}
