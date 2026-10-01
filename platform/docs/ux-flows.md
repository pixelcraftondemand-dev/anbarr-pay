# AMBER PAY — UX & Click-by-Click Specification (React)

Frontend: **React (TypeScript, Vite)** SPA — static bundle on CDN, thin view
layer only (`docs/architecture.md` "Frontend client"). Design language:
trust, clarity, accessibility (WCAG 2.2 AA target), responsive (mobile-first
— most users are on phones), fast loading, transparent fees, clear
transaction states. Never fake balances or fake production transactions.

## 1. Screen-by-screen map

```
/                     Landing (public) — product, security, FAQ; no financial claims
/register             Registration (email/phone/password + verification)
/login                Login + MFA step
/forgot-password      Password reset
/kyc                 KYC status + submission + document upload
/wallets              Wallet list + balances (server-authoritative)
/wallets/:id          Wallet detail: available / held / total, transaction history
/send                 Send money flow (multi-step)
/withdraw             Withdraw flow (destination, amount, confirm)
/beneficiaries        Beneficiaries list + add/edit (sensitive actions)
/topup                Mobile-money top-up
/merchant             Merchant dashboard: checkouts, payment links, QR, settlement
/checkout/:id         Buyer-facing checkout page (public)
/pay/:linkId          Payment link page (public, verified)
/transactions         Transaction history + receipts + statements
/transactions/:id     Transaction detail + receipt + dispute entry
/disputes             Disputes: list, create, evidence upload, status
/settings             Profile, notifications, preferences
/settings/security    Security center (password, MFA, passkeys, PIN, recovery)
/settings/security/devices   Device registry
/settings/security/sessions  Active sessions
/settings/api-keys    API keys (merchant/business) — secret shown once
/business             Business accounts: members, roles, approvals, limits, reports
/admin/*              Admin console (role-gated): users, transactions, adjustments,
                      reconciliation, fraud, compliance, config
/support              Support tickets + in-app help
```

## 2. Screen states (§53)

Every screen defines: **loading, empty, success, error, offline, unauthorized,
forbidden, expired, maintenance, retry**. Examples of non-obvious ones:

