# AMBER PAY — API Surface Proposal

- **External API**: REST over HTTPS, JSON, versioned `/v1` (e.g. `https://api.amberpay.sl/v1`).
- **Auth**: `Authorization: Bearer <access_token>` (user/session) or `X-API-Key` (merchant server-to-server). PIN verification issues a short-lived (e.g. 2-minute) `pin_token` used on transaction-confirm endpoints.
- **Idempotency**: every `POST` accepts `Idempotency-Key` header; duplicates replay the original response (204/200 with cached body).
- **Errors**: RFC 7807-style problem+json: `{ "error": { "code", "message", "field?", "request_id" } }`. HTTP codes: 400 validation, 401 auth, 403 forbidden, 404, 409 conflict/idempotency mismatch, 422 insufficient funds/hold expiry, 429 rate limited (`Retry-After`).
- **Amounts**: always minor units (integer cents) — `"amount_minor": 1500` means SLE 15.00.
- **Pagination**: cursor-based (`?cursor=&limit=`, default 25, max 100) for list endpoints.
- **Internal**: Java↔Rust uses gRPC (`docs/architecture.md` §8). Rails callbacks arrive on public endpoints below.

---

## 1. Auth & Identity

| Method | Path | Description |
|---|---|---|
| POST | `/v1/auth/register` | `{email, phone, full_name, password, kyc:{id_type, id_number}}` → creates user + SLE wallet (+ USD wallet). 201. |
| POST | `/v1/auth/login` | `{email, password}` → `{access_token, refresh_token}` |
| POST | `/v1/auth/refresh` | `{refresh_token}` → new access token |
| POST | `/v1/auth/logout` | revoke refresh token |
| POST | `/v1/auth/pin/set` | `{pin, otp}` — set/change PIN (OTP for change) |
| POST | `/v1/auth/pin/verify` | `{pin}` → `{pin_token}` (TTL 2 min) used on confirm endpoints |
| POST | `/v1/auth/otp/request` | `{purpose}` → sends OTP (SMS, email fallback) |
| POST | `/v1/auth/otp/verify` | `{purpose, code}` → ok |
| POST | `/v1/auth/password/reset` | `{email, otp, new_password}` |

## 2. KYC

| Method | Path | Description |
|---|---|---|
| GET | `/v1/kyc` | current KYC status + fields |
| PUT | `/v1/kyc` | `{id_type, id_number}` — re-submit; status → `pending` |
| POST | `/v1/kyc/documents` | multipart upload of ID document → `kyc_documents` row |
| GET | `/v1/kyc/documents` | list documents + status |

## 3. Wallets

| Method | Path | Description |
|---|---|---|
| GET | `/v1/wallets` | list: `{id, currency, available_minor, held_minor, total_minor}` |
| GET | `/v1/wallets/{id}` | detail incl. status |
| GET | `/v1/wallets/{id}/transactions` | paginated ledger statement (from gRPC `ListEntries`) |
| GET | `/v1/wallets/{id}/holds` | open holds |

## 4. P2P Transfers

| Method | Path | Description |
|---|---|---|
| POST | `/v1/transfers` | `{recipient_email_or_phone, amount_minor, currency, note}` + `Pin-Token` header. High value (> threshold) requires `Otp-Code` too. |
| GET | `/v1/transfers/{id}` | status + journal reference |

## 5. Merchant Checkout

Merchant authenticates with `X-API-Key`; buyer is the customer.

| Method | Path | Description |
|---|---|---|
| POST | `/v1/checkouts` | merchant side: `{amount_minor, currency, payment_method, buyer:{email, phone}, payment_code, return_url, metadata}` → `{id, status, checkout_url}` |
| GET | `/v1/checkouts/{id}` | status poll (merchant or buyer) |
| POST | `/v1/checkouts/{id}/confirm` | buyer: `{pin_token}` — confirms wallet-funded checkout (captures hold) |
| POST | `/v1/checkouts/{id}/pay-card` | buyer: `{payment_method_id}` → 3DS redirect URL |
| POST | `/v1/checkouts/{id}/pay-mobile-money` | buyer: `{rail, phone}` → rail flow |
| POST | `/v1/checkouts/{id}/cancel` | merchant or buyer; releases hold if held |
| POST | `/v1/checkouts/{id}/refund` | merchant: full/partial refund → reversal journal |

## 6. Mobile Money Top-ups & Bills

| Method | Path | Description |
|---|---|---|
| POST | `/v1/topups` | user: `{rail, destination_phone, amount_minor, currency}` — wallet-funded; `Pin-Token` + OTP if high value |
| GET | `/v1/topups/{id}` | status incl. `rail_reference` |
| POST | `/v1/bills` | (future) bill payments — same shape as topups with `bill_provider` |

## 6b. Vault (goal savings over ledger holds — see docs/vault.md)

