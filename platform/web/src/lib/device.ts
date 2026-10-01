/**
 * Device identity primitives (client half of docs/authentication.md §11).
 *
 * What a browser can honestly do:
 * - mint a random, stable `device_id` in localStorage;
 * - collect a *descriptive* fingerprint from harmless browser surfaces;
 * - hash that descriptor with SHA-256 (WebCrypto) so the raw descriptor
 *   never leaves the device.
 *
 * What it cannot do: make the fingerprint cryptographically unforgeable.
 * The server-side binding contract (device attestation, IP/ASN checks,
 * push challenges) is specified in docs/authentication.md §11 — the client
 * feeds it evidence; the Core API decides.
 */

export interface DeviceDescriptor {
  userAgent: string;
  platform: string;
  languages: string;
  timezone: string;
  screen: string;
  colorDepth: number;
  hardwareConcurrency: number | null;
  deviceMemory: number | null;
  touchPoints: number | null;
  /** Set by the SPA build; distinguishes this deployment's clients. */
  appVersion: string;
}

export function collectDescriptor(): DeviceDescriptor {
  const nav = navigator as Navigator & {
    hardwareConcurrency?: number;
    deviceMemory?: number;
    maxTouchPoints?: number;
  };
  return {
    userAgent: navigator.userAgent,
    platform: navigator.platform ?? 'unknown',
    languages: (navigator.languages ?? [navigator.language]).join(','),
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone ?? 'unknown',
    screen: `${screen.width}x${screen.height}@${window.devicePixelRatio}`,
    colorDepth: screen.colorDepth,
    hardwareConcurrency: nav.hardwareConcurrency ?? null,
    deviceMemory: nav.deviceMemory ?? null,
    touchPoints: nav.maxTouchPoints ?? null,
    appVersion: import.meta.env.VITE_APP_VERSION ?? 'dev',
  };
}

/** Canonical wire form: newline-stable, whitespace-insensitive, ordered. */
export function serializeDescriptor(d: DeviceDescriptor): string {
  return [
    d.userAgent,
    d.platform,
    d.languages,
    d.timezone,
    d.screen,
    String(d.colorDepth),
    d.hardwareConcurrency == null ? '' : String(d.hardwareConcurrency),
    d.deviceMemory == null ? '' : String(d.deviceMemory),
    d.touchPoints == null ? '' : String(d.touchPoints),
    d.appVersion,
  ].join('\u241f'); // ␟ unit separator — never appears in the fields above
}

/** Hex-encode a buffer without pulling in a hashing dependency. */
function toHex(buf: ArrayBuffer): string {
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/** Stable per-device installation identifier (UUID in localStorage). */
export function getOrCreateDeviceId(): string {
  const KEY = 'amberpay.device_id';
  try {
    const existing = localStorage.getItem(KEY);
    if (existing) return existing;
    const id = crypto.randomUUID();
    localStorage.setItem(KEY, id);
    return id;
  } catch {
    // Private mode with storage disabled: per-session identity is better
    // than crashing — such a browser is never trustable anyway.
    return `ephemeral-${crypto.randomUUID()}`;
  }
}

/**
 * SHA-256 of the serialized descriptor. Deterministic per device profile:
 * two browsers reporting the same surfaces collide (rare), one browser
 * changing a surface (zoom, GPU-driven UA changes) may drift (handled by
 * re-binding, never by silently trusting).
 */
export async function computeFingerprint(): Promise<string> {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(serializeDescriptor(collectDescriptor())),
  );
  return toHex(digest);
}