- **Offline**: banner + cached views only; money-moving actions disabled with
  an explanation ("You're offline — transfers are disabled until you're back
  online"); nothing queued silently client-side.
- **Expired**: checkout/payment-link expiry states with the exact remaining
  validity shown beforehand; session-expiry returns the user to login with a
  safe message and preserves the intended destination.
- **Forbidden vs unauthorized**: 401 → login redirect; 403 → explain the
  missing permission without revealing other users' data.
- **Maintenance**: pre-announced maintenance screen for scheduled windows;
  money movement disabled during them.
- **Retry**: safe retry actions always re-send with the **same idempotency
  key** (never a new one), so retries can't double-post.

## 3. Click-by-click standard (§74)

Each journey below uses this template:

```
Journey / Screen / User action / Frontend validation / Network request /
Authentication / Authorization / Security controls / Fraud controls /
Compliance controls / Database operation / Ledger operation / Provider
operation / Success state / Failure state / Timeout state / Retry behavior /
Audit event / Notification / Final UI state
```

And every **button** uses the §52 template (purpose, preconditions, frontend
validation, backend endpoint, auth, authz, security checks, financial impact,
DB changes, ledger impact, possible states, success, failure, timeout, retry,
audit event, notification). The two full examples below set the standard; the
remaining journeys follow it and are completed as their features build.

---

### 3.1 Journey: Log in (React)

- **Screens**: `/login` (email/password) → optional `/login?step=mfa`.
- **User action**: submit credentials.
- **Frontend validation**: email format, non-empty password; disable button
  while pending.
- **Network request**: `POST /v1/auth/login` (+ `POST /v1/auth/otp/verify`
  for the MFA step) with a fresh idempotency-safe request id.
- **Authentication**: server-side credential check, MFA when required
  (full step list in `docs/authentication.md` §2).
- **Authorization**: session scoped to the user; no role check at login
  beyond account status.
- **Security controls**: rate limiting, no account-existence disclosure,
  device/location risk evaluation, `LOGIN`/`LOGIN_FAILED` audit, security
  notification on new device/location.
- **Fraud controls**: step-up triggers from risk engine
  (`docs/fraud-risk.md`).
- **Compliance controls**: none at login (KYC gates transactions, not login).
- **Database operation**: verify user + password hash, create
  `app.refresh_tokens` row, register/update device.
- **Ledger operation**: none.
- **Provider operation**: none.
- **Success state**: redirect to `/wallets`; tokens per `docs/authentication.md` §3.
- **Failure state**: inline safe message ("Email or password is incorrect");
  `LOGIN_FAILED` event.
- **Timeout state**: same safe message, attempt counted, no lockout info leak.
- **Retry behavior**: re-submit allowed until rate limit; 429 with
  `Retry-After`.
- **Audit event**: `LOGIN` / `LOGIN_FAILED`.
- **Notification**: security email/push on new device or new location.
- **Final UI state**: wallet dashboard hydrated from `GET /v1/wallets`.

### 3.2 Journey: Send money (the critical flow, §15)

- **Screens**: `/send` (recipient, amount, currency, note, funding source) →
  `/send/confirm` (confirmation: recipient, amount, currency, fee, total,
  exchange rate if applicable, funding source, estimated delivery) → result.
- **User action**: fill form → Continue → Confirm (PIN, plus OTP when
  required) → result.
- **Frontend validation**: recipient format (phone/email), amount > 0 and
  within limits, currency selection, note length.
- **Network request**: `POST /v1/transfers` with `Idempotency-Key` generated
  per submission attempt and **reused on retry**; `Pin-Token` header
  (`POST /v1/auth/pin/verify` first); `Otp-Code` when required.
- **Authentication**: PIN (short-lived pin_token), OTP for high value;
  session.
- **Authorization**: ownership of funding source; per-user limits;
  recipient exists and is active.
- **Security controls**: idempotency (double-click, refresh, network loss all
  safe), re-check of balance server-side at confirm, step-up auth rules.
- **Fraud controls**: risk evaluation (velocity, recipient history, device,
  amount) before creation and re-evaluated at confirm
  (`docs/fraud-risk.md`).
- **Compliance controls**: KYC gates, limits, sanctions checks on recipient
  (docs/kyc-aml.md) — `REGULATORY_REVIEW_REQUIRED` for tiers.
- **Database operation**: domain `transfers` row (idempotency key),
  state machine per `docs/transaction-state-machine.md` §3.1.
- **Ledger operation**: `post_payment` journal (payer −amount−fee, payee
  +amount, fee_revenue +fee) atomically with optional agent commission.
- **Provider operation**: none for P2P (wallet-to-wallet).
- **Success state**: `/transactions/:id` with receipt; balances re-fetched.
- **Failure state**: safe message ("Transfer couldn't be completed —
  try again" for generic; specific safe codes for insufficient funds),
  no rule details.
- **Timeout state**: transaction stays PENDING; result screen shows
  "We're confirming your transfer" with a status refresh; never "failed".
- **Retry behavior**: same idempotency key; server replays or continues.
- **Audit event**: `PAYMENT_CREATED`, `PAYMENT_COMPLETED` /
  `PAYMENT_FAILED`.
- **Notification**: push/SMS/email to sender and recipient on completion
  (no sensitive detail beyond amount + counterparty).
- **Final UI state**: wallet view reflects server balance; transaction in
  history with clear state.

### 3.3 Journey: Withdraw (design highlights, §17)

- **Screens**: `/withdraw` (destination select — verified destinations only)
  → amount + fee preview → confirm (strong auth: PIN + MFA for large
  amounts) → result.
- Funds are **held** at creation; the ledger debit happens at provider
  confirmation; a failed payout releases the hold and returns funds —
  **never** `provider failure + funds deducted + no recovery path`.
- Timeout → UNKNOWN with "We're confirming your withdrawal" + status
  refresh; reconciliation resolves.
- Re-auth required to **change withdrawal destination**
  (`docs/authentication.md` §7).

### 3.4 Journey: Add beneficiary (sensitive action)

- **Screens**: `/beneficiaries/new`.
- Flow: `Add recipient → Validate → Authenticate → Risk evaluation → Optional
  cooling-off → Create` (§18).
- Notifications on create/change; new-beneficiary restrictions where
  appropriate; strong authentication (PIN/OTP/MFA); audit
  `BENEFICIARY_CREATED` / `BENEFICIARY_CHANGED`; cooling-off flag for
  high-value destinations where configured.

### 3.5 Journey: Merchant checkout / QR / payment link

- Checkout page (`/checkout/:id`) shows **verified merchant name, amount,
  currency, fee, total** before any authorization (§45).
- QR codes resolve to **verified** AmberPay payment requests: merchant name,
  amount, currency, reference, expiry; amount tampering, expired-request
  reuse, and unauthorized merchant substitution are prevented server-side
  (§46).
- Payment links use cryptographically secure identifiers, expire, are
  revocable, bound to the merchant, tamper-proof on amount, auditable
  (§47).

### 3.6 Journey: Refund / dispute / freeze / security / API keys / business approval

- **Refund** (§35): new financial event; authorization, idempotency, partial
  support, ledger entries, audit, notification, reconciliation. Original
  record never edited.
- **Dispute** (§34): `Transaction → Report a problem → Reason → Explanation
  → Evidence → Submit`; case id, status, timestamps, assignment,
  communication history, evidence, resolution; original financial record
  never modified; funds may be held during dispute.
- **Freeze** (§24): Settings → Security → Freeze my account; immediate
  financial restriction (ledger status check), clear explanation, strong
  auth to unfreeze, notification, audit.
- **Security settings**: per `docs/authentication.md` §7 (password, MFA,
  passkeys, devices, sessions, login history, PIN, recovery, alerts).
- **API key creation**: secret shown **once**; scoped permissions; rotation +
  revocation screens; audit `API_KEY_CREATED`/`API_KEY_REVOKED`.
- **Business approval workflow** (§48): `Employee creates payment → Pending
  approval → Manager approves → Payment executes`; the React client shows the
  approval queue and requires approver identity confirmation.

## 4. Frontend engineering rules

- Balances/statuses are **server responses only**; the client never computes
  totals for display from fragments, never trusts cached balances for
  enabling actions.
- Money input uses integer minor units in the API; the client formats
  currency for display only (locale-aware, explicit currency).
- Every form with financial impact generates its idempotency key **once per
  submission attempt** and reuses it across retries/refreshes of that attempt.
- No secrets in the bundle; no analytics on sensitive screens; error
  boundaries show safe messages with request ids.
- Feature-flagged screens (new rails, new flows) render disabled/unknown
  states rather than hiding errors.
- Accessibility: semantic HTML, keyboard nav, focus management, aria-live for
  async results, contrast AA, reduced-motion respect.

## 5. Remaining journeys to complete with the §52/§74 templates

Registration, KYC, add money (deposit/top-up), receive money, merchant
checkout confirm, QR payment, payment link, refund, dispute, account freeze,
security settings, API key creation, business approval workflow — each is
documented at the same detail as Send money when its feature builds (tracked
in `docs/production-readiness.md`).