# AMBER PAY — Transaction State Machine & Idempotency

Three layers of state, owned by different components:

1. **Business transaction** (Java Core API, `app` schema domain tables) — the
   customer-visible lifecycle: created → … → completed.
2. **Ledger journal** (Rust Ledger Service, `ledger.journals`) — financial
   truth: `pending` → `posted` | `void`. Entries exist only for posted
   journals and are immutable once posted.
3. **Hold** (Rust Ledger Service, `ledger.holds`) — `held` → `captured` |
   `released` | `expired`.
4. **Provider/rail outcome** (Java adapters, `app.rail_transactions`) —
   external truth that can be unknown, delayed, or contradictory.

**Core rule: a business transaction may only reach a terminal financial state
when the ledger journal has posted, and a journal may only post when the
underlying funds movement is known.** An HTTP 200 from a provider is **not**
proof of success; a network timeout is **not** proof of failure.

---

## 1. Canonical business-transaction states

The checklist's full state set, mapped onto the platform:

| State | Meaning | Terminal? | Notes |
|---|---|---|---|
| `CREATED` | Request accepted, validated, nothing moved | no | Idempotency key recorded from here |
| `PENDING` | Awaiting user action (PIN/OTP, 3DS) or provider | no | |
| `PROCESSING` | Funds flow started; ledger journal posted or hold placed | no | |
| `COMPLETED` | Ledger settled; provider confirmed or reconciliation-matching | **yes** | |
| `FAILED` | **Definitively** failed (provider rejection, validation, declined) | **yes** | Only on explicit provider/rail failure — never on timeout |
| `CANCELLED` | User/merchant cancelled before funds moved | **yes** | Releases hold if any |
| `EXPIRED` | Payment window expired without completion | **yes** | Releases hold if any |
| `REVERSED` | Completed transaction later reversed via new ledger journal | **yes** | New journal, never an edit |
| `REFUNDED` | Completed transaction (fully) refunded via new ledger journal | **yes** | Partial refunds keep `COMPLETED` with refund records |
| `DISPUTED` | Customer dispute opened; funds may be held | no (until resolution) | Never mutates the original record |
| `HELD_FOR_REVIEW` | Fraud/compliance hold; no money moves | no | Requires case resolution |
| `UNKNOWN` | Provider outcome genuinely uncertain (timeout, lost callback) | **no** — resolves via reconciliation | Never auto-marked failed; funds stay safely held/pending |

Implementation note: the `app` domain tables (`checkouts`, `transfers`,
`topups`, `cash_movements`, plus future `withdrawals`, `refunds`, `disputes`)
use a subset of these as their status columns. The canonical enum above is the
**contract**; the Java state machine maps each domain table onto it, and the
`transaction_states`/history is captured in the app audit log
(`docs/security-controls.md`). Schema extension to the full set lands with the
Java core build.

---

## 2. Ledger journal lifecycle (Rust, implemented)

```
                 validate (validate_spec)
pending  ──────────────────────────────────────►  posted
   ▲                                                 │
   │ (rail-dependent journals: created pending,      │  immutable once posted
   │  posted only when funds movement is confirmed)  │
   └─────────────────────────────────────────────────┘
        void (never confirmed; no entries posted)
```

- `posted` journals are immutable; entries are never updated or deleted.
- Reversals/refunds/adjustments are **new journals** referencing the original
  (`JournalType::Reversal`, `docs/architecture.md` §3). Implemented today:
  holds, captures, releases, p2p, topup, cash-in/out, checkout, fee,
  commission, reversal, adjustment.
- Balances are always derived from posted entries under row locks
  (`ledger/src/balances.rs`).

### Hold lifecycle (Rust, implemented)

```
held ──capture──► captured
  │
  ├──release──► released
  └──expire──► expired     (scheduled sweep: expire_holds)
```

A hold is real posted money (wallet → hold_escrow); capture/release/expire
post the reverse journal. Capturing an expired or released hold fails.

---

## 3. Flow state maps (Java-owned, designed)

### 3.1 P2P transfer

