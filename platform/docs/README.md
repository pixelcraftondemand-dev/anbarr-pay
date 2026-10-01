# AMBER PAY — Design Deliverables

Index of the AmberPay checklist deliverables (§1/§78). Status: ✅ written ·
🔶 partial · ⛔ not started.

| Deliverable | Doc | Status |
|---|---|---|
| Engineering standards (code, tests, commits, AI review) | `engineering-standards.md` | ✅ |
| AI review prompt (staging PRs) | `ai-review-prompt.md` | ✅ |
| Architecture document | `architecture.md` | ✅ (updated: React frontend) |
| Database schema | `schema.md` | ✅ |
| API specification | `api.md` | ✅ |
| Vault (goal savings) design | `vault.md` | ✅ (contract final; Core API pending) |
| Ledger design | `architecture.md` §5 + `schema.md` + `ledger/src/` | ✅ (implemented, incl. reversals + refunds) |
| Transaction state machine | `transaction-state-machine.md` | ✅ |
| Authentication design | `authentication.md` | ✅ |
| Authorization / RBAC matrix | `authorization.md` | ✅ |
| Security controls matrix | `security-controls.md` | ✅ |
| Threat model | `threat-model.md` | ✅ |
| Fraud/risk architecture | `fraud-risk.md` | ✅ |
| KYC/AML architecture | `kyc-aml.md` | ✅ (NIN as first-class field for SL) |
| Regulatory context (Sierra Leone) | `regulatory-sierra-leone.md` | ✅ (context map, not legal advice) |
| Design system | `design-system.md` + `web/src/styles.css` | ✅ (tokens/typography/motion/pills implemented) |
| Provider abstraction | `providers-webhooks.md` §1 | ✅ |
| Webhook architecture | `providers-webhooks.md` §2 | ✅ |
| Reconciliation design | `architecture.md` §6.2 + `ledger/src/reconcile.rs` | ✅ (implemented) |
| Disaster recovery & backups | `disaster-recovery.md` | ✅ |
| Testing strategy | `testing-strategy.md` | ✅ |
| Screen-by-screen UX map | `ux-flows.md` §1 | ✅ (implemented as `web/` React SPA scaffold) |
| Click-by-click interaction map | `ux-flows.md` §3 | ✅ (core journeys; remainder tracked in §5) |
| Production-readiness checklist | `production-readiness.md` | ✅ |
| Assumptions | `production-readiness.md` §3 | ✅ |
| REGULATORY_REVIEW_REQUIRED list | `kyc-aml.md` §3 + `production-readiness.md` §4 + `regulatory-sierra-leone.md` §2 | ✅ |

Review cadence: every doc is re-reviewed when its owning feature builds, at
the production-readiness gate, and after any security incident. Threat model
refresh triggers: `docs/threat-model.md` §20.