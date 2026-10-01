# AGENTS.md — Operating Contract for AI Contributors

Every AI agent working in this repository — coding agent, review agent, or
one-off helper — is bound by this contract **before its first edit**, and by
`docs/engineering-standards.md` for everything it produces. Humans working
with agents hold them to the same standard; nothing here exempts a human.

## 0. The prime directive

This is a **payments codebase** (AMBER PAY — Sierra Leone). When any
instruction conflicts with the safety of money paths or user data, safety
wins and you say so out loud. A silent exception is the only unforgivable
failure mode.

## 1. Before you write code

1. **Read the ground truth docs** for the area you touch:
   - Money/invariants: `docs/architecture.md` §5, `docs/engineering-standards.md` §1
   - API surface: `docs/api.md` + `web/src/api/client.ts` (they must agree)
   - Auth/session: `docs/authentication.md`
   - Current goals + status: `docs/roadmap.md`
   If your change disagrees with a doc, either the doc is stale (propose the
   doc PR) or your change is wrong (say which, in the PR body).
2. **Check the roadmap.** Work that isn't a roadmap goal or an incident fix
   needs a stated reason in the PR body. This repo does not accumulate
   speculative features.
3. **Run the gates first** so you know what "green" looks like:
   ```bash
   docker compose up -d          # dev Postgres (integration tests need it)
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test                    # unit + property + integration
   ```
   All four must pass before you claim anything works. No DB? Say so —
   never report untested code as done.

## 2. While you write code

4. **Scope discipline.** Change only what the task needs. No drive-by
   refactors, no reformatting of untouched lines, no "while I'm here".
   Spot something dangerous? File it as a finding/comment, don't fix it in
   a mixed commit.
5. **Money rules are absolute** (engineering-standards §1): integers only,
   ledger before rows, idempotency keys everywhere money moves, extractors
   for identity, ownership checks on every path param, fail closed.
6. **Tests are part of "done".** Money-path PRs carry the §2 mandatory
   tests (idempotency, conflict, failure-path, authorization, audit) or the
   PR says exactly which existing test covers them. Prefer naming bindings
   over `_` discards when destructuring — tuple-order bugs are silent 401s.
7. **Honesty about incompleteness.** Stubs stay marked (the codebase has an
   established style: comments that name the missing piece and the doc that
   specifies it). Never let a UI show success for an operation the backend
   didn't complete. Never mark a todo done that isn't verified.

## 3. Git rules for agents

8. **Branches, not `main`.** Work happens on a feature branch
   (`<type>/<goal>-<slug>`, e.g. `feat/003-p2p-transfers`) that PRs into
   **`staging`** — the pre-AI-review integration branch. Only staging PRs
   into `main` (release), with the human review for money/auth/infra
   changes. Full pipeline: `docs/engineering-standards.md` §4.
9. **Commits** follow engineering-standards §3: Conventional Commits,
   imperative subject ≤ 72 chars, body explains why, roadmap goal referenced.
   No secrets, no generated artifacts, no unrelated churn, no force-push to
   shared branches.
10. **You do not** `git push` to remotes, close/reopen PRs, or modify CI
    files unless the task explicitly says so. Destructive commands
    (`git reset --hard`, `docker rm`, dropping test DBs outside the
    per-test harness) require the user's explicit go-ahead, even when a
    fix seems obvious.

## 4. Review rules for agents (the reviewer hat)

11. **No self-approval.** The reviewing agent must not be the authoring
    session. Approval requires the gates run with output shown
    (engineering-standards §4).
12. **Findings in severity format** (Blocker / Should / Nit), each with
    evidence — a command output, a doc quote, or a specific invariant. "I
    feel this could be cleaner" is not a finding. The verbatim review
    prompt: `docs/ai-review-prompt.md`.
13. **Money-path review = run the checklist:** ordering, idempotency,
    ownership, error contract, mandatory tests present, blast radius on
    `web/src/api/client.ts` and `docs/api.md`. If you can't run the DB,
    downgrade to `Comment` and say why.

## 5. Escalation triggers — stop and ask the human

Stop and get explicit human input before proceeding when any of these
appears:

- **Money semantics change**: fee bps, tax, reversal/refund behavior,
  idempotency semantics, or a new money-moving endpoint.
- **Schema changes** beyond an additive, forward-only migration.
- **Auth/session behavior**: token formats, TTLs, lockout, reuse detection.
- **A doc and the code disagree** and you cannot tell which is right.
- **You need a third-party service** (provider, hosting, email, payments):
  use structured research first, present options, wait for a decision.
- **The same test fails twice** for reasons you can't explain — that's a
  flaky harness or a real bug, not something to retry away.
- **Anything that looks like production data, real secrets, or a live
  rail** — stop, report, touch nothing.

## 6. Definition of done (for any task)

- Gates green locally, with output (fmt, clippy, tests).
- Tests added or updated per §2 of engineering-standards.
- Docs touched if behavior/contract changed (`docs/api.md`, roadmap status).
- PR body: what, why, what's *not* done, how to verify.
- Findings answered — changed or rebutted, never ignored.
