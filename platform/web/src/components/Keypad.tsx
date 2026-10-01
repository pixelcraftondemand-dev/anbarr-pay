import { IconBackspace } from './Icons';

/**
 * Numeric keypad used for amounts and PIN entry. Keys are type="button" so a
 * keypad inside a form can never submit it.
 */
export function Keypad({
  onDigit,
  onBackspace,
  showBackspace = true,
  compact = false,
}: {
  onDigit: (d: number) => void;
  onBackspace: () => void;
  /** Hide the trailing backspace (e.g. when the flow has its own). */
  showBackspace?: boolean;
  /** Smaller max-width for PIN pads. */
  compact?: boolean;
}) {
  const digit = (d: number) => (
    <button key={d} type="button" className="key" onClick={() => onDigit(d)}>
      {d}
    </button>
  );
  return (
    <div className="keypad" style={compact ? { maxWidth: 220, margin: '10px auto 0' } : undefined}>
      {[1, 2, 3, 4, 5, 6, 7, 8, 9].map(digit)}
      <button type="button" className="key ghost" aria-label="Empty key" disabled />
      {digit(0)}
      {showBackspace ? (
        <button type="button" className="key ghost" aria-label="Delete" onClick={onBackspace}>
          <IconBackspace />
        </button>
      ) : (
        <button type="button" className="key ghost" aria-label="Empty key" disabled />
      )}
    </div>
  );
}

/** Four-dot PIN progress indicator. */
export function PinDots({ filled }: { filled: number }) {
  return (
    <div className="dots" aria-label={`PIN entry ${filled} of 4`}>
      {[0, 1, 2, 3].map((i) => (
        <span key={i} className={`dot${i < filled ? ' filled' : ''}`} />
      ))}
    </div>
  );
}
