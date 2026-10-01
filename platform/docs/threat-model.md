# AMBER PAY — Threat Model

Scope: the full AMBER PAY platform as designed in `docs/architecture.md` (Rust
ledger service + Java core modular monolith, PostgreSQL, Redis, AWS ECS/RDS).
Currency: SLE primary, USD secondary. Jurisdiction: Sierra Leone.
**Every threat with a regulatory or licensing component is marked
`REGULATORY_REVIEW_REQUIRED` and is not a go-live blocker for code, but is a
hard go-live gate.**

Format per threat: `Threat → Impact → Likelihood → Prevention → Detection →
Response → Owner`.

Likelihood scale: **L**ow / **M**edium / **H**igh. Impact scale:
**L**ow / **M**edium / **H**igh / **C**ritical.

---

## 1. Account takeover (ATO)

- **Threat**: Attacker obtains a customer's credentials (phishing, credential
  stuffing, SIM swap, malware) and moves their funds.
- **Impact**: C (customer funds lost; platform liability).
- **Likelihood**: H.
- **Prevention**:
  - Argon2id password + transaction PIN hashing; separate transaction
    authorization (PIN/OTP) on every money movement.
  - MFA (TOTP authenticator) with OTP fallback; MFA required for
    high-value operations.
  - Device registry (§ `docs/authentication.md`): new-device and
    new-location signals trigger step-up authentication.
  - Rate limiting on login, OTP, and PIN attempts with progressive delays.
  - No account-existence disclosure on login/password-reset.
- **Detection**: login notifications, new-device/new-location alerts,
  credential-stuffing velocity signals, unusual transaction behavior,
  simultaneous-session anomalies, `LOGIN_FAILED` audit events.
- **Response**: freeze account (§ `docs/fraud-risk.md`), revoke all sessions
  and devices, force password + PIN reset, refund review, fraud case.
- **Owner**: Core API Service (auth), Fraud team.

## 2. Credential stuffing / password spraying

- **Threat**: Bulk replay of breached credentials against login.
- **Impact**: H (feeds ATO).
- **Likelihood**: H.
- **Prevention**: rate limits per IP/account/device, progressive throttling,
  breached-password screening on registration and password change, MFA
  enrollment encouraged/required.
- **Detection**: `LOGIN_FAILED` velocity, bursts from single IP ranges,
  CAPTCHA/challenge insertion.
- **Response**: temporary IP/account lock, security notification, fraud case.
- **Owner**: Core API Service, Fraud team.

## 3. Phishing / social engineering

- **Threat**: User tricked into revealing password, PIN, OTP, or recovery
  codes.
- **Impact**: H–C.
- **Likelihood**: M–H.
- **Prevention**: never ask for OTP/PIN/recovery codes in support channels
  (§ `docs/ux-flows.md` support section), user-facing security warnings,
  sender authentication (SPF/DKIM/DMARC) on notifications, clear
  notification templates that never embed secrets.
- **Detection**: support-agent training enforcement, login notifications,
  anomalous logins.
- **Response**: account freeze, session revocation, fraud case, customer
  education.
- **Owner**: Support, Core API Service.

## 4. SIM-swap attack

- **Threat**: Attacker convinces the carrier to port the victim's number,
  then intercepts SMS OTPs.
- **Impact**: C (OTP-dependent controls bypassed).
- **Likelihood**: M (telecom-dependent; Sierra Leone
  `REGULATORY_REVIEW_REQUIRED` on carrier KYC/porting practices).
- **Prevention**: OTP is a fallback, never the only factor; transaction PIN
  required independently of OTP; new-beneficiary cooling-off; withdrawal
  destination changes require re-authentication; device-binding signals.
- **Detection**: sudden phone/line changes, OTP + fresh-device combinations,
  beneficiary changes.
- **Response**: account freeze, step-up to authenticator, fraud case.
- **Owner**: Core API Service, Fraud team.

## 5. Malware / device compromise

- **Threat**: Malware on the customer's device steals session tokens,
  PINs, or performs transactions in-session.
- **Impact**: H.
- **Likelihood**: M.
- **Prevention**: short-lived access tokens, refresh-token rotation and
  revocation, device registry with revoke-all, no secrets in storage,
  HttpOnly/Secure cookies where used, session reuse detection.
- **Detection**: new-device flags, anomalous session behavior.
- **Response**: revoke device + all sessions, freeze, fraud case.
- **Owner**: Core API Service.

## 6. Insider abuse (admin/agent)

- **Threat**: Employee, support agent, or agent-network operator abuses
  access to move or view customer funds.
