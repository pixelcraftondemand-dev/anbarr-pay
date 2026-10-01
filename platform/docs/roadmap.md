# AMBER PAY — Build Roadmap

The next 10 build goals, ordered by dependency and risk. Status key follows
`production-readiness.md`: ✅ implemented · ⏳ built, gap remains · 🔶
designed/client-ready · ⛔ not started. Regulatory 🛑 gates are **not**
roadmap items — see the gate rule in `production-readiness.md` §4.

Ground truth: `production-readiness.md` §2 (implementation gaps),
`docs/api.md` (contract), `docs/authentication.md`, `docs/vault.md`.

| # | Goal | Status | Why now |
|---|---|---|---|
| 1 | **Authentication & sessions for the Core API** — phone-first sign-in with OTP, sessions (access + rotating refresh), authenticated caller identity replacing the `x-amber-caller` dev header | ⏳ in progress | Unblocks everything public; the header is a dev-only configuration (`README.md` warns not to deploy) |
| 2 | **Transaction PIN + step-up** — `POST /v1/auth/pin/verify` (2-min `pin_token`), enforced on money-movement endpoints per `docs/api.md` §1 and `docs/vault.md` §2 | ⛔ | Vault contract already requires `Pin-Token`; the web client (`web/src/api/pin.ts`) already expects it |
| 3 | **P2P transfers end to end** — `POST /v1/transfers` posting the ledger payment via gRPC with idempotency; wire `SendMoney` + `useTransferSubmit` (currently an honest stub) | ⛔ | Flagship journey; first non-hold money path through the Core API |
| 4 | **Transaction history API** — `GET /v1/transactions` (+ detail) backed by ledger journals, paginated | ⛔ | `Activity` and `TransactionDetail` screens exist with nothing real to read; small and makes the app real |
| 5 | **Topup & withdrawal (cash-in/out)** — endpoints driving `rail_bridge` journals; withdraw journey (currently a placeholder) | ⛔ | Exercises the state machine's UNKNOWN → pending path; rail sources stay stubbed at this stage |
| 6 | **KYC tiers + limits enforcement** — document submission (NIN first-class, `docs/kyc-aml.md`), tier storage, server-side gates: vault goal count/caps, transaction limits (A3) | ⛔ | `/kyc` is a placeholder; vault already needs tier gates per `docs/vault.md` §2 |
| 7 | **Beneficiaries backend** — `GET/POST /v1/beneficiaries` persistence + validation | 🔶 | Client + `Beneficiaries` screen already exist (`web/src/api/types.ts`); only the backend is missing |
| 8 | **`ledger_events` outbox consumer** — exactly-once consumer for notifications/webhook dispatch | ⛔ | Gap #4 in `production-readiness.md` §2; no event is currently delivered anywhere |
| 9 | **Orange Money rail adapter + sandbox provider** — real `RailStatementSource` behind the existing trait, plus the designed sandbox rails/webhooks | 🔶 | Stub is in place (`ledger/src/reconcile.rs`); reconciliation goes live end-to-end before rail agreements (A10) clear |
| 10 | **Production hardening** — rate limiting (`security-controls.md` §1), app-layer audit log, metrics + PagerDuty on recon/audit drift, secrets via deployment platform; refresh stale docs | ⛔ | Includes doc drift: `production-readiness.md` §2 still claims "no gRPC server" though tonic exists |

Sequencing note: 1 → 2 → 3 is the critical path (identity → PIN → first real
money movement). 4, 5, 7 can follow in any order once identity exists; 6 gates
limits on 3/5; 8–10 are launch-readiness work that can proceed in parallel.