```
CREATE ─► PENDING (awaiting PIN/OTP) ─► PROCESSING (journal posted) ─► COMPLETED
   │              │                             │
   │              ├─► CANCELLED (user backs out; nothing posted)
   │              └─► FAILED (validation/insufficient funds — nothing posted)
   └──► FAILED (recipient invalid, limits, fraud BLOCK)
PROCESSING ─► UNKNOWN (ledger posted but provider/callback lost) ─► COMPLETED via reconciliation
COMPLETED ─► REFUNDED (new refund journal)   COMPLETED ─► DISPUTED ─► (resolution)
```

- Journal posted at `PROCESSING` in the same call as the domain transition;
  idempotency key ensures exactly one journal per transfer.
- Insufficient funds → `FAILED` **with no journal** (nothing was written; the
  engine's atomicity guarantees this, see `ledger/tests/engine.rs`).

### 3.2 Merchant checkout (wallet-funded)

```
created ─► requires_payment ─► processing (hold placed) ─► succeeded (captured)
   │               │                    │
   │               └──► cancelled / expired (release hold if any)
   │                                      └──► failed (capture rejected, provider declined)
   └──► failed (validation, fraud BLOCK)
succeeded ─► refunded (refund journal)   succeeded ─► disputed ─► (resolution)
```

- `HoldFunds` at `requires_payment`; `CaptureHold` at `succeeded`; releases on
  cancel/expire. All three are ledger journals — the hold, capture, and any
  release are all visible in the ledger.
- Card checkout: PSP 3DS flow runs before capture; PSP timeout → `UNKNOWN`
  (never auto-fail), resolved by PSP status polling + reconciliation.

### 3.3 Mobile-money top-up / cash movement (rail-dependent)

```
pending ─► processing (rail request sent; journal pending or posted) ─► posted
   │                 │
   │                 ├─► failed (rail definitively rejects; journal voided if never posted)
   │                 └─► unknown (timeout, lost callback) ─► posted/void via reconciliation
   └──► failed (validation, limits)
```

- The rail call and the ledger post are not atomic (external system). The
  ledger journal starts `pending`; it becomes `posted` only on rail
  confirmation or reconciliation matching. **Funds already debited from the
  customer for a payout that fails are never left stranded**: the safe-release
  path below applies.

### 3.4 Withdrawal (future, designed per §17)

```
withdraw ─► destination ─► amount ─► fee ─► confirmation ─► risk ─► authentication
  ─► create withdrawal (hold funds) ─► provider ─► confirmation ─► finalize
```

- Funds are **held** (not deducted) at creation; deduction happens at
  `finalize` when the provider confirms.
- **Failed payout** → the hold is released and the withdrawal marked `FAILED`:
  customer funds return to available. **Never**
  `provider failure + funds deducted + no recovery path`.
- Timeout → `UNKNOWN`; reconciliation resolves; funds remain held until then.

---

## 4. UNKNOWN / PENDING handling (the provider-uncertainty rule)

1. A provider **timeout** or **lost callback** maps the transaction to
   `UNKNOWN` (or stays `PENDING`) — **never** `FAILED` and never silently
   `COMPLETED`.
2. Funds affected by an `UNKNOWN` transaction stay held (or the journal stays
   `pending`) until the provider state is established by:
   - provider status polling (`CheckPaymentStatus` / `CheckPayoutStatus`),
   - a verified webhook, or
   - the reconciliation job (`docs/architecture.md` §6.2) — the source of
     truth for rails.
3. `UNKNOWN` has a **resolution deadline**: an aging job alerts ops when an
   `UNKNOWN` exceeds its SLA window (configurable per rail); unresolved items
   surface as reconciliation drift and page ops. Any unexplained customer-funds
   discrepancy is a critical incident.
4. Contradictory statuses (webhook says success, status poll says failed) are
   resolved by reconciliation, with the **ledger as source of truth**: the
   journal only posts once, and the domain transaction follows the ledger.

---

## 5. Idempotency rules (every money-moving API)

Requirements (mirrored from `ledger/src/engine.rs`, which implements them at
the ledger layer; the Java layer enforces the same on its own endpoints):

1. **Every money-moving POST requires an idempotency key** (`Idempotency-Key`
   header): payments, transfers, deposits/topups, withdrawals, refunds,
   holds/captures/releases.
2. The key is bound to the **authenticated requester scope**
   (`(idempotency_scope, idempotency_key)` unique in `ledger.idempotency_keys`
   and `ledger.journals`).
3. Stored per key: requester scope, **request hash**, status
   (`in_progress` | `done`), cached response, transaction id, timestamps,
   expiration policy.
4. **Same key + same request** → the original result is replayed; **one
   financial operation total**. (Implemented and tested:
   `ledger/tests/engine.rs` `duplicate_key_replays_without_double_posting`.)
5. **Same key + different payload** → **rejected** (409, idempotency
   mismatch). ✅ **Implemented**: the engine stores `hash_spec(spec)` (journal
   type, currency, origin, and every leg) on the cache row **and** on the
   `journals` row (`request_hash`, migration 0004); replay verifies it and
   returns `IdempotencyMismatch` on any difference
   (`ledger/tests/engine.rs` `same_key_with_different_payload_is_rejected`).
6. **Expiration policy**: keys expire after a TTL (24 h constant
   `IDEMPOTENCY_KEY_TTL`); an expired/stale `in_progress` row is **reclaimed**
   rather than blocking forever, and `LedgerEngine::prune_expired_idempotency_keys`
   sweeps expired cache rows. ✅ **Implemented** — and safe because the
   `journals` (scope, key) row is the durable backstop: replay falls back to
   it even after the cache row expires or is pruned
   (`expired_done_key_replays_from_journal`, `pruned_cache_still_replays_from_journal`,
   `expired_in_progress_key_is_reclaimed`, `expired_in_progress_key_with_journal_replays`).
7. Concurrent submission of the same key → one wins; the other receives
   `409/in_progress` and retries (tested:
   `idempotency_claim_race_fails`).
8. `POST /v1/checkouts` and domain endpoints carry their own idempotency
   columns (`UNIQUE (merchant_id, idempotency_key)` etc.), so the **domain
   row** and the **ledger journal** are both replay-safe.

---

## 6. Reversals & refunds — implemented engine operations

- **`reverse_journal`** — full reversal of a posted journal: a new `reversal`
  journal negates every leg (principal **and** fee return to the payer),
  `reference` = original journal id, original never edited. Guards: original
  posted and reversible (holds lifecycle types and reversal/refund journals
  are excluded), not already reversed, no refunds exist. Tests:
  `ledger/tests/reversals.rs`.
- **`refund_payment`** — partial or full refund: payee returns principal to
  the payer, `reference` = original payment journal; the platform fee stays
  earned. Guards: original posted and refundable, not reversed,
  `refund_from` must be the original's payee, cumulative refunds ≤ payee
  principal. All guards run under the original journal's row lock, so
  concurrent reversals/refunds of the same journal serialize.
- A payment's agent commission is a **separate** journal; reversing the
  payment does not reverse the commission (caller reverses it separately via
  `commission_journal_id`).

## 7. Mandatory failure-path guarantees (design invariants)

| Scenario | Guarantee |
|---|---|
| Double click / duplicate submission | One financial operation (idempotency) |
| Refresh during payment | State is server-authoritative; the refreshed screen re-reads the transaction; idempotency prevents double-post |
| Network loss mid-request | Transaction stays `PENDING`/`UNKNOWN`; retry with the same key replays or continues |
| Provider timeout | `UNKNOWN`/`PENDING`, never `FAILED` |
| Provider sends duplicate events | Webhook event-id idempotency + ledger idempotency → no duplicate ledger entry |
| Two simultaneous withdrawals | Row locks + derived balances → cannot exceed available funds |
| Database failure mid-operation | One ACID transaction → no partial financial commit |
| Reversal/refund | New ledger event referencing the original; original never edited |
| Failed payout | Funds safely released/reconciled, never stranded |

Each of these is a named test case in `docs/testing-strategy.md` §61.