- **Impact**: C.
- **Likelihood**: M.
- **Prevention**: RBAC least privilege (§ `docs/authorization.md`), no admin
  can create/delete/modify customer money directly, maker–checker for
  financial adjustments (§30), scoped roles, break-glass with
  justification + alerting + review, DB role separation (`amber_app` has no
  write access to `ledger`), full audit logging.
- **Detection**: audit log review, reconciliation drift, maker–checker
  violations, break-glass usage review.
- **Response**: revoke access, escalate, fraud case, postmortem.
- **Owner**: Platform/Compliance.

## 7. Fraudulent merchants / business abuse

- **Threat**: Merchant or business account defrauds customers (fake goods) or
  the platform (chargebacks, refund abuse).
- **Impact**: M–H.
- **Likelihood**: M.
- **Prevention**: merchant onboarding with KYC/verification
  (`REGULATORY_REVIEW_REQUIRED`), hold/escrow patterns, refund limits,
  velocity monitoring, business approval workflows.
- **Detection**: refund-rate anomalies, dispute frequency, complaint velocity.
- **Response**: merchant freeze, manual review, dispute workflow.
- **Owner**: Compliance, Fraud team.

## 8. Money laundering / structuring

- **Threat**: Platform used to launder funds or structure transactions below
  reporting thresholds.
- **Impact**: H (licensing risk) `REGULATORY_REVIEW_REQUIRED`.
- **Likelihood**: M.
- **Prevention**: KYC/AML architecture (§ `docs/kyc-aml.md`), transaction
  monitoring, velocity and structuring detection, sanctions screening,
  transaction limits.
- **Detection**: monitoring rules, compliance cases.
- **Response**: compliance case, SAR/reporting workflow
  (`REGULATORY_REVIEW_REQUIRED`), account hold/freeze.
- **Owner**: Compliance.

## 9. Transaction manipulation / race conditions

- **Threat**: Double-spend, concurrent withdrawal overdraw, duplicate
  submission, or out-of-order posting.
- **Impact**: C.
- **Likelihood**: L (engine defenses) — but treat as critical if it occurs.
- **Prevention**: single-writer ledger, `FOR UPDATE` locks in canonical
  order, derived balances under lock, idempotency `(scope, key)` unique
  constraint + request-hash replay check, atomic outbox. See
  `docs/transaction-state-machine.md` and `ledger/src/engine.rs`.
- **Detection**: ledger audit job (`audit_snapshots`), reconciliation,
  mandatory financial test cases (§ `docs/testing-strategy.md` §61).
- **Response**: stop the rail/withdrawals, investigate, reconcile, correct
  via new journals (never mutation).
- **Owner**: Ledger Service, Core API Service.

## 10. Replay attacks (API / webhook)

- **Threat**: A captured request or webhook is replayed to move money twice
  or to fake a state transition.
- **Impact**: H.
- **Likelihood**: M.
- **Prevention**: idempotency keys on every money-moving endpoint (duplicate
  payload with the same key is rejected; same key + different payload is
  rejected), webhook signature verification + timestamp window + event-id
  uniqueness, nonce/expiry on OTP.
- **Detection**: duplicate event ids, idempotency-mismatch audit events.
- **Response**: reject, alert, investigate.
- **Owner**: Core API Service, Ledger Service.

## 11. API abuse (scraping, enumeration, DoS)

- **Threat**: Automated abuse of public APIs — enumeration of users/IDs,
  brute force, resource exhaustion.
- **Impact**: M.
- **Likelihood**: H.
- **Prevention**: rate limiting (token bucket, Redis), request size limits,
  timeouts, pagination caps, unguessable UUIDs, no sequential IDs,
  strict CORS, secure error responses (no stack traces / internals).
- **Detection**: rate-limit hits, anomaly metrics.
- **Response**: throttle, block, alert.
- **Owner**: Core API Service / gateway.

## 12. Database compromise

- **Threat**: Attacker reads or modifies database contents (SQLi, stolen
  credentials, exposed replica).
- **Impact**: C (financial + PII exposure).
- **Likelihood**: M.
- **Prevention**: parameterized queries only, least-privilege DB roles,
  separate schemas/roles, encryption at rest, secrets manager, no secrets in
  code, network isolation (RDS in private subnets), restricted IAM.
- **Detection**: audit logs, DB activity monitoring, credential rotation,
  reconciliation + snapshot audit (catches silent ledger mutation).
- **Response**: isolate, rotate credentials, restore from tested backups
  (§ `docs/disaster-recovery.md`), postmortem.
- **Owner**: Platform/DevSecOps.

