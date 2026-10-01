# AMBER PAY — Production-Readiness Gate

Status key: ✅ **implemented + verified** · 🔶 **designed, not yet built** ·
⏳ **built but gap remains** · ⛔ **not started** · 🛑 **go-live blocker**.

**Nothing in this document is a claim that AMBER PAY is licensed, regulated,
compliant, or production-ready.** Those claims can only be made after
verification by the appropriate humans and authorities.

## 1. The §75 gate

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Authentication complete | 🔶 | design: docs/authentication.md; Java core to build |
| 2 | MFA complete | 🔶 | TOTP + OTP fallback; passkeys behind flag |
| 3 | Authorization complete | 🔶 | design: docs/authorization.md; RBAC to build |
| 4 | Ledger complete | ✅ | engine, holds, idempotency, derived balances, reversals + refunds |
| 5 | Double-entry accounting verified | ✅ | Σdebits=Σcredits per journal; property + integration tests |
| 6 | Idempotency complete | ✅ | hard backstop + request-hash replay check + key expiry/reclaim/prune (migration 0004) |
| 7 | Transaction state machine complete | 🔶 | design: docs/transaction-state-machine.md; Java states to build |
| 8 | Fraud controls complete | 🔶 | design: docs/fraud-risk.md |
| 9 | KYC architecture complete | 🔶 | design: docs/kyc-aml.md |
| 10 | AML/compliance review complete | 🛑 | REGULATORY_REVIEW_REQUIRED (A1–A11) |
| 11 | Provider integrations tested | 🔶 | sandbox adapters planned; Orange Money first |
| 12 | Webhooks secured | 🔶 | design: docs/providers-webhooks.md |
| 13 | Reconciliation complete | ✅ | matcher + scheduler + sinks implemented; rail sources stubbed |
| 14 | Audit logging complete | 🔶 | design: docs/security-controls.md §4; table to build |
| 15 | Encryption complete | 🔶 | TLS/at-rest designed; field-level + key rotation to build |
| 16 | Secrets management complete | 🔶 | env-injected; Secrets Manager at deploy; gitleaks live |
| 17 | Rate limiting complete | 🔶 | design in docs/security-controls.md §1 row 19 |
| 18 | Monitoring complete | 🔶 | CloudWatch design; metrics to build |
| 19 | Alerting complete | 🔶 | PagerDuty on recon/audit drift to wire |
| 20 | Backups tested | 🔶 | design: docs/disaster-recovery.md; drills to schedule |
| 21 | Disaster recovery tested | 🔶 | monthly restore drill to start |
| 22 | Penetration testing complete | 🛑 | required before real-money launch (§62) |
| 23 | Dependency scanning enabled | ✅ | cargo audit + nightly; Java scanning to add |
| 24 | CI/CD security complete | 🔶 | fmt/clippy/tests/coverage/audit/gitleaks live; SAST/container scan to add |
| 25 | Incident response documented | 🔶 | runbook outline in docs/disaster-recovery.md §4 |
| 26 | Support workflows documented | 🔶 | see docs/ux-flows.md support section |
| 27 | Privacy requirements reviewed | 🛑 | REGULATORY_REVIEW_REQUIRED (A11 + data minimization) |
| 28 | Applicable regulatory requirements reviewed | 🛑 | REGULATORY_REVIEW_REQUIRED (A1–A10) |
| 29 | Financial controls reviewed | 🔶 | internal review pending; independent assessment at go-live |
| 30 | Load testing complete | 🔶 | not started |
| 31 | Concurrency testing complete | ✅ | ledger concurrency tests implemented |
| 32 | Accessibility testing complete | 🔶 | WCAG 2.2 AA target; audits to run |
| 33 | Independent security assessment complete | 🛑 | required before real-money launch |

## 2. Known implementation gaps (updated)

1. ~~Idempotency request-hash mismatch not enforced on replay~~ **closed**
   (`IdempotencyMismatch`, migration 0004 `journals.request_hash`, tested).
2. ~~Idempotency-key expiry not enforced/pruned~~ **closed** (expiry TTL,
   expired-row reclaim, `prune_expired_idempotency_keys`, tested).
3. ~~Reversal/refund engine paths not exposed~~ **closed**
   (`reverse_journal`, `refund_payment` + `ledger/tests/reversals.rs`).
4. **`ledger_events` outbox has no consumer yet** — the Java core outbox
   consumer is part of the Java build.
5. **No gRPC server** (tonic) exposing the ledger interface
   (docs/api.md §12) — ledger is exercised via the Rust API in tests.
6. **Rail statement sources are stubs** (`RailStatementSource::Stub`) —
   Orange Money adapter to land with the rail integration.
7. **App-layer audit log, webhook event store, devices, and risk/case
   tables** are designed but not built (Java core).
8. **Sandbox provider** (simulated rails/webhooks) designed, not built.
9. **Web client**: `web/` scaffolded (React + TypeScript + Vite) with the
   route map, auth context, API client, idempotency hook, and core screens;
   remaining journeys are honest placeholders per §73.

## 3. Assumptions (open items to confirm)

- Single deployable per side (modular monolith Java + ledger Rust) is right
  for current scale; extraction points are documented (architecture §2).
- SLE + USD with 2 minor units; new currencies require a review pass.
- Fee model: 0.5% to payer, agent commission absorbed from fee revenue
  (F3/F8, architecture §9) — confirmed.
- Rails without reliable webhooks: reconciliation is source of truth (F5).
- SMS OTP is a fallback factor, never the sole factor (F6).
- Cards deferred to a PSP selected via structured research (F1).
- Event sourcing deferred; journals/entries are the audit substrate (F7).

## 4. REGULATORY_REVIEW_REQUIRED — canonical list

Maintained in `docs/kyc-aml.md` §3 (A1–A11) plus the jurisdiction register in
`docs/regulatory-sierra-leone.md` §2 (R1–R8), repeated here for the gate:

- A1 Custodial wallet licensing & segregated escrow (Bank of Sierra Leone)
- A2 KYC/identity provider selection
- A3 Transaction/tiered limits
- A4 Sanctions/watchlist screening obligations
- A5 SAR/STR reporting thresholds & deadlines
- A6 Record-keeping & retention minimums
- A7 AML program requirements
- A8 Agent network / cash-in-out regulatory treatment
- A9 Card/PSP acquiring path
- A10 Mobile-money rail agreements (Orange Money, Afrimoney)
- A11 Privacy/data-protection obligations for SL residents
- R1 Licensing/partnering path & entity structure (see regulatory doc §1)
- R2 KYC tiers, permissible ID documents, NIN linkage mechanics
- R3 AML/CFT obligations, sanctions lists, STR process
- R4 Data protection: retention, cross-border transfers, lawful basis
- R5 Consumer protection: complaints, remedies, disclosures
- R6 Cross-border remittance limits & licensing
- R7 Whether/when an e-levy-style transaction tax applies in a launch market
- R8 National Payment Switch integration requirements

**Gate rule:** any item marked 🛑 or listed above must be cleared by the
appropriate humans and authorities before AMBER PAY handles real customer
money — none of it can be cleared by code or by this document.