import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Unit tests for the device-identity primitives (web/src/lib/device.ts).
 * WebCrypto (crypto.subtle) is jsdom-mocked; the descriptor collection runs
 * against jsdom's navigator/screen stubs.
 */

const mockDigest = vi.hoisted(() => vi.fn());

vi.stubGlobal('crypto', {
  randomUUID: vi.fn(() => 'test-uuid-4'),
  subtle: { digest: mockDigest },
});

import { collectDescriptor, computeFingerprint, getOrCreateDeviceId, serializeDescriptor } from '../device';

describe('device identity', () => {
  beforeEach(() => {
    localStorage.clear();
    mockDigest.mockReset();
    mockDigest.mockResolvedValue(new Uint8Array([0xde, 0xad, 0xbe, 0xef]).buffer);
  });

  it('mints and persists a stable device id', () => {
    const first = getOrCreateDeviceId();
    const second = getOrCreateDeviceId();
    expect(first).toBe(second);
    expect(localStorage.getItem('amberpay.device_id')).toBe(first);
  });

  it('serializes the descriptor deterministically with ordered fields', () => {
    const a = serializeDescriptor(collectDescriptor());
    const b = serializeDescriptor(collectDescriptor());
    expect(a).toBe(b);
    expect(a.split('\u241f')).toHaveLength(10);
  });

  it('hashes the serialized descriptor with SHA-256 and hex-encodes it', async () => {
    const fp = await computeFingerprint();
    // Assert via mock.calls (not expect.any(Uint8Array)) — TextEncoder
    // produces a jsdom-realm Uint8Array that instanceof checks against the
    // test realm would misjudge.
    expect(mockDigest).toHaveBeenCalledTimes(1);
    expect(mockDigest.mock.calls[0][0]).toBe('SHA-256');
    expect(fp).toBe('deadbeef');
  });

  it('falls back to an ephemeral id when localStorage is unavailable', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    const id = getOrCreateDeviceId();
    expect(id).toMatch(/^ephemeral-/);
    vi.mocked(Storage.prototype.setItem).mockRestore();
  });
});
