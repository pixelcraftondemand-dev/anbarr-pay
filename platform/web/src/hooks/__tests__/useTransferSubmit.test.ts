import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TransferRequest, TransferResponse } from '../../api/types';
import { ApiError } from '../../api/client';

// Mock the API client — the hook's contract is with this module, not the
// network. The PIN verifier is injected as a plain function, so no module
// mock is needed for it.
const apiRequestMock = vi.fn();
const verifyPinMock = vi.fn();

vi.mock('../../api/client', () => ({
  ApiError: class ApiError extends Error {
    constructor(
      public status: number,
      public code: string,
      message: string,
    ) {
      super(message);
    }
  },
  apiRequest: (...args: unknown[]) => apiRequestMock(...args),
}));

import { useTransferSubmit } from '../useTransferSubmit';

const PIN_TOKEN = 'pin-token-123';

const transfer: TransferRequest = {
  recipient_email_or_phone: 'aminata@example.com',
  amount_minor: 450_000,
  currency: 'SLE',
};

const apiResult: TransferResponse = {
  id: 'tx-1',
  status: 'COMPLETED',
  fee_minor: 2_250,
  tax_minor: 0,
  total_minor: 452_250,
};

beforeEach(() => {
  apiRequestMock.mockReset();
  verifyPinMock.mockReset();
  verifyPinMock.mockImplementation((pin: string) => Promise.resolve(`pin-token-${pin}`));
});

/** Render with the injected mock verifier. No access token exists — auth is
 *  not part of the client build right now. */
function setup() {
  return renderHook(() => useTransferSubmit(null, verifyPinMock));
}

describe('useTransferSubmit — send flow', () => {
  it('verifies the PIN first, then posts with Pin-Token and Idempotency-Key', async () => {

    apiRequestMock.mockResolvedValue(apiResult);

    const { result } = setup();
    expect(result.current.state.kind).toBe('idle');

    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));

    expect(verifyPinMock).toHaveBeenCalledWith('1234');
    expect(apiRequestMock).toHaveBeenCalledWith(
      '/transfers',
      expect.objectContaining({
        method: 'POST',
        idempotencyKey: expect.any(String),
        pinToken: 'pin-token-1234',
        token: undefined,
        body: transfer,
      }),
    );
    expect(result.current.state.kind).toBe('result');
  });

  it('rejects an obviously invalid PIN before any network call', async () => {
    const { result } = setup();

    await act(() => result.current.submit({ pin: '12', pinToken: null, transfer }));

    expect(verifyPinMock).not.toHaveBeenCalled();
    expect(apiRequestMock).not.toHaveBeenCalled();
    expect(result.current.state.kind).toBe('error');
  });

  it('renders the server status as-is (never invents success)', async () => {

    apiRequestMock.mockResolvedValue({ ...apiResult, status: 'PENDING' });

    const { result } = setup();
    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));

    expect(result.current.state.kind).toBe('result');
    expect(result.current.state.kind === 'result' && result.current.state.result.status).toBe(
      'PENDING',
    );
  });

  it('surfaces a definitive server rejection as an error with the server message', async () => {

    apiRequestMock.mockRejectedValue(
      new ApiError(422, 'insufficient_funds', 'Not enough available balance.'),
    );

    const { result } = setup();
    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));

    expect(result.current.state.kind).toBe('error');
    expect(result.current.state.kind === 'error' && result.current.state.message).toBe(
      'Not enough available balance.',
    );
  });
});

describe('useTransferSubmit — network-pending semantics', () => {
  it('network loss mid-request becomes pending, never error', async () => {

    apiRequestMock.mockRejectedValue(new ApiError(0, 'network_error', 'unreachable'));

    const { result } = setup();
    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));

    expect(result.current.state).toMatchObject({
      kind: 'pending',
      amountMinor: 450_000,
      recipient: 'aminata@example.com',
    });
  });

  it('retry reuses the SAME idempotency key and held pin_token', async () => {

    apiRequestMock
      .mockRejectedValueOnce(new ApiError(0, 'network_error', 'unreachable'))
      .mockResolvedValueOnce(apiResult);

    const { result } = setup();
    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));
    const firstKey = apiRequestMock.mock.calls[0][1].idempotencyKey;

    await act(() => result.current.retry());
    const secondCall = apiRequestMock.mock.calls[1][1];

    expect(secondCall.idempotencyKey).toBe(firstKey);
    expect(secondCall.pinToken).toBe('pin-token-1234'); // held token reused
    expect(verifyPinMock).toHaveBeenCalledTimes(1); // no re-prompt
    expect(result.current.state.kind).toBe('result');
  });

  it('retry while still unknown stays pending (no false failure)', async () => {

    apiRequestMock.mockRejectedValue(new ApiError(0, 'network_error', 'unreachable'));

    const { result } = setup();
    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));
    await act(() => result.current.retry());

    expect(result.current.state.kind).toBe('pending');
  });
});

describe('useTransferSubmit — PIN retry', () => {
  it('a held token skips re-verification on submit', async () => {
    apiRequestMock.mockResolvedValue(apiResult);

    const { result } = setup();
    await act(() =>
      result.current.submit({ pin: '', pinToken: PIN_TOKEN, transfer }),
    );

    expect(verifyPinMock).not.toHaveBeenCalled();
    expect(apiRequestMock).toHaveBeenCalledWith(
      '/transfers',
      expect.objectContaining({ pinToken: PIN_TOKEN }),
    );
  });

  it('401/403 with a held token discards the token and demands a fresh PIN', async () => {
    apiRequestMock.mockRejectedValue(new ApiError(401, 'pin_token_expired', 'expired'));

    const { result } = setup();
    await act(() =>
      result.current.submit({ pin: '', pinToken: PIN_TOKEN, transfer }),
    );

    expect(result.current.state.kind === 'error' && result.current.state.message).toMatch(
      /expired/i,
    );
    // Token discarded: the next retry must not silently reuse it.
    await act(() => result.current.retry());
    expect(apiRequestMock.mock.calls[1][1].pinToken).not.toBe(PIN_TOKEN);
  });

  it('startOver clears the token and requests a fresh idempotency key', async () => {

    apiRequestMock
      .mockRejectedValueOnce(new ApiError(0, 'network_error', 'unreachable'))
      .mockResolvedValueOnce(apiResult);

    const { result } = setup();
    await act(() => result.current.submit({ pin: '1234', pinToken: null, transfer }));
    const firstKey = apiRequestMock.mock.calls[0][1].idempotencyKey;

    act(() => result.current.startOver());
    expect(result.current.state.kind).toBe('idle');

    await act(() =>
      result.current.submit({
        pin: '5678',
        pinToken: null,
        transfer: { ...transfer, amount_minor: 1 },
      }),
    );
    const secondCall = apiRequestMock.mock.calls[1][1];
    expect(secondCall.idempotencyKey).not.toBe(firstKey);
    expect(secondCall.pinToken).toBe('pin-token-5678');
  });
});
