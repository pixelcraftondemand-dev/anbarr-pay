import { apiRequest } from './client';

/**
 * Verify the transaction PIN (docs/api.md §1: POST /v1/auth/pin/verify →
 * short-lived pin_token). With no Core API reachable the client throws and
 * money movement reports its honest pending/error states.
 */
export async function verifyPin(pin: string): Promise<string> {
  const res = await apiRequest<{ pin_token: string }>('/auth/pin/verify', {
    method: 'POST',
    body: { pin },
  });
  return res.pin_token;
}
