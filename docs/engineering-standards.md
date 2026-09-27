# AMBER PAY — Engineering Standards

Binding for **every contributor, human or AI agent**. This is a payments
codebase: the cost of a sloppy change is measured in money and trust, not
reverted commits. The rules below encode what the repo already enforces —
the invariants in `docs/architecture.md`, the gates in `README.md` /
`.github/workflows/ci.yml`, and the test mandates in
`docs/testing-strategy.md` §3/§7 — plus the collaboration rules for AI
contributors (see `AGENTS.md` for the operating contract).

---

## 1. Maintainable code rules

**Money rules (non-negotiable):**

1. **Integers only.** All amounts are `i64` minor units. Floats are banned
   anywhere near amounts; fees/taxes use the ledger's banker's rounding.
2. **Money moves first, rows second.** A handler posts to the ledger
   (`PostTransfer`/`HoldFunds`/…) *before* persisting metadata. A crash must
   never leave a phantom row that claims money moved.
3. **Idempotency everywhere.** Every money-moving endpoint requires a client
   `Idempotency-Key`. Same key + same request → replay the stored response
   (200). Same key + different request → `409 Conflict`, never a silent
   replay of the wrong payment. Ledger-side keys derive from the caller's
   key so crash-recovery retries replay the journal.
4. **One wire contract.** Paths, status codes, and bodies follow
   `docs/api.md` and match `web/src/api/client.ts` exactly. If a route moves,
   the client and the docs move in the same PR.
5. **Fail closed, leak nothing.** problem+json errors via `ApiError` only;
   no SQL/ledger internals in messages; no enumeration (unknown phone and
   bad code are indistinguishable); every sign-in/PIN failure is deliberately
   vague. New failure modes get a variant or a safe message — never `unwrap()`
   on request paths.
6. **Identity through extractors.** Handlers take `Caller` (session) and
   `PinAuth` (one-time step-up) — never raw headers. Money movement requires
   both, with ownership checks on every resource id from the URL.
7. **One-time tokens stay one-time.** pin_tokens and OTPs are consumed
   atomically (`UPDATE … WHERE consumed_at IS NULL`) and stored hashed.
   Secrets never appear in logs or audit details.

**Code health (enforced by CI, so these are also review rules):**

8. `cargo fmt` + `cargo clippy --all-targets -- -D warnings` + `cargo test`
   are green before every push. A red local run is a blocked push, not a
   "CI will catch it".
9. No new dependency without a stated reason in the PR body (what it does,
   what it replaces, license). Prefer std + existing workspace crates.
10. Complex `sqlx::query_as` tuples get a named `type` alias with a doc
    comment listing the fields in order (clippy `type_complexity` will force
    this anyway; the alias is for humans).
11. Migrations are append-only SQL files (`core-api/migrations/`,
    `ledger/migrations/`), forward-only, each with a header comment saying
    which roadmap goal and doc section it implements.
12. Doc comments explain *why* (which doc section, which invariant), not
    *what*. Code that needed a debugging session to understand must get the
    comment the next person will need.

---

## 2. Tests that actually matter

A test earns its place if it can fail for a reason a user would feel. We do
not collect coverage for its own sake (the coverage floors in CI are a
floor, not the goal).

**Mandatory for every money-path PR** (`docs/testing-strategy.md` §7):

- **Idempotency test** — same request twice ⇒ ONE financial transaction and
  one row; replay returns the first response.
- **Conflict test** — same key, different body ⇒ 409, nothing extra moved.
- **Failure-path test** — the ledger refusing (insufficient funds ⇒ 422, no
  row persisted) and auth refusing (no/unknown/spent pin_token ⇒ 401).
- **Authorization test** — a foreign authenticated caller cannot read or act
  on another user's resource (404, not 403-with-confirmation).
- **Audit assertion** — the expected `audit_log` event exists with the right
  outcome when the action has compliance weight.

**Integration over mock.** The core-api harness runs the real ledger gRPC
server in-process and a real per-test Postgres — no mocks on the money path.
A mock that "proves" a transfer works proves nothing about double-entry.

**Harness rules (each of these has bitten us once):**

13. Each test passes a unique `tag` to `harness()`; parallel tests get
    isolated DBs by construction. Never share phones, keys, or wallets
    across tests.
14. `common::sign_in` returns **(access_token, refresh_token)** — stated here
    because tuple order bugs are silent until a 401. Check the order at
    every call site; prefer naming over `_` discard.
15. `users.phone` is normalized E.164 (`+232…`). Tests that query by phone
    must bind the normalized form (mirror `tests/auth.rs`), never the
    `076…` input form.
16. Test names state the behavior: `transfer_same_key_different_body_is_conflict`,
    not `test_transfers_2`. The name is the first documentation a reviewer reads.

