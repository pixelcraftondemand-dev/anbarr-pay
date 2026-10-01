import type { Currency } from '../api/types';

/**
 * Display-only formatting. All amounts arrive as integer minor units
 * (docs/api.md); the client never computes financial totals — it only formats
 * what the server sent.
 */
export function formatMinorUnits(minor: number, currency: Currency): string {
  return `${currency} ${(minor / 100).toFixed(2)}`;
}

export function Money({
  minor,
  currency,
  className,
}: {
  minor: number;
  currency: Currency;
  className?: string;
}) {
  return <span className={className}>{formatMinorUnits(minor, currency)}</span>;
}