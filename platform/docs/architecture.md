# AMBER PAY — System Architecture Proposal

Version: v0.1 (proposed, awaiting confirmation)
Scope: Sierra Leone only. Currency: SLE (primary), USD (secondary).

---

## 1. Design Principles

1. **Correctness over throughput.** Money moves only through the ledger engine. Every balance mutation happens inside a single database transaction with pessimistic locking.
2. **The ledger is immutable.** Posted entries are never updated or deleted. Corrections are new journals (reversals/adjustments) that reference the original.
3. **One writer for money.** Only the Rust Ledger Service writes to ledger tables. The Java services reach the ledger only through its gRPC API. This is enforced with separate database roles (see §5), not just convention.
4. **Auditability by construction.** Every journal records origin (user, channel, session, payment code); balances are derived from immutable entries, never stored or mutated directly.
5. **Idempotency everywhere.** All state-changing operations carry idempotency keys, enforced at the ledger layer — retries can never double-post.
6. **Amounts are integers.** All monetary values are `BIGINT` in minor units (cents). Floats are banned. SLE and USD both use 2 minor units (0.01 SLE = 1 cent).

---

## 2. Service Topology

```
   ┌───────────────────────────────────────────────────────┐
   │              Web client — React SPA                   │
   │  TypeScript · Vite · static hosting (CDN/S3+CloudFront)│
   │  no financial logic · server-authoritative state      │
   └──────────────────────────┬────────────────────────────┘
                              │ HTTPS /v1 (REST, JSON)
                              ▼
                        ┌──────────────────────────────────────────────┐
                        │              API Gateway / Edge              │
                        │   TLS · rate limiting · authn · routing      │
                        └──────────┬───────────────┬───────────────────┘
                                   │               │
                 public REST /v1   │               │  webhook → merchants
                                   ▼               │
   ┌─────────────────────────────────────────────┐ │
   │          Core API Service (Java)            │ │
   │  Spring Boot — modular monolith             │ │
   │                                             │ │
   │  · identity / auth (password, PIN, OTP)     │ │
   │  · KYC                                      │ │
   │  · merchants, agents, payment codes         │ │
   │  · checkout / transfer / top-up state       │ │
   │    machines                                 │ │
   │  · fee computation                          │ │
   │  · fraud rules (velocity, anomaly)          │ │
   │  · webhook dispatch + retries               │ │
   │  · email/SMS notifications                  │ │
   │  · rail adapters (Orange, Afrimoney, bank)  │ │
   │  · card PSP adapter (3DS)                   │ │
   │  · reconciliation orchestration             │ │
   └───────────────┬─────────────────────────────┘ │
                   │ gRPC (ledger API)             │
                   ▼                               │
   ┌──────────────────────────────────────┐        │
   │      Ledger Service (Rust)           │        │
   │  tonic/gRPC server                  │        │
   │  · double-entry posting engine      │        │
   │  · holds / captures / releases      │        │
   │  · idempotent journal execution     │        │
   │  · row-lock orchestration           │        │
   │  · balance queries, entry paging    │        │
   │  · internal audit recomputation     │        │
   └──────────────────┬───────────────────┘        │
                      │ owns (writes)              │
                      ▼                            │
   ┌──────────────────────────────────────┐        │
   │        PostgreSQL (RDS)              │        │
   │  ledger schema  (Rust-owned)         │        │
   │  app schema     (Java-owned)         │        │
   │  outbox events  (ledger → Java)      │◄───────┘
   └──────────────────────────────────────┘
                      │
                      ▼
   Redis (ElastiCache): rate-limit counters, OTP store, session cache
```

### Why this shape: modular monolith (Java) + dedicated ledger engine (Rust)

**Not a microservices sprawl.** A team building a payments platform with correctness as the priority gets no benefit from 10 small services and pays dearly in distributed-transaction and operational complexity. We run **two** deployables:

