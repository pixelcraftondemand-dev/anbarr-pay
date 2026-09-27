# Amber Pay — Ledger Service (Rust)

The double-entry ledger engine for **AMBER PAY** (Sierra Leone; SLE primary, USD
secondary). This crate (`anbarr-ledger`) is the correctness-critical money path:
journal posting, holds, idempotency, derived balances, fee math, and rail
reconciliation. Only this service ever writes to the ledger tables — enforced
with separate database roles, not convention.

> **Conventions (PixelCraft Engineering Standards):** every Rust change must
> pass `cargo fmt --check` and `cargo clippy -- -D warnings`; correctness
> logic targets **85%+ coverage**; every PR must pass CI and get one review
> before merging to `main` (branch protection is configured in GitHub Settings
> — see [Branch protection](#branch-protection)). The binding rules —
> maintainable-code invariants, which tests matter, commit format, and the
> AI-first review process — live in
> [`docs/engineering-standards.md`](docs/engineering-standards.md); AI agents
> are additionally bound by [`AGENTS.md`](AGENTS.md).

---

## Repository layout

```
Cargo.toml                workspace root
ledger/                   the anbarr-ledger crate
  src/engine.rs           posting engine: journals, holds, reversals/refunds, idempotency, funds rule
  src/balances.rs         derived balances (available / held / total), snapshots, audit
  src/money.rs            integer minor units, banker's-rounding fee math
  src/reconcile.rs        pure rail reconciliation + scheduler + alert sinks
  src/types.rs            domain types (sum types, no illegal states)
  migrations/             sqlx migrations (schema is version-controlled)
  tests/                  integration tests (real Postgres) + property tests
core-api/                 the anbarr-core-api crate — app-domain REST boundary
  src/vault.rs            Vault goal savings (docs/vault.md): metadata + ledger holds via gRPC
  src/wallets.rs          wallet registry (caller → verified ledger account)
  migrations/             core metadata schema (wallet_links, vault_goals, vault_goal_locks)
  tests/                  integration tests: in-process ledger gRPC + real Postgres
web/                      React (TypeScript, Vite) SPA — the thin view layer
docs/                     architecture, API surface, and schema proposals
.github/workflows/        CI + nightly security pipelines
```

Design and API proposals live in [`docs/architecture.md`](docs/architecture.md),
[`docs/api.md`](docs/api.md), and [`docs/schema.md`](docs/schema.md). The
full set of AmberPay checklist deliverables (threat model, state machine,
authentication, RBAC, security controls, fraud, KYC/AML, providers/webhooks,
DR, testing, UX/click-by-click, production-readiness gate) is indexed in
[`docs/README.md`](docs/README.md). The frontend is a **React (TypeScript,
Vite) SPA** — see `docs/architecture.md` "Frontend client" and
`docs/ux-flows.md`.

## Prerequisites

- Rust (stable toolchain)
- Docker (for the local dev Postgres used by integration tests)

## Quick start

```bash
# 1. Start the dev database (postgres:16, host port 5433)
docker compose up -d

# 2. Run the whole test suite (unit, property, and integration tests)
cargo test
```

The tests use `postgres://amber:amber_dev@localhost:5433/amber` by default;
override with `DATABASE_URL` (see below). Each test binary runs in its own
Postgres **schema** (`test_<binary>` via `search_path`), so the parallel
binaries never step on each other's shared singleton accounts
(`ledger/tests/common/mod.rs`). The core-api tests additionally use the
`amber_core_test` database (created on the fly) for app metadata — the
boundary under test.

### Core API (vault endpoints)

The Core API is the app-domain service (docs/architecture.md): it owns
metadata (wallet registry, vault goals) and drives money movement **only**
through the ledger's gRPC contract — it never writes ledger tables.

```bash
# 0. One-time: the core metadata database (init script also does this)
docker exec amber-postgres psql -U amber -c 'CREATE DATABASE amber_core'

# 1. Start the ledger gRPC server (terminal 1)
cargo run -p anbarr-ledger

# 2. Start the Core API (terminal 2) — REST on :8080
cargo run -p anbarr-core-api
```

Endpoints (contract in `docs/api.md` §6b/§6c and `docs/vault.md`):

- `POST /v1/wallets` — link the caller to a ledger account (verified via
  `GetAccount` before storing); `GET /v1/wallets` — live balances
- `POST /v1/vault/goals` — create a goal + first lock (`HoldFunds`)
- `POST /v1/vault/goals/{id}/locks` — add a lock
- `POST /v1/vault/goals/{id}/release` — unlock (whole holds, `ReleaseHold`)

Until sign-in ships, the gateway (or a test) names the caller with the
`x-amber-caller` header; this is a development configuration — do not deploy
publicly.

## Web client (React)

```bash
cd web
npm install
npm run dev        # dev server on :5173, proxies /v1 to the Core API
npm run build      # production bundle (dist/)
```

The SPA is a thin view layer only (no financial logic, no secrets) — see
`docs/architecture.md` "Frontend client" and `docs/ux-flows.md`.

## Environment variables

| Variable      | Required | Default (dev)                                                        | Description                                  |
| ------------- | -------- | -------------------------------------------------------------------- | -------------------------------------------- |
| `DATABASE_URL`| dev-only | `postgres://amber:amber_dev@localhost:5433/amber?sslmode=disable`    | Postgres connection string (migrations + tests). In CI this points at the disposable `postgres:16` service container. |
| `CORE_DATABASE_URL` | dev-only | `postgres://amber:amber_dev@localhost:5433/amber_core?sslmode=disable` | Core API metadata database (wallet links, vault goals). |
| `LEDGER_ENDPOINT` | no | `http://127.0.0.1:50051` | Ledger gRPC address the Core API calls. |
| `LEDGER_GRPC_TOKEN` | deploy | `amberpay-internal-dev` | Shared secret presented as `x-ledger-token`. Real value from the secrets manager. |
| `CORE_API_ADDR` | no | `127.0.0.1:8080` | Core API REST listen address. |

Placeholders only — no real credentials are ever committed. Production
credentials arrive via the deployment platform (AWS Secrets Manager / SSM) when
the service deploys. Future env vars (gRPC bind address, log level, rail
credentials) will be documented here as the gRPC server lands.

## Running tests and coverage locally

```bash
cargo fmt --check                      # formatting gate (CI + pre-commit)
cargo clippy --all-targets -- -D warnings   # lint gate, warnings are errors
cargo test --lib --test properties     # unit + property tests (no DB needed)
cargo test --tests                     # integration tests (needs: docker compose up -d)
cargo test                             # everything

# Coverage (PixelCraft bar: 85%+ on ledger/transaction/balance logic):
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo llvm-cov --all-targets --lcov --output-path lcov.info
python3 .github/scripts/check_core_coverage.py lcov.info
```

The core-module bar is enforced in CI for `engine.rs`, `balances.rs`,
`money.rs`, and `reconcile.rs` (85%+, per the standards); the whole crate has a
global 80% floor. Add tests for new code going forward rather than backfilling
retroactively.

## Pre-commit hooks

```bash
pip install pre-commit
pre-commit install
```

Hooks run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
and the quick unit/property test suite before every commit. Integration tests
need a database, so they run in CI instead of the hook.

## Branch model

```
feat/<goal>-<slug>  →  staging  →  main
     (work)            (AI review)  (release)
```

- **`staging`** — integration branch: feature branches PR here, CI runs on
  every push, and the **AI code review happens on staging PRs**.
- **`main`** — release branch: receives PRs from staging only, requires
  human review for financial/auth/infra changes, always deployable.
- Direct pushes to either are blocked (branch protection, GitHub Settings).

Details: `docs/engineering-standards.md` §4.

## CI/CD

GitHub Actions (`.github/workflows/ci.yml`), staged to fail fast:

1. **fmt check** → 2. **clippy** (warnings as errors) → 3. **cargo check**
   (type check) → 4. **unit + property tests** → 5. **integration tests**
   against a disposable `postgres:16` service container (the Rust equivalent
   of Testcontainers) → 6. **release build** → 7. **coverage** (core modules
   ≥ 85%, whole crate ≥ 80%) → 8. **cargo audit** (fails on High/Critical
   CVEs, per `.cargo/audit.toml`) → 9. **gitleaks** full-history secret scan.

Cargo dependencies are cached between runs to keep the pipeline fast.

`.github/workflows/security-nightly.yml` re-runs the audit and secret scans
daily, so new advisories or leaked secrets surface even with no code changes.

### Branch protection

Enabled in the GitHub repository settings (not enforceable from this repo):
- No direct pushes to `main`; all changes land via pull request.
- Required status checks: `lint`, `test`, `build`, `coverage`, `security`.
- At least **1 approving review** required before merge.

## Security

- **Dependency audit:** `cargo audit` on every PR and nightly; the build fails
  on **High/Critical** CVEs (lower severities are warnings).
- **Secret scanning:** `gitleaks` scans full git history on every push and
  nightly. `.env` files are gitignored; never commit real credentials.

## Architecture note — the double-entry model

- **Immutable entries, derived balances.** No stored balances anywhere:
  `available`/`held`/`total` are summed from immutable `entries` (and open
  `holds`) under account row locks. `wallet_snapshots` is a read-model rebuilt
  from the same source, and a nightly audit job verifies
  snapshot == recomputation (drift = alert).
- **Journals balance by construction.** Every journal has ≥ 2 legs with equal
  debit/credit totals (`validate_spec`), and the engine rejects any posting
  that would drive an account negative (`check_funds`) — both pure functions,
  pinned by property tests in `ledger/tests/properties.rs`. The only explicit
  overdraft is the `rail_bridge` clearing account, monitored by
  reconciliation instead.
- **Single writer, pessimistic locks.** The engine locks `accounts` rows in
  canonical (sorted) order inside one transaction per journal, so concurrent
  operations serialize and deadlocks are structurally impossible.
- **Idempotency everywhere.** Unique `(scope, idempotency_key)` on `journals`
  is the hard backstop; the engine replays the stored response for retries.
- **Holds are real entries.** A hold posts a journal (wallet → escrow);
  capture/release/expiry post the reverse, so the audit trail sees everything.
- **Atomic outbox.** Each journal writes its `ledger_events` row in the same
  transaction, giving downstream consumers exactly-once delivery.
- **All amounts are integers** (`BIGINT` minor units); floats are banned. Fees
  round with banker's rounding (`round_fee`).

See `docs/architecture.md` for the full proposal (service topology, data
ownership, holds lifecycle, reconciliation, design flags).