## 13. Supply-chain attacks (dependencies)

- **Threat**: Compromised or malicious dependency in Rust/Java code.
- **Impact**: H.
- **Likelihood**: M.
- **Prevention**: `cargo audit` (High/Critical fail) + nightly, Java
  dependency scanning, locked dependency files, artifact integrity checks,
  container scanning.
- **Detection**: CI + nightly pipelines, vulnerability feeds.
- **Response**: update/patch, investigate exposure window.
- **Owner**: DevSecOps.

## 14. Cloud compromise (AWS account)

- **Threat**: Compromised AWS credentials or misconfigured infrastructure.
- **Impact**: C.
- **Likelihood**: M.
- **Prevention**: least-privilege IAM, no long-lived keys in code (Secrets
  Manager / SSM), private subnets, security groups, IaC with scanning,
  no public exposure of the ledger service.
- **Detection**: CloudTrail, config drift, alerting.
- **Response**: rotate keys, isolate, DR plan activation.
- **Owner**: Platform/DevSecOps.

## 15. Webhook forgery

- **Threat**: Attacker forges a provider callback (e.g. "payment succeeded")
  or a merchant-facing event.
- **Impact**: C (fake success → funds marked available without money).
- **Likelihood**: M.
- **Prevention**: inbound webhooks verified by provider secret
  (HMAC/signature per provider), timestamp freshness, event-id idempotency,
  provider transaction-id mapping, never trust unsigned payloads; outbound
  webhooks HMAC-signed with per-merchant secrets. See
  `docs/providers-webhooks.md`.
- **Detection**: signature failures, duplicate event ids, reconciliation
  mismatch.
- **Response**: reject, alert, investigate, reconcile.
- **Owner**: Core API Service, Ledger Service.

## 16. Provider compromise / rail outage

- **Threat**: A rail (Orange Money, Afrimoney, bank, PSP) is compromised,
  returns false data, or is down.
- **Impact**: H.
- **Likelihood**: M.
- **Prevention**: provider abstraction with timeouts, status polling,
  reconciliation as source of truth (callbacks are acceleration signals
  only), UNKNOWN/PENDING handling (never auto-fail on timeout).
- **Detection**: health checks, reconciliation drift, delayed-callback
  metrics, provider status.
- **Response**: disable the rail (feature flag), reconcile, safe fund release
  (§ `docs/transaction-state-machine.md`), incident response.
- **Owner**: Core API Service, Ops.

## 17. DDoS

- **Threat**: Volumetric attack on public endpoints.
- **Impact**: M (availability; not fund loss if controls hold).
- **Likelihood**: M.
- **Prevention**: edge protection (ALB/WAF/CDN), rate limiting, no public
  exposure of internal services.
- **Detection**: traffic/error metrics, alerting.
- **Response**: edge mitigation, scale, incident response.
- **Owner**: Platform.

## 18. Social engineering of support

- **Threat**: Attacker social-engineers a support agent into changing phone,
  email, or resetting credentials.
- **Impact**: C.
- **Likelihood**: M.
- **Prevention**: support agents never request passwords/OTPs/PINs/recovery
  codes; identity verification for account changes; re-authentication for
  phone/email/MFA changes; audit of support actions; maker–checker where
  appropriate.
- **Detection**: audit events on sensitive changes, unusual support actions.
- **Response**: revoke, freeze, fraud case, retraining.
- **Owner**: Support, Compliance.

---

## 19. Residual-risk register (things we explicitly accept or defer)

| # | Residual risk | Why accepted | Review trigger |
|---|---|---|---|
| R1 | SMS delivery reliability in Sierra Leone | OTP is a fallback factor; PIN remains primary control (F6 in `docs/architecture.md`) | If SMS becomes the sole factor anywhere |
| R2 | Rails without reliable webhooks | Reconciliation is the source of truth; polling + UNKNOWN handling (F5) | When a rail's callback quality is proven |
| R3 | Card/3DS deferred to a PSP | Platform is not an acquirer (F1) | When PSP is selected and integrated |
| R4 | Licensing/custody of customer balances | Design supports segregated escrow; go-live requires Bank of Sierra Leone review (F2) `REGULATORY_REVIEW_REQUIRED` | At go-live gate |

---

## 20. Threat model ownership & refresh

- Every threat above has an **owner**; owners confirm prevention/detection
  controls in design review.
- The model is re-reviewed:
  - before any new money-moving feature,
  - when a new provider/rail is integrated,
  - after any security incident,
  - at least annually, and
  - as part of the production-readiness gate (§ `docs/production-readiness.md`).