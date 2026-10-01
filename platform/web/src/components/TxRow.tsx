import type { StatementEntry } from '../api/types';
import { formatMinorUnits } from './Money';
import { entryAvatar, isCredit, journalLabel, formatEntryTime } from './EntryLabels';
import { useNavigate } from 'react-router-dom';

/**
 * One ledger statement row. The ledger is the only source of truth: label,
 * direction and amount come verbatim from the entry; the UI adds no
 * interpretation beyond formatting (docs/ux-flows.md §1). Rows link to the
 * transaction detail/receipt when the flow exposes a GET-by-id (transfers,
 * topups); other journal types render non-navigably.
 */
export function TxRow({ entry }: { entry: StatementEntry }) {
  const credit = isCredit(entry);
  const amount = formatMinorUnits(Math.abs(entry.amount_minor), entry.currency as 'SLE');
  const navigate = useNavigate();
  // The statement records journals, not domain ids — a transfer's receipt is
  // fetched by transfer id, so we only link flows that can be resolved. Until
  // the API exposes journal→domain lookup, rows deep-link only via search
  // params on the detail screen (which tries /transfers/{id} then /topups/{id}).
  const openable = entry.journal_type === 'p2p' || entry.journal_type === 'topup';
  const open = () => {
    if (openable) navigate(`/transactions/${entry.journal_id}`);
  };
  return (
    <div
      className="tx-row"
      onClick={open}
      style={openable ? { cursor: 'pointer' } : undefined}
      role={openable ? 'button' : undefined}
      tabIndex={openable ? 0 : undefined}
      onKeyDown={(e) => {
        if (openable && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault();
          open();
        }
      }}
    >
      <div className="tx-avatar" aria-hidden>
        {entryAvatar(entry)}
      </div>
      <div className="tx-mid">
        <div className="tx-name">{journalLabel(entry.journal_type)}</div>
        <div className="tx-meta">
          <span>{formatEntryTime(entry.created_at)}</span>
          <span>
            {credit ? 'Credited' : 'Debited'} · journal {entry.journal_id.slice(0, 8)}
          </span>
        </div>
      </div>
      <div className={`tx-amt${credit ? ' pos' : ''}`}>
        {credit ? '+' : '−'}
        {amount}
      </div>
    </div>
  );
}
