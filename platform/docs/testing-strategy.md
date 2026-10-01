# AMBER PAY — Testing Strategy

Three layers, mirroring the architecture: **ledger** (Rust, correctness
critical), **core API** (Java, domain/integration), **client** (React, UX/E2E).

## 1. Test pyramid & ownership

| Layer | What | Owner | Runs |
|---|---|---|---|
| Unit — pure logic | fee math, journal validation, funds rule, reconciliation matcher | Rust (implemented: `ledger/src` + `ledger/tests/properties.rs`) | every CI run |
| Unit — business logic | permission logic, risk decisions, fee incidence, state transitions | Java core (with the Java build) | every CI run |
| Integration — ledger | real Postgres: posting, holds, idempotency, concurrency, audit | Rust (implemented: `ledger/tests/*`) | CI with service container |
| Integration — API | DB + ledger via gRPC: domain state machines, provider adapters, webhooks | Java core | CI |
| E2E — client + API | registration → login → KYC → deposit → transfer → withdrawal → refund → dispute | React (Playwright) + sandbox | CI (staged) + nightly |
| Security tests | auth, authorization, IDOR/BOLA, injection, XSS, CSRF, SSRF, rate limit, privilege escalation | cross-layer | CI + pre-go-live |
| Reliability tests | timeout, provider outage, DB failure, queue failure, duplicate webhook/request, concurrent requests | cross-layer | CI (fault-injection stage) |

Coverage bar (PixelCraft standard, already enforced): core ledger modules
(`engine.rs`, `balances.rs`, `money.rs`, `reconcile.rs`) ≥ 85%; whole crate
≥ 80% (`python3 .github/scripts/check_core_coverage.py lcov.info`). The Java
core and React layers adopt the same bar as they land.

## 2. What exists today (ledger crate)

- **Property tests** (`ledger/tests/properties.rs`): `round_fee` (bounded
  error, monotone, half-to-even, exact multiples, ≤ principal), `validate_spec`
  accepts exactly the balanced journals, `check_funds` accepts exactly the
  non-negative postings (bridges exempt), fee journals always balance,
  `payment_legs` always balance, funds rule exact at the boundary.
- **Integration tests** (`ledger/tests/engine.rs`): transfer + fee posting,
  atomic insufficient-funds rejection, unbalanced rejection, duplicate-key
  replay, hold/capture/release/expire cycles, concurrent transfers never
  overdraw, snapshot audit clean, idempotency races, frozen accounts, unknown
  accounts, malformed idempotency cache.
- **Coverage + CI**: fmt, clippy `-D warnings`, unit/property, integration
  against disposable Postgres, release build, coverage gates, `cargo audit`,
  `gitleaks`. Daily security-nightly pipeline.

## 3. Mandatory financial test cases (§61) — status

| Case | Expectation | Status |
|---|---|---|
| Double payment (identical request twice) | ONE financial transaction | ✅ implemented (`duplicate_key_replays_without_double_posting`) |
| Double withdrawal (identical twice) | ONE withdrawal | ⏳ Java layer (ledger idempotency covers the journal) |
| Concurrent withdrawals | Cannot exceed available funds | ✅ implemented (`concurrent_transfers_never_overdraw`) |
| Provider timeout | `UNKNOWN`/`PENDING`, never auto-failed | ⏳ Java layer (state machine + sandbox) |
| Duplicate webhook | No duplicate ledger entry | ⏳ Java layer (event-id idempotency) |
| Reversal | New reversal journal event | ✅ implemented (`reverse_journal` + `ledger/tests/reversals.rs`) |
| Refund (partial/full) | New refund journal, principal only, ≤ payee principal | ✅ implemented (`refund_payment` + tests) |
| Failed payout | Funds safely released/reconciled | ⏳ Java layer + hold-release path |
| Database failure mid-operation | No partial financial commit | ✅ single ACID transaction per journal (rollback tested via insufficient-funds) |

## 4. Security tests (§60, §62)

- AuthN: brute force, credential stuffing, session reuse, token rotation,
  OTP replay, PIN lockout.
- AuthZ: IDOR/BOLA cross-user and cross-org, privilege escalation,
  maker–checker separation (approver ≠ creator), role-gated admin endpoints.
- Web: injection (SQLi via parameterization), XSS, CSRF, SSRF (webhook URL
  validation), clickjacking, open redirects, upload abuse (magic bytes,
  size, traversal, SVG/HTML payloads).
- API: rate limits (429 + Retry-After), request size, idempotency mismatch
  (same key, different payload → 409), pagination abuse.
- **Penetration testing**: authorized independent assessment (OWASP Top 10 +
  payment-specific: race conditions, replay, webhook forgery, business-logic
  abuse) before real-money launch; findings tracked to closure.
- Replay attacks and webhook forgery are already modeled in
  `docs/threat-model.md` threats 10 and 15.

## 5. Reliability tests (§60)

Timeout / provider outage / database failure / queue failure / duplicate
webhook / duplicate request / concurrent requests — run against the sandbox
provider (docs/providers-webhooks.md §3) in a fault-injection CI stage
(tokio-turbine-style chaos for Rust; Testcontainers + HikariCP failure
injection for Java; Playwright network-throttle for the React client).

## 6. E2E journeys (React, Playwright) (§60, §50)

Registration → email/phone verification → KYC submit/review → deposit
(sandbox rail) → transfer → withdrawal → refund → dispute → merchant checkout
→ QR payment → payment link → account freeze → security settings → API key
creation → business approval workflow. Sandbox scenarios: successful/failed/
pending payment, timeout, refund, webhook retry, duplicate webhook, provider
outage. Every journey runs against the SANDBOX environment — never production.

## 7. Regression gating

- Every PR: fmt, clippy, unit + property, integration, coverage, cargo audit,
  gitleaks (existing pipeline).
- Java core + React layers add: Java unit/integration, Playwright E2E,
  dependency + container scanning, secret scanning for their artifacts.
- Financial-feature PRs additionally require: idempotency test, failure-path
  test, audit-event test, reconciliation-implication review, and a second
  review — per the checklist §78 "after every major feature" gate.