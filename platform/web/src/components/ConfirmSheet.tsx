import { useEffect, type ReactNode } from 'react';

/**
 * Bottom confirmation sheet — the prototype's destructive-action pattern
 * (design committee notes: "Destructive actions get a confirm sheet; everything
 * else doesn't"). Escape closes; focus moves to the sheet when opened.
 */
export function ConfirmSheet({
  open,
  title,
  body,
  confirmLabel,
  onConfirm,
  onCancel,
  children,
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  /** Extra content rendered between body and actions (e.g. a reason field). */
  children?: ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onCancel]);

  if (!open) return null;
  return (
    <div
      className="sheet-back"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div className="sheet" role="dialog" aria-modal="true" aria-label={title}>
        <h3>{title}</h3>
        <p>{body}</p>
        {children}
        <div className="sheet-actions">
          <button type="button" className="button button-danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
          <button type="button" className="button button-ghost" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