| Deployable | Language | Responsibility |
|---|---|---|
| **Core API Service** | Java / Spring Boot | Everything that is I/O-bound, state-machine-heavy, or integration-heavy: identity, KYC, merchant/agent management, checkout flows, rail adapters, webhooks, notifications, reconciliation orchestration, admin tooling. |
| **Ledger Service** | Rust | The correctness-critical hot path: journal posting, holds, balance math, idempotent execution, locking discipline. |

### Why Rust owns the ledger

- **The ledger is the hardest part to get right and the most expensive to get wrong.** Rust's type system makes illegal states unrepresentable (sum types for entry direction, journal status, hold state), and its memory-safety guarantees remove an entire class of bugs from the one component where a bug means lost or duplicated money.
- **Single-writer discipline is easier to enforce.** One process, one language, one codebase owning all balance mutations makes the "correctness over throughput" rule structurally enforceable rather than a convention.
- **Performance headroom.** SLE-scale volumes (even thousands of TPS) don't *require* Rust's speed — the honest statement is that we're choosing Rust for correctness and auditable concurrency, not raw throughput. The performance is a free bonus, not the justification.
- **Java's strengths are used where they matter.** Spring Boot gives mature web/security/integration ecosystems for the long tail of I/O work (rail SDKs, email, admin UIs, config). Nothing in that long tail is correctness-critical in the same way the ledger is.

### Why gRPC as the Java↔Rust boundary (and the tradeoffs)

| Option | Verdict | Tradeoff |
|---|---|---|
| **gRPC over localhost** (chosen) | ✅ | One network hop on localhost is ~0.5 ms — negligible next to the Postgres round trip the call performs anyway. Gives a versioned protobuf contract, independent deploy/scaling on ECS, and a clean place for cross-service authn (mTLS or shared secret). |
| In-process FFI (JNI) | ❌ | Lowest latency, but couples JVM and Rust lifecycle: one panic kills the whole API process; hard to scale/deploy independently; painful JNI ABI maintenance. |
| Message queue (Kafka/SQS) | ❌ | Async decoupling is attractive for scale but wrong here: holds and transfers need a synchronous response (did the hold succeed?). Async ledgering would force compensating complexity in the Java layer for no current benefit. Kafka can be added later for event fan-out if volume demands it. |

The boundary contract is a small protobuf service — see §8 and `docs/api.md`.

### Frontend client — React SPA (chosen)

| Option | Verdict | Tradeoff |
|---|---|---|
| **React (TypeScript, Vite) SPA** (chosen) | ✅ | One deployable static bundle served from CDN/S3+CloudFront, built by the same team. Mature ecosystem for forms, accessibility, and state; the client is deliberately **thin** (see below), so framework risk is low. |
| Server-rendered (Next.js/SSR) | ⚠️ | SSR adds value for SEO/public marketing pages, but the app itself is authenticated and JS-only; we can add Next.js later if landing/marketing pages need SSR. Not chosen now to keep one build pipeline. |
| Native mobile (React Native / Flutter) | ❌ deferred | The product starts as mobile-first **web** (Sierra Leone context: web access via phone browsers is the lowest-friction on-ramp). A React Native app reusing the same API and state model can be added later. |

Hard rules for the React client (enforced in review, not just convention):

- **The client is a view layer only.** No balance arithmetic, no fee math, no
  business rules, no authorization decisions. All financial truth comes from
  the Core API; balances displayed are server responses (`GET /v1/wallets`).
  See `docs/ux-flows.md` for the screen map and §51 of the checklist.
- **No secrets ever.** Tokens live in memory / HttpOnly cookies as designed in
  `docs/authentication.md`; API keys and webhook secrets never touch the
  client.
- **Every request is idempotent-safe.** Money-moving forms generate and reuse
  an idempotency key per submission attempt (retry-safe on network loss and
  double-clicks) — see `docs/transaction-state-machine.md`.
