// API types per docs/api.md. Amounts are ALWAYS integer minor units
// (e.g. 1500 = SLE 15.00); the client only formats them for display.

export type Currency = 'SLE' | 'USD';

export interface Wallet {
  id: string;
  currency: Currency;
  /** Σ wallet entries — the unencumbered amount (server-authoritative). */
  available_minor: number;
  /** Σ open holds against this wallet. */
  held_minor: number;
  /** available + held. */
  total_minor: number;
  status: string;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
}

export interface RefreshRequest {
  refresh_token: string;
}

export interface RefreshResponse {
  access_token: string;
  refresh_token: string;
}

export interface RegisterRequest {
  email: string;
  phone: string;
  full_name: string;
  password: string;
}

/** POST /v1/auth/pin/verify → short-lived pin_token (TTL 2 min) used as the
 *  `Pin-Token` header on transaction-confirm endpoints (docs/api.md §1). */
export interface PinVerifyRequest {
  pin: string;
}

export interface PinVerifyResponse {
  pin_token: string;
}

export interface TransferRequest {
  recipient_email_or_phone: string;
  amount_minor: number;
  currency: Currency;
  note?: string;
}

export interface TransferResponse {
  id: string;
  status: string;
  journal_id?: string;
  /** Fee charged to the payer (minor units), as computed by the ledger. */
  fee_minor?: number;
  /** Government transaction tax collected from the payer (e.g. e-levy),
   *  minor units — never part of the platform's fee. */
  tax_minor?: number;
  total_minor?: number;
}

/** POST /v1/topups — wallet-funded mobile-money top-up (docs/api.md §6). */
export interface TopupRequest {
  rail: string;
  destination_phone: string;
  amount_minor: number;
  currency: Currency;
}

export interface TopupResponse {
  id: string;
  status: string;
  rail_reference?: string;
}

/** One line of a wallet statement — mirrors ledger gRPC `ListEntries.Entry`
 *  (ledger/proto/ledger.proto). A journal can produce up to two entries on the
 *  same wallet (e.g. a p2p debit and its fee debit), so (journal_id, direction)
 *  is the natural row identity. */
export interface StatementEntry {
  entry_id: string;
  journal_id: string;
  journal_type: string;
  direction: 'debit' | 'credit' | string;
  amount_minor: number;
  currency: Currency | string;
  /** RFC 3339 timestamp. */
  created_at: string;
}

export interface StatementPage {
  entries: StatementEntry[];
  /** Opaque cursor; empty string = no more pages. */
  next_page_token: string;
}

// --- Beneficiaries (docs/ux-flows.md §3.4 — GET /v1/beneficiaries) ----------

/** A saved recipient. The server resolves the person behind the handle;
 *  the client renders only what the API returns. */
export interface Beneficiary {
  id: string;
  name: string;
  email_or_phone: string;
}

/** problem+json error contract (docs/api.md). */
export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
    field?: string;
    request_id: string;
  };
}

/** Verified identity claims carried by the access token (docs/authentication.md
 *  §2). Read client-side for display only — never for authorization, which is
 *  always server-side. */
export interface Identity {
  sub: string;
  email?: string;
  name?: string;
  exp?: number;
}

// --- Vault (goal savings over ledger holds — docs/vault.md §2) -------------

/** One ledger hold inside a goal. Locks are atomic: unlock releases the
 *  whole hold (the engine has no partial release). */
export interface VaultLock {
  hold_id: string;
  amount_minor: number;
  locked_at: string;
  expires_at?: string;
}

export interface VaultGoal {
  id: string;
  name: string;
  status: 'active' | 'released' | (string & {});
  currency: Currency;
  locked_minor: number;
  target_minor?: number;
  maturity_at?: string;
  locks: VaultLock[];
}

export interface CreateGoalRequest {
  name: string;
  currency: Currency;
  amount_minor: number;
  target_minor?: number;
  maturity_at?: string;
}

export interface CreateGoalResponse {
  id: string;
  status: string;
  locked_minor: number;
  hold_id: string;
}

export interface AddLockResponse {
  hold_id: string;
  locked_minor: number;
}

export interface ReleaseResponse {
  released_minor: number;
  released_holds: number;
}
