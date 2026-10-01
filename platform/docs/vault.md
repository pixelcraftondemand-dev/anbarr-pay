# AMBER PAY — Vault (goal savings) design

> Vault is AmberPay's answer to the "savings lock" pattern users already
> trust from African fintechs (Monime-style goal pockets). It rides the
> ledger's existing holds engine — **no new balances, no new money path** —
> which is exactly why it is safe to ship: the correctness-critical engine
> is unchanged.

## 1. Model: a Goal is metadata over ledger holds

The engine (ledger/src/engine.rs) exposes exactly three hold operations:

- `HoldFunds` — debit wallet / credit `hold_escrow`, creating a `hold` row
  with `expires_at`;
- `CaptureHold` — settle the **whole** hold to a target account;
- `ReleaseHold` — return the **whole** hold to the wallet.

There is no partial release and no hold update. The Vault therefore treats a
**hold as atomic**: a Goal is a named group of whole holds plus display
metadata. Nothing in this design requires engine changes.

```
Core DB (users domain)                Ledger (only writer of money tables)
vault_goals                           holds + journals (type=hold)
  id            UUID                    hold_id  ← vault_goal_locks.hold_id
  user_id, name, currency            ↔
  target_minor?, status                hold_escrow account (per currency)
  created_at
vault_goal_locks
  goal_id, hold_id, amount_minor, locked_at, released_at?
```

- **Lock** = `HoldFunds(idempotency_scope="vault.goal.lock")`. The response's
  `hold_id` is stored in `vault_goal_locks`.
- **Top-up a goal** = another `HoldFunds` under the same goal (goals grow in
  whole-lock increments; the UI shows Σ of open holds).
- **Unlock** = `ReleaseHold` per open hold. A goal closes when it has no open
  holds; `status = active → released` is derived, not stored as money state.
- **Maturity** maps to `expires_at`. The ledger already refuses capture of an
  expired hold and its release path returns funds to the wallet; the Vault
  reads expired-but-unreleased holds as `matured` and offers one-tap return.
  Money is never lost and never earns fictional interest — what we display is
  Σ of real holds.

## 2. REST contract (added to docs/api.md §6b)

All endpoints require `Authorization: Bearer` and standard `Idempotency-Key`
semantics on POST. Money-movement endpoints additionally require `Pin-Token`
(2-min TTL from `POST /v1/auth/pin/verify`); OTP for high value follows the
transfer rules. Amounts are integer minor units.

| Method | Path | Description |
|---|---|---|
| GET | `/v1/vault/goals` | list goals: `{id, name, status: active\|released, currency, locked_minor, target_minor?, maturity_at?, locks: [{hold_id, amount_minor, locked_at, expires_at?}]}` |
| POST | `/v1/vault/goals` | create + first lock: `{name, currency, amount_minor, target_minor?, maturity_at?}` → 201 `{id, status:"active", locked_minor, hold_id}` |
| POST | `/v1/vault/goals/{id}/locks` | add a lock: `{amount_minor}` → `{hold_id, locked_minor}` |
| POST | `/v1/vault/goals/{id}/release` | unlock: `{hold_id?}` (omit = all open holds) → `{released_minor, released_holds}` |

Error mapping follows the standard problem+json table: 422
`insufficient_funds` (the funds rule fires at hold time), 404 unknown goal,
409 idempotency replay mismatch, 429 rate limited. Network-loss obeys the
transaction state machine: outcome UNKNOWN → UI pending, retry reuses the
same idempotency key and pin_token.

Server-side rules beyond the ledger:

- KYC tier gates goal count and per-goal cap (docs/kyc-aml.md tiers);
- a goal name is free text ≤ 60 chars, stored in the Core DB only (never in
  ledger tables — the ledger stays PII-free);
- release requires the same authentication strength as lock (PIN; step-up by
  risk engine) so a thief with the phone still needs the PIN to unlock.

## 3. Frontend flow (web/src/screens/Vault.tsx)

```
Vault list (goals + progress) ──► New goal sheet:
                                  name → target (optional) → amount keypad
                                  → review (fee = none; "your money, held
                                  on the ledger") → PIN → result
        │
        ├─ goal card ──► Add to goal (amount keypad → PIN)
        └─ goal card ──► Unlock (ConfirmSheet → PIN → released state)
```

Honesty rules carried over from the design system: the review screen states
there is **no fee** and no interest; the result screen derives its title from
the server status; unknown outcome renders as pending, never success. Until
the Core API ships these endpoints, the screen shows its real error state
(endpoint missing) — it never fabricates goals.