- **Accessible by default** (WCAG 2.2 AA target): semantic HTML, keyboard
  navigation, focus states, screen-reader support, adequate contrast
  (§ `docs/ux-flows.md`).

---

## 3. Java/Rust Data Ownership

Single PostgreSQL instance, two schemas, two database roles:

- `ledger` schema — tables `accounts`, `journals`, `entries`, `holds`, `idempotency_keys`. **Only the Ledger Service can write here** (it connects as the `amber_ledger` role which owns the schema).
- `app` schema — users, merchants, agents, KYC, checkouts, transfers, topups, cash movements, rail transactions, webhooks, notifications, fraud, reconciliation. Owned by the Core API Service (`amber_app` role).

The `amber_app` role has **no DML privileges** on the `ledger` schema. If the Java service ever needs ledger data, it goes through gRPC (or read-only views we grant deliberately). This is the enforceable version of the "ledger engine is the only writer" rule.

An **outbox** table (`ledger.ledger_events`) is written by the Ledger Service inside the same transaction as each journal. The Java service consumes it (polling or `LISTEN/NOTIFY`) to drive webhooks, notifications, and fraud scoring. This gives transactional reliability without two-phase commit: the event either exists with its journal or not at all.

---

## 4. Data Flows

### 4.1 Merchant checkout (wallet-funded)

1. Customer pays on merchant site → merchant backend calls `POST /v1/checkouts` (merchant API key, amount, currency, `Idempotency-Key`).
2. **Java**: authenticates merchant, runs fraud pre-checks (rate limit, velocity, KYC/limit gates), creates `checkouts` row (`status=created`).
3. **Java → Ledger (gRPC)**: `HoldFunds(buyer_wallet, amount, currency, expires_in)`.
4. **Ledger (Rust)**: single DB transaction → lock buyer wallet row (`SELECT … FOR UPDATE`) → balance check → post hold journal (debit buyer wallet / credit `hold_escrow`) → return `hold_id`. Wallet's *available* balance drops; *held* rises.
5. **Java**: returns checkout id + payment URL. Customer confirms with PIN → `POST /v1/checkouts/{id}/confirm` → Java verifies PIN, calls gRPC `CaptureHold(hold_id)`.
6. **Ledger (Rust)**: new journal — debit `hold_escrow`, credit merchant wallet, credit `fee_revenue` (0.5%, computed in Rust via `round_fee`, banker's rounding — see §9 flag F3). Hold status → `captured`.
7. **Java**: outbox event → webhook `checkout.succeeded` to merchant (HMAC-signed, retried with backoff) + email to buyer.

### 4.2 P2P transfer

1. User initiates `POST /v1/transfers` with PIN verification; OTP required above the high-value threshold.
2. **Java**: validates recipient, calls gRPC `PostPayment` (fee computed in Rust, 0.5% charged to the payer) with idempotency key.
3. **Ledger (Rust)**: `post_payment` — one transaction — lock accounts **in canonical order** → check balances → post the fee journal (payer pays principal + fee, payee gets principal, `fee_revenue` keeps the fee) and, when an agent facilitated it, the commission journal (`fee_revenue` → agent float, platform-absorbed) atomically → commit. Idempotency keys `{key}` and `{key}:commission` commit or roll back together.
4. Outbox → notification to both parties.

### 4.3 Mobile money top-up (user wallet → mobile money) and agent cash-in/cash-out

1. **Cash-in**: agent receives cash → `POST /v1/agents/cash-in` → Java calls Orange Money / Afrimoney to move e-money into the platform's rail account, then gRPC `Transfer` credits the customer's wallet. The `rail_transactions` row links the external rail reference to the journal.
2. **Cash-out**: mirror image — wallet debited, e-money pushed to the customer's mobile money number; agent float earns commission.
3. The rail call and the ledger post are **not atomic** (external system). We treat the ledger as source of truth and mark the journal `pending` until the rail confirms, then `posted`; the **reconciliation job** (§6) is the safety net that catches drift.

### 4.4 Cards (3DS)

Card payments go through a PSP (acquirer) — 3DS liability shift only applies if the PSP/network runs 3DS. The platform redirects the buyer to the PSP's 3DS challenge; on success the PSP confirms settlement, we post the ledger journal, and the rail reference is the PSP charge id. (See flag F1 — we will research and pick a PSP via the Gravity Index before integrating.)

---

## 5. Ledger Design Details

### 5.1 Double-entry structure

- `journals` — a group of ≥2 `entries` that sums to zero (debits = credits). Each journal has a type, status, currency, amount, and full origin metadata (user, channel, session, payment code) — the "ownership graph" requirement.
- `entries` — single leg: account, direction, amount, currency. Immutable once posted; balances are always derived from entries (see `wallet_snapshots` read model in `docs/schema.md`).
- `accounts` — chart of accounts. Wallets are accounts with an owner. Platform-side accounts (`fee_revenue`, `hold_escrow`, per-rail `rail_bridge`, `platform_revenue`, `tax_payable`) are internal accounts with no owner.
- **Invariants enforced by the engine + a nightly audit job** (not just code review):
  - Σ debits = Σ credits per journal.
  - A wallet account's balance never goes negative (only `hold_escrow` and fee accounts can go negative if we ever allow it — we don't; all balances are ≥ 0 by construction).
  - `wallet_snapshots == Σ posted entries` per account (snapshots rebuilt nightly and after commits; drift = alert).