| Method | Path | Description |
|---|---|---|
| GET | `/v1/vault/goals` | list goals incl. open locks (hold ids, amounts, expiry) |
| POST | `/v1/vault/goals` | create + first lock: `{name, currency, amount_minor, target_minor?, maturity_at?}` + `Pin-Token` → 201 |
| POST | `/v1/vault/goals/{id}/locks` | add a lock: `{amount_minor}` + `Pin-Token` |
| POST | `/v1/vault/goals/{id}/release` | unlock: `{hold_id?}` (omit = all open holds) + `Pin-Token` |

Each lock maps 1:1 to a ledger `HoldFunds`; release maps to `ReleaseHold`
(whole-hold, no partial unlock). KYC tier gates goal count and caps.

## 6c. Beneficiaries

| Method | Path | Description |
|---|---|---|
| GET | `/v1/beneficiaries` | saved recipients `{id, name, email_or_phone}` — feeds the Send flow's picker |
| POST | `/v1/beneficiaries` | (future) add manually — sensitive action: strong auth + cooling-off (§18) |
| PUT | `/v1/beneficiaries/{id}` | (future) edit |
| DELETE | `/v1/beneficiaries/{id}` | (future) remove |

The server derives recipients from paid transfers; the client never infers
them from statement rows (statements carry no counterparty identity by
design).

## 7. Agent Cash-in / Cash-out

| Method | Path | Description |
|---|---|---|
| POST | `/v1/agents/cash-in` | agent: `{user_phone, amount_minor, currency}` → customer wallet credited; agent float debited; commission credited |
| POST | `/v1/agents/cash-out` | agent: `{user_phone, amount_minor, currency}` → customer wallet debited; agent float credited |
| GET | `/v1/agents/float` | float balance + today's volume |
| GET | `/v1/agents/transactions` | paginated movement history |

## 8. Cards (after PSP selection)

| Method | Path | Description |
|---|---|---|
| POST | `/v1/payment-methods/cards` | `{psp_token}` → save card for buyer |
| GET | `/v1/payment-methods` | saved cards |
| POST | `/v1/payment-methods/cards/{id}/3ds-challenge` | complete 3DS challenge (redirect back from PSP) |

## 9. Inbound rail callbacks (from rails/PSP)

| Method | Path | Description |
|---|---|---|
| POST | `/v1/rails/{rail}/callbacks` | Orange Money / Afrimoney / PSP / bank notifications. Treated as acceleration signals; reconciliation is source of truth. Authenticated per rail. |
| GET | `/v1/rails/{rail}/status` | ops health check per rail adapter |

## 10. Admin / Ops (role-gated)

| Method | Path | Description |
|---|---|---|
| GET | `/v1/admin/users` | search users |
| POST | `/v1/admin/users/{id}/suspend` | suspend/close |
| GET | `/v1/admin/transactions` | search journals (via ledger read) |
| POST | `/v1/admin/adjustments` | manual correction journal (requires two approvers) |
| GET | `/v1/admin/reconciliations` | run history |
| POST | `/v1/admin/reconciliations/run` | `{rail, period}` — trigger on-demand run |
| GET | `/v1/admin/fraud` | rule hits, velocity breaches |
| PUT | `/v1/admin/velocity-limits` | update limits config |

---

## 11. Outbound Webhook Events (to merchants)

Delivered to `merchants.webhook_url`, signed `X-Amber-Signature: sha256=HMAC(webhook_secret, raw_body)`, retried with exponential backoff + jitter (e.g. 1m → 2m → 4m … max 10 attempts, then dead-letter).

| Event | Trigger |
|---|---|
| `checkout.created` | checkout created |
| `checkout.succeeded` | funds captured to merchant wallet |
| `checkout.failed` | payment failed / declined |
| `checkout.expired` | hold expired / checkout abandoned |
| `checkout.refunded` | refund posted |
| `transfer.completed` | P2P settled |
| `topup.completed` | mobile money top-up confirmed by rail |
| `agent.cashin.completed` / `agent.cashout.completed` | cash movement posted |

Payload shape (consistent across events):

```json
{
  "id": "evt_...",
  "type": "checkout.succeeded",
  "created_at": "2026-09-04T10:00:00Z",
  "data": {
    "checkout_id": "...",
    "journal_id": "...",
    "amount_minor": 1500,
    "currency": "SLE",
    "payment_method": "wallet",
    "payment_code": "...",
    "metadata": {}
  }
}
```

---

## 12. Internal gRPC — Ledger Service

Full proto definition will be written in step 1 of the build; the interface (from `docs/architecture.md` §8):

```
HoldFunds / CaptureHold / ReleaseHold   — holds lifecycle
PostTransfer / PostFee                  — journal posting (P2P, topups, cash movements, standalone fees)
GetAccount / ListEntries                — balances & statements
AuditAccounts                           — internal recompute job
```

Every request carries `idempotency_scope`, `idempotency_key`, and origin metadata (`origin_user_id`, `origin_channel`, `origin_session_id`, `payment_code`) so the ledger itself records full traceability.