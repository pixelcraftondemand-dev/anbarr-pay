import { useEffect, useState } from 'react';
import type { Beneficiary } from '../api/types';
import { request } from '../api/requests';

/**
 * Recent/saved recipients for the Send flow (docs/ux-flows.md §3.4). The
 * list comes from GET /v1/beneficiaries — real saved recipients maintained
 * server-side, not a guess from statement rows (statements carry no
 * counterparty identity by design). If the endpoint isn't available in this
 * environment, the picker hides itself: the recipient input still works and
 * nothing is faked.
 */
export function BeneficiaryPicker({
  onSelect,
  refreshKey,
}: {
  /** Called with the chosen beneficiary's handle (email or phone). */
  onSelect: (emailOrPhone: string) => void;
  /** Bump to refetch after the flow adds a beneficiary server-side. */
  refreshKey?: number;
}) {
  const [people, setPeople] = useState<Beneficiary[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const list = await request<Beneficiary[]>('/beneficiaries');
        // Only accept a real array — a malformed/unexpected body means the
        // picker stays hidden rather than crashing the Send flow.
        if (!cancelled && Array.isArray(list)) setPeople(list);
      } catch {
        // Unavailable (404/401/no API yet): hide, don't fake.
        if (!cancelled) setPeople(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refreshKey]);

  if (!people || people.length === 0) return null;

  return (
    <div className="beneficiary-grid" style={{ margin: '4px 0 12px' }}>
      {people.slice(0, 8).map((p) => (
        <button
          key={p.id}
          type="button"
          className="ben"
          onClick={() => onSelect(p.email_or_phone)}
          title={p.email_or_phone}
        >
          <span className="av" aria-hidden>
            {initials(p.name)}
          </span>
          <span>{p.name}</span>
        </button>
      ))}
    </div>
  );
}

function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length >= 2) return (words[0][0] + words[1][0]).toUpperCase();
  return name.slice(0, 2).toUpperCase();
}