### 5.2 Holds (reservations)

Holds are **real posted entries**, not flags:

1. **Hold**: journal `hold` — debit buyer wallet, credit `hold_escrow` (both `posted`). `held = Σ open holds` for the wallet; `available = Σ wallet entries` (holds already moved funds into `hold_escrow`, so the wallet's own entries are unencumbered); `total = available + held`.
2. **Capture**: journal `capture` — debit `hold_escrow`, credit merchant wallet. Hold row → `captured`.
3. **Release/expiry**: journal `release` — debit `hold_escrow`, credit buyer wallet. Hold row → `released`.

Because holds are visible in the ledger as real entries, reconciliation and audit see them, and expiry is a scheduled sweep, not a special case.

### 5.3 Concurrency

- **Pessimistic locking**: the Ledger Service locks `accounts` rows (`SELECT … FOR UPDATE`) in **canonical order** (sorted by account id) inside a single transaction. Because there are no stored balances, the engine then **computes** balances from `entries` under those locks, validates, and posts. The account row is the mutex; entries are the state. Multiple concurrent transfers touching the same wallets serialize safely; deadlocks are structurally impossible.
- **Single-writer**: only the Ledger Service ever holds these locks; there is no other path that can mutate balances concurrently.
- **Idempotency**: unique `(scope, idempotency_key)` on `journals`; the gRPC layer replays the stored response for a duplicate key so retries are safe end-to-end. The Java API additionally stores idempotency responses for its own endpoints.

### 5.4 Time & money

- All timestamps `TIMESTAMPTZ` (UTC storage).
- All amounts `BIGINT` minor units. Fee = `round(amount × bps / 10000)` with banker's rounding, applied per journal (see F3 for the rounding policy decision). A government transaction tax (e-levy-style) uses the same exact rounding and posts into the `tax_payable` liability — configured per payment via `TaxPolicy` (see F7); no schema change to enable.

---

## 6. Rails, Reconciliation & Fraud

### 6.1 Rails adapters (Java)

One adapter per rail, behind a common interface. Reality check: Orange Money and Afrimoney APIs in Sierra Leone may be SOAP/XML or poorly documented, and webhooks may be unreliable or absent — the adapter layer exists to absorb this. **Design assumption: callbacks are treated as acceleration signals only; the reconciliation job is the source of truth.**

### 6.2 Reconciliation

Two jobs, both scheduled (nightly, plus on-demand):

1. **Internal ledger audit** (Ledger Service): recompute every account balance and the `wallet_snapshots` from `entries`; verify snapshot == recomputation; alert on any drift.
2. **Rail reconciliation**: the matching engine lives in the Rust Ledger Service (`ledger/src/reconcile.rs`): a pure `reconcile()` that cross-matches statement lines against `journals.payment_code` and compares the `rail_bridge` net movement to the statement total, plus a `ReconcileService` that pulls the ledger side from Postgres. A `RailStatementSource` trait separates the rail-specific fetch (stub today; Orange Money adapter lands with the rail integration). The Java core schedules runs, embeds the internal reference in the rail memo where supported, and alerts ops on `Drift`. Outcomes per line: `matched`, `missing_internal`, `missing_external`, `amount_mismatch`, `duplicate_reference`.

### 6.3 Fraud & security (Stripe-style model)

- **Rate limiting**: token-bucket per API key / user / IP in Redis, enforced at the gateway. 429s with `Retry-After`.
- **Velocity & anomaly checks** (Java): daily/monthly volume limits per user (configurable in `velocity_limits`), unusual-geography / unusual-amount signals, block or challenge (OTP) on threshold breaches. Hits recorded in `fraud_rule_hits` for audit.
- **Webhooks to merchants**: HMAC-SHA256 signature over the raw body with a per-merchant secret; exponential backoff with jitter; dead-letter after N attempts; delivery log in `webhook_deliveries`.
- **Secrets**: AWS Secrets Manager / SSM. Passwords: bcrypt/argon2. PINs: argon2, verified only at transaction confirmation. OTPs: single-use, TTL ~5 min, hashed at rest.
- **3DS**: handled by the card PSP (liability shift) — see F1.

---

## 7. Deployment (AWS ECS)

- Two Fargate services: `core-api` (Java) and `ledger` (Rust), behind an ALB. Rust service is internal-only (no public exposure).
- RDS PostgreSQL (primary + read replica for reporting), ElastiCache Redis.
- ECR images, IaC via Terraform (later step).
- CloudWatch logs/metrics; PagerDuty alerts on reconciliation mismatches and ledger audit drift.

---

## 8. Ledger gRPC Interface (summary)

```proto
service Ledger {
  rpc HoldFunds(HoldRequest) returns (HoldResponse);          // reserve funds
  rpc CaptureHold(CaptureRequest) returns (CaptureResponse);  // settle a hold
  rpc ReleaseHold(ReleaseRequest) returns (ReleaseResponse);  // cancel a hold
  rpc PostTransfer(TransferRequest) returns (JournalResponse);// P2P / top-up / cash movement
  rpc PostFee(FeeRequest) returns (JournalResponse);          // standalone fee posting
  rpc GetAccount(AccountRequest) returns (AccountResponse);   // balance (available/held/total)
  rpc ListEntries(ListRequest) returns (ListResponse);        // paginated statement
  rpc AuditAccounts(AuditRequest) returns (AuditResponse);    // internal recompute job
}
// Every request carries: idempotency_scope, idempotency_key, origin (user/channel/session/payment_code)
```

---

## 9. Design Flags — where I propose to deviate or decide

The following are places where I recommend something different from, or more specific than, the brief. **None are implemented yet.**

- **F1 — Cards/3DS requires a PSP.** 3DS liability shift is a network/acquirer mechanism; AMBER PAY cannot implement 3DS directly against Visa/Mastercard in Sierra Leone without becoming an acquirer. We must integrate a card PSP (e.g. Stripe, Paystack, Flutterwave, or a local acquirer). **Recommendation:** defer card integration until the wallet/mobile-money core is live, then choose the PSP via structured research (Gravity Index) with the regulatory context (see F2).
- **F2 — Custodial wallets need licensing & segregated funds.** Holding customer balances makes AMBER PAY an e-money/payments institution in Sierra Leone; this requires Bank of Sierra Leone licensing and, prudentially, segregated trust/escrow bank accounts for customer funds, with agent floats backed separately. **Not a code blocker for building, but a hard go-live gate.** I'd flag it now so the ledger design (internal `rail_bridge` and escrow accounts) already supports segregation.
- **F3 — Fee incidence (confirmed): the 0.5% platform fee is charged to the paying party** per transaction (Monime-style), recorded as its own journal leg, with the incidence per transaction type configurable via `fee_configs.charged_to` (`payer` | `merchant`). Remaining open detail: rounding (banker's rounding as default) and the dust-transaction floor — pure 0.5% stands for now, but `fee_configs.min_amount_minor` is ready if a minimum fee is ever needed (agents doing many tiny cash-ins could otherwise arbitrage commission).
- **F4 — "Single balance per wallet" is insufficient as a UI concept.** The brief says double-entry, which we do; but wallets need `available / held / total` exposed, and holds expire — so the API exposes all three.
- **F5 — Rails don't have reliable webhooks.** Plan for polling + reconciliation as the source of truth (see §6.1), even where a rail offers callbacks.
- **F6 — SMS OTP delivery is unreliable in Sierra Leone.** Short TTL, single-use, email fallback, and OTP re-issue limits; PIN remains the primary transaction control and OTP only for high-value/profile changes (per brief).
- **F7 — Government transaction tax is configuration, not schema (implemented).** E-levy-style levies are a live regional pressure (Ghana 2022; see `docs/regulatory-sierra-leone.md`), so `post_payment` accepts an optional `TaxPolicy { bps }`; the tax leg credits the `tax_payable` **liability** — never revenue — and rides in the payment journal, so reversals return tax automatically. Enabling/changing/removing a tax is a request parameter, not a migration. Whether/when a tax applies in any launch market: `REGULATORY_REVIEW_REQUIRED`.
- **F8 — Event sourcing deferred.** Full append-only event sourcing is the ultimate audit structure, but doubles complexity. We get immutability + full audit trail from the double-entry design (derived balances, immutable entries, `wallet_snapshots`, outbox). If a future requirement (replay, projections) demands it, the `journals`/`entries` tables are already a compatible substrate.
- **F9 — Agent commission (confirmed):** the 0.5% platform fee is charged to the paying party; **agent commission is a separate cost the platform absorbs from that fee revenue**, paid into the agent's float at a configurable basis-points rate per transaction (`agents.commission_bps`), and is **never deducted from the customer or merchant directly**.

---

## 10. Build Order (incremental, per the brief)

1. **Ledger core (Rust)**: schema, accounts, journal posting, holds, idempotency, gRPC API, property-based tests (proptest) for the invariants in §5.1. *Nothing else builds on sand until this is right.*
2. **Identity + wallets (Java)**: registration, login, PIN, OTP, KYC capture, wallet read endpoints wired to the ledger.
3. **P2P transfers** end-to-end.
4. **Merchant checkout**: holds, capture, PIN confirm, webhooks + retries, email notifications.
5. **Mobile money rails — Orange Money first**: Orange Money adapter, top-ups, agent cash-in/out, rail reconciliation. Orange Money settles in under ~2 minutes (vs up to 24h for Afrimoney on the same transaction type), so holds/reservation logic can be validated end-to-end quickly; it also has stronger adult penetration in Sierra Leone and a documented developer API. Afrimoney is integrated second, once the ledger and hold logic are proven against a fast-confirming rail.
6. **Cards via PSP + 3DS** (after PSP selection per F1).
7. **Hardening**: rate limiting, velocity/fraud rules, admin/ops console, ledger audit jobs, notifications polish.

Each step ships with tests; step 1's ledger tests are the non-negotiable gate for everything else.