# AMBER PAY — Security Controls Matrix & Audit Design

## 1. Security controls matrix

Mechanism column cites where the control lives (React client, Core API,
Ledger Service, gateway, DB, infra). Status: **implemented** (in `ledger`
crate today) · **designed** (specified here or in sibling docs, Java core to
build) · **planned** (deferred/flagged).

| # | Control area | Requirement | Mechanism | Where | Status |
|---|---|---|---|---|---|
| 1 | Transport | TLS everywhere | TLS 1.2+ at ALB; HSTS | gateway | designed |
| 2 | Encryption at rest | DB + backups encrypted | RDS encryption, KMS keys, encrypted backups | infra | designed |
| 3 | Field-level encryption | ESPECIALLY sensitive fields (recovery codes, TOTP secrets, webhook secrets, OTP hashes already) | app-layer AES-GCM with KMS-managed keys; key rotation | Core API | designed |
| 4 | Secrets management | No secrets in code/repo/logs/errors | AWS Secrets Manager / SSM; env-injected at deploy; gitleaks CI + nightly | infra/CI | designed (scans live) |
| 5 | Password storage | Argon2id, never plaintext | Argon2id with per-user salt | Core API | designed |
| 6 | PIN storage | Argon2id, attempt limits | Argon2id + lockout (docs/authentication.md §5) | Core API | designed |
| 7 | OTP storage | Never plaintext | hashed, single-use, TTL 5 min | Core API | designed |
| 8 | Session tokens | Short-lived, rotating, revocable | 15-min access + rotating refresh, hashed at rest, reuse detection | Core API | designed |
| 9 | CSRF | No state-changing CSRF for cookie auth | SameSite=Strict cookies + CSRF token for cookie flows; bearer-in-memory for API | Core API/React | designed |
| 10 | XSS | Output encoding everywhere | React escapes by default; CSP; no `dangerouslySetInnerHTML` without review; sanitize rich text | React | designed |
| 11 | CSP | Restrict script sources | CSP header (default-src 'self'; no inline scripts unless hashed) | gateway/React | designed |
| 12 | SQL injection | Parameterized queries only | sqlx bound params (ledger); JDBC prepared statements (Java); no string-built SQL except whitelisted identifiers | Ledger/Core API | implemented (ledger) |
| 13 | SSRF | No internal-network fetches from user input | allow-list of rail endpoints; no user-controlled URLs fetched server-side; webhook URLs validated (https only, SSRF-guarded) | Core API | designed |
| 14 | Clickjacking | No UI redress | `X-Frame-Options: DENY` / CSP frame-ancestors | gateway | designed |
| 15 | Open redirects | No redirect-to-arbitrary-URL | allow-list return URLs (checkout return_url validated to merchant's registered domains) | Core API | designed |
| 16 | Request smuggling | Normalize content-length/transfer-encoding | ALB/WAF + HTTP/2 only | gateway | designed |
| 17 | Malicious uploads | KYC/evidence files | type + magic-bytes validation, size limit, malware scan, non-executable storage (S3, no HTML/SVG), short-lived signed URLs, path-traversal safe keys | Core API | designed |
| 18 | CORS | No wildcard for authenticated APIs | explicit origin allow-list, credentials only for cookie auth origins | gateway/Core API | designed |
| 19 | Rate limiting | Login/OTP/reset/transactions/withdrawals/beneficiaries/API/uploads | token bucket per key/IP/user in Redis; progressive throttling; 429 + Retry-After | gateway/Core API | designed |
| 20 | Input validation | Schema validation on every endpoint | JSON-schema/bean validation; amounts as integers; enum whitelists | Core API | designed |
| 21 | Idempotency | Money-moving POSTs | (scope, key) unique; request-hash replay check (gap: see production-readiness) | Ledger (impl.)/Core API | implemented (ledger) |
| 22 | Authorization | RBAC + ownership checks server-side | docs/authorization.md; BOLA tests | Core API | designed |
| 23 | Webhook inbound | Verify provider signatures, timestamp, replay | per-provider HMAC/signature; event-id idempotency; async processing | Core API | designed (docs/providers-webhooks.md) |
| 24 | Webhook outbound | HMAC-signed, retried, dead-lettered | SHA-256 HMAC with per-merchant secret; exponential backoff; delivery log | Core API | designed |
| 25 | Audit logging | Tamper-resistant, no unauthorized deletion | append-only `audit_log`; hash-chain option; restricted role | Core API (new) | designed (§4) |
| 26 | Ledger integrity | No hidden mutation of entries | entries immutable; snapshot audit job nightly; reconciliation | Ledger Service | implemented |
| 27 | Failure isolation | No partial commits | single ACID transaction per journal; rollback on error | Ledger Service | implemented |
| 28 | Error handling | No stack traces/db details/secrets to customers | problem+json with safe message + request_id; detailed internal logs | Core API/Ledger | designed (api.md) |
| 29 | Dependency security | CVE + supply-chain monitoring | cargo audit (High/Critical fail), nightly; Java dependency scanning; locked files | CI | implemented (Rust) |
| 30 | Container/artifact security | No secrets in images; integrity | image scanning (trivy), SBOM, signed artifacts | CI/infra | planned |
| 31 | Service-to-service | mTLS/shared secret for gRPC; network segmentation | internal-only ledger service; scoped credentials | infra | designed |
| 32 | Break-glass | Emergency admin access | time-limited, justification-required, alerting, audited, reviewed | Core API | designed |
| 33 | Feature flags | Risky features gated | flag service; new rails/providers/fraud models behind flags | Core API | designed |
| 34 | Sandbox vs production | Never cross funds | separate environments + credentials; sandbox data clearly labeled SANDBOX_PAYMENT | infra | designed |

## 2. Web security detail (React client + API)

- **CSP** baseline: `default-src 'self'; script-src 'self'; style-src 'self'
  'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors
  'none'; base-uri 'self'; form-action 'self'`. Tightened per environment.
- **Security headers**: `Strict-Transport-Security`,
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`,
  `Permissions-Policy`.
- **Cookies** (when used): `HttpOnly`, `Secure`, `SameSite=Strict` for
  session/refresh cookies; JS-accessible storage avoided for tokens
  (`docs/authentication.md` §3).
- **No wildcard CORS.** Authenticated APIs only respond to the registered
  production origin and the current environment's dev origin.

## 3. Error handling contract

- Customer-facing: `{ error: { code, message, request_id } }` (problem+json,
  `docs/api.md`). Never stack traces, SQL text, secrets, internal hostnames,
  or fraud-rule details.
- Internal: structured logs with the same `request_id` (correlation id),
  severity, actor, and resource — full detail server-side only.
- Retry guidance returned where safe (e.g. 429 `Retry-After`, 409 idempotency).

## 4. Audit logging design (§31)

New `app.audit_log` table (lands with the Java core build), append-only by
convention and enforced by DB role (`amber_app` may only INSERT; a dedicated
`amber_audit_reader` role can read; nobody can UPDATE/DELETE except a
break-glass path that itself logs):

| Column | Meaning |
|---|---|
| id (bigserial) | sequence (also enables hash chaining) |
| actor_id, actor_type | user / merchant / system / service / break-glass |
| action | enum: LOGIN, LOGIN_FAILED, LOGOUT, PASSWORD_CHANGED, MFA_ENABLED, MFA_DISABLED, DEVICE_ADDED, DEVICE_REVOKED, BENEFICIARY_CREATED, BENEFICIARY_CHANGED, PAYMENT_CREATED, PAYMENT_COMPLETED, PAYMENT_FAILED, WITHDRAWAL_CREATED, REFUND_CREATED, ACCOUNT_FROZEN, ADMIN_ACTION, KYC_REVIEWED, LIMIT_CHANGED, API_KEY_CREATED, API_KEY_REVOKED, … |
| target_type, target_id | resource |
| result | success / failure / blocked |
| request_id | correlation id (also in logs + provider calls) |
| security_context | ip, device_id, session_id, mfa_method, risk decision |
| created_at | TIMESTAMPTZ |
| prev_hash | SHA-256 of previous row (hash chain → tamper-evident) |

- Every security-sensitive and financial event in the checklist §31 list is
  recorded with actor, action, target, timestamp, result, request id, and
  security context.
- Retention: audit logs kept per policy (regulatory minimums —
  `REGULATORY_REVIEW_REQUIRED`); exported to immutable object storage
  (S3 object-lock) nightly.
- Unauthorized deletion is prevented by role, and tampering is detectable via
  the hash chain + nightly verification job.

## 5. Queues / async processing (§43)

Payment flows may use: `Payment → Queue → Worker → Provider → Webhook →
Reconciliation`. Worker requirements: idempotent, retry-safe, observable,
timeout-aware, protected from duplicate execution (claim/lease on the job),
dead-letter handling. **Retries must never duplicate money movement** — the
ledger idempotency keys are the backstop; workers carry the same key on
retry.