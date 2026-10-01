# AMBER PAY — Fraud & Risk Engine Design

Owned by the Java Core API (`app` schema: `fraud_rule_hits`, `velocity_limits`,
plus planned `risk_decisions`, `fraud_cases`, `account_freezes`). The ledger is
never bypassed: a risk decision can hold, block, or step up, but it cannot
post money directly.

**Never expose internal fraud rules to customers.** Error messages and UI
must not reveal thresholds, rule names, or decision logic.

## 1. Risk signals (§22)

Evaluated server-side at transaction time (and on sensitive non-financial
actions):

- **Amount** — size vs. account history, daily/monthly volumes.
- **Transaction velocity** — count/volume per user, per destination, per
  device, per IP in rolling windows.
- **Account age** — restrictions for newly created accounts.
- **Device** — new device, device risk status (docs/authentication.md §6).
- **IP/network** — new location, VPN/proxy, known-bad ranges.
- **Location** — geo-inconsistency with account/device history.
- **Recipient history** — first-time recipient, recipient risk score,
  recipient velocity.
- **Failed authentication** — recent PIN/OTP/password failures.
- **Beneficiary changes** — new or recently changed beneficiaries.
- **Withdrawal patterns** — withdrawal velocity, destination changes,
  overnight patterns.
- **Historical behavior** — deviation from the account's normal profile.
- **Other signals** — chargeback/refund rates (merchants), structuring
  indicators (AML cross-ref docs/kyc-aml.md), and provider-side signals.

## 2. Risk levels and decisions

Risk levels: **LOW · MEDIUM · HIGH · CRITICAL**.
Decisions: **ALLOW · STEP_UP_AUTH · HOLD · MANUAL_REVIEW · BLOCK**.

| Decision | Behavior | Example trigger |
|---|---|---|
| ALLOW | proceed | low risk, known device, normal amount |
| STEP_UP_AUTH | require extra factor (OTP/MFA/PIN re-entry) | new device, first-time recipient |
| HOLD | funds held (ledger hold) pending review | medium-high suspicion |
| MANUAL_REVIEW | create a `fraud_cases` item for a human | HIGH score, pattern match |
| BLOCK | reject before any money moves; generic user message | CRITICAL score, known-bad signals |

Design: a decision engine evaluates a **rule set** (configurable, versioned,
behind feature flags — e.g. `NEW_FRAUD_MODEL`) into a score/level, then maps
level → decision per transaction type. Every evaluation is recorded in
`risk_decisions` (actor/transaction, signals used, version, decision) for
audit and model iteration. Rules run **before** any ledger call for
`BLOCK`/`STEP_UP_AUTH`, and can trigger `HOLD` around the hold/capture cycle
for checkout-style flows.

## 3. Velocity and limits

`velocity_limits` (schema exists): scope (user/merchant/agent/global), key
(`daily_send_minor`, `daily_checkout_count`, `monthly_volume`, …), limit,
window, action (`block` | `challenge_otp`). Enforced in the Core API before
ledger calls; breaches recorded in `fraud_rule_hits`. Default limits are
country-configurable (docs §44 of the checklist) — `REGULATORY_REVIEW_REQUIRED`
for any statutory limits (e.g. transaction caps).

## 4. Account takeover protection (§23)

Detect and respond to: new device, new location, unusual IP/network, password
change, phone change, email change, MFA change, beneficiary change, and
unusual transaction behavior.

Responses (escalating): security notification → step-up authentication →
temporary sensitive-operation restriction → account freeze → fraud case
creation. See also threat model threats 1–5 (`docs/threat-model.md`) and the
auth triggers in `docs/authentication.md` §2.

## 5. Account freeze (§24)

`Freeze my account` (self-service, in Settings → Security) and
platform-initiated freezes (fraud/compliance/support escalation):

- Immediate restriction of financial activity (ledger `accounts.status =
  'frozen'` blocks posting — enforced by the engine's `AccountNotActive`
  check; tested in `ledger/tests/engine.rs`).
- Clear explanation to the user; strong authentication (MFA) to unfreeze;
  security notification on freeze and unfreeze; support escalation path;
  audit events `ACCOUNT_FROZEN` / `ACCOUNT_UNFROZEN`.
- Fraud/compliance freezes are case-linked; self-freeze can be lifted
  self-service after verification.

## 6. Fraud cases and investigation

`fraud_cases` (planned): case id, status, timestamps, assignment, linked
transactions/accounts/devices, evidence, resolution — same lifecycle shape as
disputes (§34). Fraud analysts work cases with **case-scoped** data access
(docs/authorization.md §2), never open-ended browsing.

## 7. Operational rules

- Fraud rule configuration is a privileged, audited action
  (`LIMIT_CHANGED` / `ADMIN_ACTION`), behind feature flags, versioned.
- Emergency controls (checklist §69): disable withdrawals, disable a payment
  rail, freeze suspicious accounts, revoke compromised API keys, disable a
  compromised integration — all audited and reversible via the same
  maker–checker patterns where required.
- Every decision that touched money is reconcilable: HOLD decisions map to
  ledger holds; BLOCK happens pre-ledger; MANUAL_REVIEW items resolve to
  ALLOW/RELEASE or RELEASE-with-refund paths that are themselves new journal
  events.