**What we don't write:** snapshot tests of JSON, tests that restate the
implementation line-by-line, exhaustively-parameterized validation tables for
messages the contract doesn't promise, UI tests that assert pixels.

---

## 3. Git commit guideline

Format (Conventional Commits):

```
<type>(<scope>): <imperative summary, ≤ 72 chars>

<why this change exists — 1–3 sentences. Reference the doc section or
roadmap goal the change implements.>

<footer: refs, breaking notes>
```

- **Types:** `feat`, `fix`, `test`, `docs`, `refactor`, `chore`, `perf`.
- **Scopes:** `ledger`, `core-api`, `web`, `infra`, `docs`.
- **Subject:** imperative ("add PIN step-up to transfers", not "added" /
  "fixes"). No dangling "update" or "misc changes" — if the subject can't be
  specific, the commit is too big; split it.
- **Body:** the *why*, and any decision a reviewer would otherwise have to
  reconstruct (e.g. "replay check runs before PIN so retries don't need a
  fresh token — docs/api.md §2").
- **Roadmap linkage:** commits implementing a goal reference it
  (`roadmap goal 3`).
- **Never in a commit:** secrets or env values (gitleaks scans full history
  and it's forever), generated artifacts, unrelated file churn, formatting-
  only noise mixed with logic (separate `style:`/`refactor:` commits).
- **History discipline:** no force-push to shared branches; `main` receives
  PRs only (branch protection). Rebases are fine on your own branch.

---

## 4. Code review process (AI-first)

The default reviewer is an AI agent; humans arbitrate. Branch protection
still requires one approving review on every PR — the process below is how
that review is performed so approval means the same thing regardless of who
(or what) performs it.

### Branch pipeline

```
feat/<goal>-<slug>  →  staging  →  main
     (work)            (AI review)  (release)
```

- **`staging`** — the integration branch. All feature branches PR into it.
  CI runs on every push; the **AI review (§4) happens against staging PRs**.
  Staging must always build and pass tests — it is the review surface, not a
  dumping ground.
- **`main`** — the release branch. Receives PRs **from staging only**,
  requires the human review for financial/auth/infra changes, and stays
  deployable at all times. Direct pushes are blocked by branch protection.
- Hotfix exception (see below): the incident fix may PR straight to `main`,
  then back-merge to staging.

### Roles

- **Author** (human or AI): opens the PR, self-reviews against §5's
  checklist before requesting review, and answers every finding with either
  a change or a reasoned rebuttal — never silence.
- **AI reviewer** (a *different* agent/session than the author — no
  self-approval): runs the mechanical gates itself (it has shell access, so
  "CI will catch it" is not a review), reads the diff against §5, and
  writes findings in the format below.
- **Human reviewer**: required additionally for *financial-feature* PRs and
  anything touching `ledger/src/engine.rs`, auth/session logic, migrations,
  or CI files (`docs/testing-strategy.md` §7's second-review rule). The
  human reviews the AI review too — findings skipped without rebuttal
  invalidate the approval.

### What an AI reviewer must actually do

1. **Run, don't assume:** `cargo fmt --check`, `cargo clippy --all-targets
   -- -D warnings`, `cargo test` (integration tests need
   `docker compose up -d`). Paste or summarize real output; an approval
   without command output is void.
2. **Read the money path first:** for any diff touching money, verify §1
   rules 1–7 hold (ordering, idempotency, extractor use, ownership checks,
   error contract) and that the PR contains the §2 mandatory tests. A
   financial PR without an idempotency test is an automatic Request Changes.
3. **Check the blast radius:** new routes match `docs/api.md` +
   `web/src/api/client.ts`; migrations are append-only; no secret-bearing
   values; no new deps without justification.
4. **Verify honesty:** the PR description states what is *not* done (stubs
   marked, env vars required, docs that will lag). Hidden incompleteness is
   the one sin that can't be refactored later.

### Finding format

Every finding is one of three severities, stated per comment:

- **Blocker** — breaks an invariant in §1/§2 or fails a gate. PR cannot
  merge until changed or the reviewer is shown wrong with evidence.
- **Should** — maintainability debt that is cheap to fix now and expensive
  later. Merge requires either the fix or a written follow-up issue.
- **Nit** — taste. Author's call; never blocks.

Review verdicts: `Approve`, `Request Changes`, or `Comment`. An AI reviewer
that cannot run the gates (no DB, no network) must say so explicitly and
downgrade to `Comment` — a conditional approval is not an approval.

### Emergency path

Hotfixes to a production incident may land with human review only
(`fix:` + `incident:` footer), followed within 24h by the normal PR that
reconciles docs and tests. The exception is for the incident, not from the
process.
