# AMBER PAY — Provider Abstraction & Webhook Architecture

## 1. Payment provider abstraction (§20)

A single adapter interface so AMBER PAY is never tightly coupled to one rail
(Orange Money first, then Afrimoney, bank transfer, card PSP, per
`docs/architecture.md` §10). Adapters live in the Java Core API; the ledger
service talks to no provider directly.

### 1.1 Adapter interface

```java
interface RailAdapter {
  Rail rail();

  // Payments / payouts
  CreatePaymentResult createPayment(PaymentRequest req);      // top-up, checkout pay
  PaymentStatus checkPaymentStatus(String railReference);     // poll
  CreatePayoutResult createPayout(PayoutRequest req);         // cash-out, withdrawal
  PayoutStatus checkPayoutStatus(String railReference);       // poll
  RefundResult refund(RefundRequest req);                     // where the rail supports it

  // Verification & health
  VerifyResult verifyWebhook(WebhookEnvelope envelope);       // signature + timestamp + event id
  HealthStatus health();                                      // reachability / degraded
  List<StatementLine> statement(DateTime since, DateTime until); // for reconciliation
}
```

### 1.2 Failure semantics (the contract)

| Condition | Adapter contract |
|---|---|
| Timeout | return `UNKNOWN` — never success, never failure |
| Rail outage | `health()` degraded; calls fail fast with typed `RailUnavailable`; jobs retry with backoff |
| Delayed callback | status polling + reconciliation resolve; callback is acceleration only (F5) |
| Duplicate callback | adapter verifies event id; core dedupes (below) |
| Contradictory statuses | recorded as-is; reconciliation (ledger = source of truth) resolves |
| Partial failure | typed result with per-line/partial outcome; core handles per domain state |
| Unknown status | maps to transaction `UNKNOWN` (docs/transaction-state-machine.md §4) |

Every provider call records request/response payloads in
`app.rail_transactions` (schema exists: rail, journal_id, rail_reference,
rail_status, payloads, amount, `UNIQUE (rail, rail_reference)`) — the audit
and reconciliation substrate.

### 1.3 Provider health & status

`GET /v1/rails/{rail}/status` (ops): last call latency, error rate, last
successful callback, health probe result. Degraded rails trip feature flags
(`NEW_PAYMENT_RAIL` per rail) and can be disabled by ops
(docs/fraud-risk.md §7).

## 2. Webhook architecture (§19)

### 2.1 Inbound (provider → AMBER PAY)

`POST /v1/rails/{rail}/callbacks` (public but per-rail authenticated):

1. **Authenticate provider** — shared secret per rail; TLS only.
2. **Verify cryptographic signature** — HMAC (or provider's scheme) over the
   raw body; per-rail secret from Secrets Manager. **Never trust an unsigned
   payload.**
3. **Validate timestamp** — reject outside a freshness window (e.g. ±5 min)
   against replay.
4. **Prevent replay** — the webhook's event id must be unique: store every
   event in `app.webhook_events` (planned) with `UNIQUE (rail, event_id)`;
   duplicate event ids are dropped (or reported) and never reprocessed.
5. **Validate event id + transaction mapping** — the event references a
   `rail_reference` that must exist in `app.rail_transactions`; unknown
   references go to reconciliation as `missing_internal`.
6. **Enforce idempotency** — the domain transition it triggers carries an
   idempotency key derived from `(rail, event_id)`; duplicate processing can
   never double-post (ledger backstop).
7. **Store event securely** — raw payload + verification result + processing
   result persisted (audit + replay for investigation).
8. **Process asynchronously** — enqueue; respond 202 to the provider quickly;
   a worker applies the state transition.
9. **Support retries** — provider retries are deduped by event id; our
   processing retries with backoff + dead-letter (`webhook_events` status:
   `received` → `processed` | `failed` | `dead`).
10. **Record processing result** — every attempt logged; failures alert ops
    (a webhook that fails to process is a reconciliation risk).

**Callbacks are acceleration signals; reconciliation is the source of truth**
(F5). A forged or lost callback therefore cannot silently move or strand
money.

### 2.2 Outbound (AMBER PAY → merchant/business)

Delivered to `merchants.webhook_url`, signed
`X-Amber-Signature: sha256=HMAC(webhook_secret, raw_body)`, retried with
exponential backoff + jitter (1m → 2m → 4m … max 10 attempts, then
dead-letter). Delivery log in `app.webhook_deliveries` (schema exists).
Events: `checkout.created/succeeded/failed/expired/refunded`,
`transfer.completed`, `topup.completed`, `agent.cashin.completed`,
`agent.cashout.completed` — payload shape per `docs/api.md` §11.

- Webhook endpoint URLs are validated at merchant setup: https-only,
  SSRF-guarded, and limited to the merchant's registered domains where
  possible.
- Outbound events are produced from the **ledger outbox**
  (`ledger.ledger_events`, written in the journal transaction), so an event
  exists iff its journal exists — exactly-once downstream (architecture §3).

## 3. Sandbox (§50)

- **SANDBOX** and **PRODUCTION** are separate environments with separate
  credentials and separate databases. Sandbox credentials can never reach
  production funds (enforced by credential separation + environment
  config; nothing is shared).
- Sandbox rail adapters simulate: successful payment, failed payment, pending
  payment, timeout, refund, webhook retry, duplicate webhook, provider
  outage — the test scenarios from §50, used by E2E and provider-testing
  suites (docs/testing-strategy.md).
- Every sandbox transaction is labeled `SANDBOX_PAYMENT` in the ledger
  outbox/UI so it can never be mistaken for real money (§73).

## 4. Provider-specific notes

- **Orange Money (Sierra Leone)**: first rail (fast confirmation, documented
  API — architecture §10). Adapter converts their statement/callback formats
  to the signed `StatementLine` convention (docs/architecture.md §6.2).
- **Afrimoney**: second (slower settlement — up to 24 h); polling +
  reconciliation critical.
- **Bank transfer**: rails via partner banks (to be scoped) —
  `REGULATORY_REVIEW_REQUIRED` for settlement terms.
- **Cards/PSP**: selection via structured research (Gravity Index) per F1;
  3DS handled by the PSP; integration deferred until wallet/mobile-money core
  is live.