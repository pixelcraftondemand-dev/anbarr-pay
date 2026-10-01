import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { apiRequest, ApiError } from '../client';

// jsdom's fetch is fine for tests that stub globalThis.fetch.
const fetchMock = vi.fn();

beforeEach(() => {
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function jsonResponse(body: unknown, init: ResponseInit = {}) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
}

describe('apiRequest — headers the Core API contract requires', () => {
  it('emits Authorization, Idempotency-Key, and Pin-Token when provided', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ ok: true }));
    await apiRequest('/transfers', {
      method: 'POST',
      token: 'jwt',
      idempotencyKey: 'key-1',
      pinToken: 'pin-1',
      body: { amount_minor: 100 },
    });
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers['Authorization']).toBe('Bearer jwt');
    expect(headers['Idempotency-Key']).toBe('key-1');
    expect(headers['Pin-Token']).toBe('pin-1');
    expect(headers['Content-Type']).toBe('application/json');
  });

  it('omits auth headers when not provided', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ ok: true }));
    await apiRequest('/auth/login', { method: 'POST', body: { email: 'a@b.c' } });
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers['Authorization']).toBeUndefined();
    expect(headers['Pin-Token']).toBeUndefined();
  });
});

describe('apiRequest — error contract (problem+json)', () => {
  it('parses the safe server message, code, request_id, and field', async () => {
    fetchMock.mockResolvedValue(
      jsonResponse(
        {
          error: {
            code: 'insufficient_funds',
            message: 'Not enough available balance.',
            request_id: 'req-9',
            field: 'amount',
          },
        },
        { status: 422 },
      ),
    );
    const err = (await apiRequest('/transfers', { method: 'POST' }).catch((e) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(422);
    expect(err.code).toBe('insufficient_funds');
    expect(err.message).toBe('Not enough available balance.');
    expect(err.requestId).toBe('req-9');
    expect(err.field).toBe('amount');
  });

  it('carries Retry-After from a 429', async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ error: { code: 'rate_limited', message: 'Slow down.', request_id: 'r' } }, {
        status: 429,
        headers: { 'Retry-After': '30' },
      }),
    );
    const err = (await apiRequest('/auth/login', { method: 'POST' }).catch((e) => e)) as ApiError;
    expect(err.status).toBe(429);
    expect(err.retryAfter).toBe(30);
  });

  it('network failure maps to status 0 with the pending-oriented message', async () => {
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    const err = (await apiRequest('/transfers', { method: 'POST' }).catch((e) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(0);
    expect(err.message).toMatch(/still being confirmed/i);
  });

  it('a malformed error body still yields an ApiError, not a crash', async () => {
    fetchMock.mockResolvedValue(new Response('<html>bad gateway</html>', { status: 502 }));
    const err = (await apiRequest('/wallets').catch((e) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(502);
  });
});
