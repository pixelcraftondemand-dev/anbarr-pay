# AMBER PAY — Authentication & Session Design

Owned by the Java Core API (`app` schema); the React SPA is the primary
client. Rules that are absolute:

- Never store plaintext passwords, PINs, OTPs, or recovery codes.
- Never log authentication secrets.
- Never leak whether an account exists (login, register, reset must use
  indistinguishable responses).
- Authentication decisions are always server-side; the React client only
  renders state returned by the API.

Password hashing: **Argon2id** (OWASP-recommended; memory-hard against GPU
cracking). PINs: Argon2id with separate salts/parameters. OTPs: single-use,
TTL ≈ 5 min, stored hashed. Recovery codes: stored hashed, shown once.

---

## 1. Registration

| Step | Detail |
|---|---|
| Screen | `/register` — email, phone (E.164), full name, password, optional initial KYC fields |
| Frontend validation | format checks (email, phone, password strength ≥ 12 chars), terms checkbox |
| API | `POST /v1/auth/register` (idempotency key required) |
| Backend | validates, checks uniqueness (email CITEXT, phone), hashes password (Argon2id), creates `app.users` row (`status=active`, `kyc_status=unverified`), creates SLE + USD wallet accounts via the ledger (`create_account`) |
| Verification | phone + email OTP required before first transaction (`otp_codes` rows, purpose `verify_phone` / `verify_email`); account may log in before verification but is **transaction-limited** until verified |
| Security events | `USER_REGISTERED`, `PHONE_VERIFIED`, `EMAIL_VERIFIED` |
| Failure states | duplicate email/phone → safe "already registered" prompt with login/reset link (no enumeration of which field matched); validation errors → inline messages; 429 on OTP abuse |

## 2. Login flow — every step documented (§9)

**Screen: `Log in`** (React route `/login`; email + password, optional
"MFA code" step).

1. **UI validation** — format checks; no network call yet. Disable submit while
   pending (prevents double-submit).
2. **API request** — `POST /v1/auth/login` with email + password. Every login
   carries a request id (correlation id) used across audit + rate limit + logs.
3. **Rate-limit check** — token bucket per (account, IP, device). Exceeded →
   `429` with `Retry-After`; progressive backoff after repeated failures
   (e.g. 5→10→30 min, exponential with cap); no message distinguishing
   "too many attempts for this account" from generic throttling.
4. **Credential validation** — verify Argon2id hash. On failure, run a dummy
   hash comparison (constant-time behavior) and record `LOGIN_FAILED`.
5. **Account-status check** — suspended/closed accounts receive the generic
   failure message (`AUTH_FAILED` — identical wording to wrong password) and a
   `LOGIN_FAILED` event with `reason=account_status`. Frozen accounts may log
   in but are blocked from financial actions at the authorization layer.
6. **Device-risk evaluation** — device fingerprint (registered in the device
   registry, `docs/security-controls.md`): is this a known device?
   First-seen device → risk signal → step-up (MFA) and `DEVICE_ADDED` audit +
   security notification to the account owner.
7. **IP/network-risk evaluation** — new location / unusual network → risk
   signal; may force MFA or block per fraud rules (`docs/fraud-risk.md`).
8. **MFA requirement** — required when: account has MFA enabled, device is
   new, location is new, or a step-up rule fired. MFA code verified server-side
   (TOTP authenticator, or OTP fallback single-use).
9. **Session creation** — on success: short-lived access token (15 min) +
   refresh token (rotating, 30-day absolute lifetime, inactivity timeout
   14 days); session row in `app.refresh_tokens` (hash only). If device is
   unknown, register it now.
10. **Audit event** — `LOGIN` (actor, target, timestamp, result, request id,
    IP, device id, MFA method used).
11. **Security notification** — `login` email/push when the login is from a
    new device/location; always for accounts with security notifications
    enabled.
12. **UI response** — React stores tokens per `docs/authentication.md` §4,
    redirects to the dashboard, and hydrates the wallet/transaction view from
    `GET /v1/wallets`.

**Every failure defines:** error state (inline message + shake/aria-live),
safe user message ("Email or password is incorrect" — never "account not
found"), security event (`LOGIN_FAILED` with reason), retry behavior (same
form, attempt counter incremented), rate-limit behavior (429 + progressive
delays; no lockout countdown disclosure beyond `Retry-After`).

## 3. Session security (§10)

- **Access tokens**: short-lived (15 min), in memory (React) or HttpOnly
  cookies (web). Not in localStorage when avoidable; if bearer-in-header is
  used, tokens live in memory only.
