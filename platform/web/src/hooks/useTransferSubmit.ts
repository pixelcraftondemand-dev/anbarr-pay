import { useCallback, useRef, useState } from 'react';
import { ApiError } from '../api/client';
import type { Currency, TransferRequest, TransferResponse } from '../api/types';
import { useIdempotentKey } from './useIdempotentKey';

/**
 * PIN verification is a Core-API capability (POST /v1/auth/pin/verify →
 * pin_token, docs/api.md §1). Injected so the hook stays decoupled from where
 * the Core API lives; until a Core API exists this stays unimplemented and
 * money movement honestly reports it can't authorize. No access token is
 * involved — authentication is not part of the client build right now.
 */
export type PinVerifier = (pin: string) => Promise<string>;
export const pinVerificationUnavailable = () =>
  Promise.reject(
    new ApiError(
      503,
      'pin_unavailable',
      'Transfers need the Core API for PIN authorization, which is not connected yet.',
    ),
  );

export type SubmitState =
  | { kind: 'idle' }
  | { kind: 'submitting' }
  | { kind: 'pending'; amountMinor: number; recipient: string }
  | { kind: 'error'; message: string }
  | {
      kind: 'result';
      result: TransferResponse;
      request: TransferRequest;
    };

export interface TransferSubmitInput {
  /** 4–6 digit transaction PIN, as typed by the user. */
  pin: string;
  /** A pin_token from a previous verify in this attempt, if one is held. */
  pinToken: string | null;
  /** The transfer payload (amount, recipient, currency, note). */
  transfer: TransferRequest;
}

/**
 * The transfer submission state machine, extracted from the SendMoney screen so
 * it can be unit-tested without rendering the UI.
 *
 * Rules encoded here (all traced to docs):
 * - PIN is verified first (`POST /auth/pin/verify` → pin_token, TTL 2 min);
 *   a held token is reused for status retries of the SAME attempt.
 * - The POST carries one Idempotency-Key per attempt, reused on every retry —
 *   a retry can never double-post (docs/transaction-state-machine.md §5).
 * - Network failure (status 0) leaves the outcome UNKNOWN: the state becomes
 *   `pending`, never `error` (docs/transaction-state-machine.md §4).
 * - 401/403 with a held token means the pin_token expired: the caller is sent
 *   back to re-enter the PIN (fresh verification), not retried blindly.
 */
export function useTransferSubmit(
  accessToken: string | null,
  verifyPin: PinVerifier = pinVerificationUnavailable,
) {
  const { key, reset } = useIdempotentKey();
  const [state, setState] = useState<SubmitState>({ kind: 'idle' });
  // Refs so retry closures always act on the attempt's original values even
  // if the component re-renders mid-flight.
  const pinTokenRef = useRef<string | null>(null);
  const requestRef = useRef<TransferRequest | null>(null);

  const postTransfer = useCallback(
    async (transfer: TransferRequest, pinToken: string): Promise<TransferResponse> => {
      return apiPost(transfer, pinToken, key, accessToken);
    },
    [key, accessToken],
  );

  const submit = useCallback(
    async (input: TransferSubmitInput): Promise<SubmitState> => {
      if (state.kind === 'submitting') return state;
      if (!/^\d{4,6}$/.test(input.pin) && !input.pinToken) {
        const next: SubmitState = { kind: 'error', message: 'Enter your 4–6 digit PIN.' };
        setState(next);
        return next;
      }

      requestRef.current = input.transfer;
      setState({ kind: 'submitting' });
      try {
        // 1. PIN → pin_token (reuse a held token for the same attempt).
        const token = input.pinToken ?? (await verifyPin(input.pin));
        pinTokenRef.current = token;

        // 2. Idempotent POST with Pin-Token.
        const result = await postTransfer(input.transfer, token);
        const next: SubmitState = { kind: 'result', result, request: input.transfer };
        setState(next);
        return next;
      } catch (err) {
        return applyError(err, input.transfer, setState, pinTokenRef);
      }
    },
    [state.kind, postTransfer],
  );

  /** Re-attempt the SAME submission (same idempotency key, same held token). */
  const retry = useCallback(async (): Promise<SubmitState> => {
    const transfer = requestRef.current;
    if (!transfer) {
      const next: SubmitState = { kind: 'error', message: 'Nothing to retry.' };
      setState(next);
      return next;
    }
    setState({ kind: 'submitting' });
    try {
      // The original PIN is not re-typed on retry; a held pin_token is
      // reused. Without one, retry re-verifies with an empty PIN which the
      // Core API will (correctly) reject — retries after expiry start a new
      // attempt instead.
      const token = pinTokenRef.current ?? (await verifyPin(''));
      const result = await postTransfer(transfer, token);
      const next: SubmitState = { kind: 'result', result, request: transfer };
      setState(next);
      return next;
    } catch (err) {
      return applyError(err, transfer, setState, pinTokenRef);
    }
  }, [postTransfer]);

  /** A genuinely NEW attempt: fresh idempotency key, token discarded. */
  const startOver = useCallback(() => {
    pinTokenRef.current = null;
    requestRef.current = null;
    reset();
    setState({ kind: 'idle' });
  }, [reset]);

  return { state, submit, retry, startOver, pinToken: pinTokenRef };
}

// ---- shared error mapping -------------------------------------------------

type SetState = (s: SubmitState) => void;

async function applyError(
  err: unknown,
  transfer: TransferRequest,
  setState: SetState,
  pinTokenRef: { current: string | null },
): Promise<SubmitState> {
  if (err instanceof ApiError && err.status === 0) {
    // UNKNOWN outcome — pending, never failed. Retry reuses key + token.
    const next: SubmitState = {
      kind: 'pending',
      amountMinor: transfer.amount_minor,
      recipient: transfer.recipient_email_or_phone,
    };
    setState(next);
    return next;
  }
  if (
    err instanceof ApiError &&
    (err.status === 401 || err.status === 403) &&
    pinTokenRef.current
  ) {
    // The held pin_token expired mid-attempt: force a fresh PIN.
    pinTokenRef.current = null;
    const next: SubmitState = {
      kind: 'error',
      message: 'Your authorization expired. Enter your PIN again.',
    };
    setState(next);
    return next;
  }
  const next: SubmitState = {
    kind: 'error',
    message: err instanceof ApiError ? err.message : 'Transfer could not be completed.',
  };
  setState(next);
  return next;
}

// Placed last so the import graph stays readable; the real client is injected
// via the `postTransfer` closure in tests (they mock ../api/client).
import { apiRequest } from '../api/client';
async function apiPost(
  transfer: TransferRequest,
  pinToken: string,
  key: string,
  accessToken: string | null,
): Promise<TransferResponse> {
  return apiRequest<TransferResponse>('/transfers', {
    method: 'POST',
    idempotencyKey: key,
    pinToken,
    token: accessToken ?? undefined,
    body: transfer,
  });
}

export type { Currency };
