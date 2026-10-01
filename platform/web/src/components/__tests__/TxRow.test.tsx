import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { TxRow } from '../TxRow';
import { entryAvatar, formatEntryTime, isCredit, journalLabel } from '../EntryLabels';
import type { StatementEntry } from '../../api/types';

const entry = (overrides: Partial<StatementEntry> = {}): StatementEntry => ({
  entry_id: 'e-1',
  journal_id: '123e4567-e89b-12d3-a456-426614174000',
  journal_type: 'p2p',
  direction: 'debit',
  amount_minor: 450_000,
  currency: 'SLE',
  created_at: '2026-09-10T12:34:56Z',
  ...overrides,
});

// TxRow navigates to the receipt screen, so it needs a Router context.
function renderRow(e: StatementEntry) {
  return render(
    <MemoryRouter>
      <TxRow entry={e} />
    </MemoryRouter>,
  );
}

describe('TxRow — statement rendering', () => {
  it('renders a p2p debit with the ledger label and a minus amount', () => {
    renderRow(entry());
    expect(screen.getByText('Transfer')).toBeInTheDocument();
    expect(screen.getByText('−SLE 4500.00')).toBeInTheDocument();
    expect(screen.getByText(/Debited/)).toBeInTheDocument();
  });

  it('renders a topup credit as money-in with a plus amount', () => {
    renderRow(
      entry({
        journal_type: 'topup',
        direction: 'credit',
        amount_minor: 2_500_000,
      }),
    );
    expect(screen.getByText('Top-up')).toBeInTheDocument();
    expect(screen.getByText('+SLE 25000.00')).toBeInTheDocument();
    expect(screen.getByText(/Credited/)).toBeInTheDocument();
  });

  it('shows an unknown journal type as its raw ledger value (no invention)', () => {
    renderRow(entry({ journal_type: 'mystery_rail' }));
    expect(screen.getByText('mystery_rail')).toBeInTheDocument();
  });

  it('shows the debit direction for fee legs', () => {
    renderRow(entry({ journal_type: 'fee', amount_minor: 2250 }));
    expect(screen.getByText('Fee')).toBeInTheDocument();
    expect(screen.getByText('−SLE 22.50')).toBeInTheDocument();
  });
});

describe('EntryLabels — pure helpers', () => {
  it('maps known journal types and passes unknown ones through', () => {
    expect(journalLabel('p2p')).toBe('Transfer');
    expect(journalLabel('cash_in')).toBe('Cash in');
    expect(journalLabel('unknown_x')).toBe('unknown_x');
  });

  it('classifies direction', () => {
    expect(isCredit(entry({ direction: 'credit' }))).toBe(true);
    expect(isCredit(entry({ direction: 'debit' }))).toBe(false);
  });

  it('builds two-letter avatars from the label', () => {
    expect(entryAvatar(entry({ journal_type: 'cash_in' }))).toBe('CI');
    expect(entryAvatar(entry({ journal_type: 'p2p' }))).toBe('TR');
  });

  it('formats timestamps in a stable locale-independent shape', () => {
    // The component uses toLocaleString; assert it produces a non-empty,
    // date-like string rather than pinning a locale.
    const out = formatEntryTime('2026-09-10T12:34:56Z');
    expect(out).toMatch(/\d/);
    expect(out.length).toBeGreaterThan(4);
  });

  it('returns an empty string for invalid timestamps rather than "Invalid Date"', () => {
    expect(formatEntryTime('not-a-date')).toBe('');
  });
});
