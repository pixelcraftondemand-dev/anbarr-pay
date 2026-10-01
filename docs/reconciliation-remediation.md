# Reconciliation Record — `remediation/amberpay-scope-and-security`

**Decision:** Option A — **Rust stays the Core API platform of record**; only
stack-independent work is ported from the remediation branch. Recorded
2026-09-27 after review of all 13 commits on the branch.

## What the branch was

A parallel work track (2026-09-19/20, before the branch pipeline and
engineering standards existed) containing two very different things:

1. **A Java Spring rewrite of the Core API** (49 Java files, Flyway
   migrations, ~25-table `app` schema) — a different platform from the Rust
   axum implementation our line built and tested. Its own `gap-analysis.md`
   records it red at snapshot (67 tests: 10 failures + 9 errors) and running
   on in-memory stores until its persistence phase. No JDK/Maven exists in
   this environment to verify any of it.
2. **Stack-independent gold** in the shared Rust ledger, CI/CD, and web
   client — portable regardless of the platform decision.

It also closed three named security defects (C-4 unauthenticated agent
endpoints, C-5 checkouts that moved no money and allowed cross-merchant
refunds, C-6 unsigned rail callbacks). The *lessons* are valid on any stack;
the code that closed them is Java and was not adopted. §3 records how they
carry forward.

## What was ported (this branch)

- `ledger/src/sandbox.rs` — sandbox rail provider (`SandboxRail`,
  scenarios: success / failed / pending / timeout / refunded / duplicate
  webhook / provider outage) with statement sources and HMAC-signed
  webhooks; `ledger/tests/sandbox.rs` (375 lines).
- The remediation line's ledger test hardening (outbox, tax, reconcile,
  reversals, accounts, engine, fees, grpc, service suites).
- Crate rename `amber-ledger` → `anbarr-ledger` (matches the Anbarr Pay
  rebrand that line carried); core-api dependency, proto package
  (`anbarr.ledger.v1`), env vars (`ANBARR_*`), dev credentials
  (`anbarr`/`anbarr_dev`, db `anbarr`/`anbarr_core`), compose container,
  and CI service container all aligned on one naming scheme.

## What the port surfaced (and fixed)

The sandbox tests are DB-backed and **never ran on the remediation side**
(its gap-analysis: "6/11 green, 5 pending Postgres"). First execution here
exposed a sign-convention bug in `reconcile.rs`:

- `fetch_ledger_refs` and `bridge_movement` signed bridge legs as
  credit = +, while `StatementLine` and the whole sandbox use the platform
  view (+ = platform received ⇒ inbound payment debits the bridge).
- Consequence: every reconcile run with real fixtures reported
  `Drift` for balanced books (e.g. `Drift { -24000, +24000 }` for two
  settled topups, and `Drift { 0, 0 }` for payment+refund).
- Fix: debit = +, credit = − in both SQL sums (platform view), old
  `tests/reconcile.rs` fixtures flipped to the correct convention, and the
  sign convention is now documented at both query sites.

Result: workspace suite went 137 passed / 2 failed → **147 passed /
0 failed** (fmt + clippy `-D warnings` green).

## Deliberately not ported

- The entire Java `core-api/` (superseded by the Rust implementation:
  auth/OTP sessions, PIN step-up, P2P transfers with idempotency —
  22/22 integration tests at the time of the merge, no mocks on the
  money path).
- Their Dockerfiles, `docker-bake.hcl`, prod compose, deploy gating
  (valuable, but a separate, reviewable port — see follow-ups).
- Their web-journey screens (Checkout/KYC/Withdraw/Support + tests) —
  they target the Java contract; ours target the Rust contract. Porting
  needs a contract pass, screen by screen.

## Follow-ups (tracked)

1. **Verify C-4/C-5/C-6 against the Rust core** as the corresponding
   roadmap goals land: agent endpoints authz (goal 5), checkout
   capture/refund (goal 6), rail-callback HMAC verification with
   freshness + replay rejection (goal 9). The sandbox provider just
   ported is the test vehicle for all three.
2. **Port the CI/CD hardening** (multi-stage pipeline, Trivy, GHCR
   images, compose smoke test, deploy gate) as a dedicated reviewed PR.
3. **Port the web journeys** against the Rust contract, screen by screen,
   with their tests.

## Branch disposition

`remediation/amberpay-scope-and-security` is **archived, read-only**: kept
on the remote as the provenance record for the C-4/5/6 analysis and the
gap-analysis; no further work lands there, and it must never be merged
wholesale. It can be deleted once the follow-ups above are verified
against the Rust platform.