- **Refresh tokens**: rotating — every refresh issues a new refresh token and
  revokes the old; reuse of a revoked token (token theft signal) revokes the
  whole session family and triggers `SESSION_REUSE_DETECTED` + freeze review.
  Stored hashed in `app.refresh_tokens`.
- **Revocation**: logout revokes the refresh token (and access token expiry
  window); "log out everywhere" revokes all sessions for the account.
- **Lifetime**: inactivity timeout 14 days; absolute session lifetime 30 days;
  both configurable.
- **Session/device list**: `Settings → Security → Sessions` shows active
  sessions (device, IP, last active) with per-session revoke.
- **Session reuse detection**: access-token family reuse or refresh-token
  rotation violations → alert + revoke-all.

## 4. MFA (§10)

- Primary: **TOTP authenticator app** (RFC 6238, 30 s window, server secret
  stored encrypted at rest).
- Fallback: **OTP** via SMS (email fallback — F6, `docs/architecture.md`),
  single-use, TTL 5 min, hashed, per-purpose, re-issue rate-limited (max ~3
  per 10 min), attempt-limited (5, then lock for 15 min).
- **Passkeys/WebAuthn** where appropriate (platform authenticators; exempt
  from TOTP step-up on trusted devices) — deferred behind a feature flag.
- **Recovery codes**: generated at MFA enrollment, shown once, stored hashed;
  each code single-use; revoke-all on use with audit + notification.
- **Disabling MFA requires re-authentication** (password + current TOTP/OTP)
  and a cooling-off review for high-risk accounts.

## 5. Transaction PIN (§13)

- Separate authorization factor for money movement; Argon2id-hashed.
- Set/change requires OTP (`POST /v1/auth/pin/set`).
- **Attempt limits**: 5 failed attempts → 1 min lockout; progressive: 10 → 15
  min, then temporary protection (support review) after 15.
- Verification issues a short-lived `pin_token` (TTL 2 min) consumed by
  transaction-confirm endpoints (matches `docs/api.md`).
- Reset/recovery: OTP + device verification + support escalation path;
  audit events `PIN_SET`, `PIN_CHANGED`, `PIN_RESET`, `PIN_FAILED`.

## 6. Device registry (§11)

`app.devices` (new table — lands with the Java core build):

| Field | Purpose |
|---|---|
| id, user_id | identity |
| device_identifier | fingerprint (never a raw fingerprint string in plaintext logs) |
| platform, app_version, user_agent | context |
| first_seen, last_seen | recency |
| risk_status | `trusted` / `known` / `new` / `high_risk` |
| auth_history | last auth result + method |

Security screen `Settings → Security → Devices`: view, rename, revoke,
revoke-all. **High-risk devices** (flagged by fraud signals) trigger stronger
authentication for sensitive actions (withdrawals, beneficiary changes).

## 7. Account Security Center (§12)

`Settings → Security` exposes: password management, MFA management, passkeys,
devices, active sessions, login history, transaction PIN, recovery methods,
security alerts, and "freeze my account" (§ `docs/fraud-risk.md`).

**Re-authentication (password or PIN + MFA/OTP) is required for:**
changing phone number, changing email, disabling MFA, changing transaction
PIN, adding a beneficiary, changing a beneficiary, large withdrawals, changing
a withdrawal destination, generating API keys. Each is also an audit event
and a security notification.

## 8. Password reset

`POST /v1/auth/password/reset` — email/phone + OTP + new password. Same
response for unknown accounts (no enumeration). New password screened against
breached-password lists. On success: revoke all sessions except the current
one, `PASSWORD_CHANGED` audit, security notification, optional temporary
step-up for the next 24 h.

## 9. Audit events (auth subset)

`LOGIN`, `LOGIN_FAILED`, `LOGOUT`, `USER_REGISTERED`, `PHONE_VERIFIED`,
`EMAIL_VERIFIED`, `PASSWORD_CHANGED`, `PASSWORD_RESET`, `MFA_ENABLED`,
`MFA_DISABLED`, `PIN_SET`, `PIN_CHANGED`, `PIN_RESET`, `PIN_FAILED`,
`DEVICE_ADDED`, `DEVICE_REVOKED`, `SESSION_REVOKED`, `SESSION_REUSE_DETECTED`,
`RECOVERY_CODE_USED`. Full audit design (tamper-resistance, retention) in
`docs/security-controls.md` §4.

## 10. WhatsApp phone sign-in (strict device policy)

**Status: built, staged.** Email OTP is the live default channel (free via
Supabase's built-in mailer; upgrade to a free-tier SMTP provider — Brevo
300/day, Resend 100/day — to lift the ~4/hour built-in cap). WhatsApp is the
desired mobile-first channel for Sierra Leone (high WhatsApp penetration, no
per-message cost to the user) but delivery requires a **Twilio WhatsApp
sender** (paid); the client ships with the flow complete and the toggle
available, reporting honestly that phone sign-in is not enabled until the
sender is configured.

### 10.1 Flow

1. User enters a national SL number (`076 123456`); the client normalizes it
   to E.164 (`+23276123456`) before anything is sent (`web/src/lib/phone.ts`).
2. `signInWithOtp({ phone, options: { channel: 'whatsapp', shouldCreateUser: true } })`.
3. The 6-digit code verifies with `type: 'sms'` (Supabase verifies every phone
   channel as `sms`), TTL 60 min (Supabase default), resend cooldown 60 s.
4. On success the **device is bound immediately** (§11) — before the session
   is used for anything.

### 10.2 Why strict device binding (threat model)

WhatsApp numbers can be re-registered on another phone (SIM swap, shared
handsets, cloned app backups). An OTP alone proves *number possession at
sign-in*, not *who is asking*. The strict policy closes the gap:

- **Session stealing via copied storage** — the most practical web attack:
  export `localStorage` from the victim's browser, import it elsewhere, and
  the session tokens come along. Binding makes the imported session
  worthless: the fingerprint check fails and the session signs out.
- **Shared/family phones** — binding is per (user, device); a second account
  on the same phone gets its own binding (shared-device use is normal in this
  market — not an attack signal).
- **OTP forwarding** — a fraudster who social-engineers the code still has
  to verify from a device the victim never touches; the binding only ever
  binds *their* device, and the victim's next load of the app from a NEW
  device shows the mismatch and signs the session out.

### 10.3 Hard rules

- A WhatsApp OTP session can **never move money by itself** — every
  transaction requires the transaction PIN (`pin_token`) regardless of how
  fresh the session is (docs/api.md §1).
- OTP attempts are locked **per device**: 5 wrong codes → 1 min; 10
  cumulative → 15 min (`web/src/lib/deviceTrust.ts`), mirroring §5.
- New device + phone sign-in = **re-auth required for sensitive actions**
  (change PIN, add beneficiary, change withdrawal destination) even when the
  session is valid.
- Phone-number changes require the full re-authentication path of §7 —
  a WhatsApp code to the *new* number alone is never sufficient.

## 11. Strict device binding

Client half: `web/src/lib/device.ts` + `web/src/lib/deviceTrust.ts`.

| Element | Mechanism |
|---|---|
| Device identity | Stable UUID minted per installation (`amberpay.device_id` in localStorage) |
| Device fingerprint | SHA-256 over a descriptor: UA, platform, languages, timezone, screen×DPR, color depth, CPU count, device memory, touch points, app version — descriptor never leaves the device, only the hash |
| Binding record | `(user_id, device_id, fingerprint, bound_at)` written to `amberpay.device_binding` **only after** successful OTP verification |
| Verification | On every app load and every auth-state change: live fingerprint + device id compared to the binding |
| Violation | Session signed out locally (`signOut({ scope: 'local' })`), binding cleared, Security centre shows the mismatch — never silently ignored |
| Untrusted devices | Sessions that predate binding or whose binding write failed are treated as unbound → forced re-verification (strict mode leaves no "grace" session) |

**Honest limitation:** a browser fingerprint is evidence, not cryptography.
The client feed is necessary but not sufficient; the Core API (Java) must
enforce the server half when it lands:

1. **Server-side session registry** — `app.devices` + `app.sessions` store
   the binding at verification time; every authenticated request carries the
   device id header and is checked server-side, not just client-side.
2. **Attestation upgrade path** — WebAuthn/getIsUVPAA on capable browsers,
   Play Integrity on Android — makes "device" cryptographically checkable.
3. **Network signals** — IP/ASN drift between bind time and use is a
   violation signal evaluated by the risk engine (docs/fraud-risk.md).
4. **Audit** — `DEVICE_BOUND`, `DEVICE_BINDING_VIOLATION` (high severity,
   triggers session-family revocation review), `DEVICE_UNBOUND`.
5. **Recovery** — a violation is resolved by a fresh WhatsApp OTP + the §7
   re-authentication path, never by client-side override.

## 12. Open items

- Passkeys/WebAuthn rollout order (feature-flag gated).
- Login notification channel default (push/SMS/email) — customer preference
  UX in `docs/ux-flows.md`.
- OTP delivery provider selection (SMS gateway) —
  `REGULATORY_REVIEW_REQUIRED` for any telecom-related requirements.