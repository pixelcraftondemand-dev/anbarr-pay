# AMBER PAY — Database Schema Proposal

PostgreSQL 15+. Two schemas with separate DB roles (`amber_ledger` owns `ledger`; `amber_app` owns `app`). The app role has **no write access** to `ledger`.

> **Implementation note (core-api crate):** the Core API's first slice (wallet
> registry + vault goals, `core-api/migrations`) ships in its own crate and,
> in dev, its own database (`amber_core` — see docker/init-core-db.sql). That
> is a *stronger* enforcement of the same boundary this doc specifies via
> roles; as more `app` schema tables (users, sessions, KYC, …) are built out,
> they migrate into the role-separated `app` schema described below.

Conventions:
- `UUID` primary keys (`gen_random_uuid()`).
- All timestamps `TIMESTAMPTZ`, stored UTC.
- All monetary amounts `BIGINT` in **minor units** (2 dp for SLE and USD). Never floats.
- Status fields are `TEXT` with `CHECK` constraints (or Postgres enums) — deliberately simple; enums can be migrated later.
- Every mutation table has `created_at`; mutable tables have `updated_at`.

---

## 1. `ledger` schema (Rust-owned, write-only via gRPC)

### accounts — chart of accounts; wallets are owner-scoped accounts

```sql
CREATE TABLE ledger.accounts (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  owner_type   TEXT NOT NULL,          -- user | merchant | agent | platform
  owner_id     UUID,                   -- NULL for platform accounts
  type         TEXT NOT NULL,          -- wallet | float | hold_escrow | fee_revenue | platform_revenue | rail_bridge
  currency     CHAR(3) NOT NULL,       -- SLE | USD
  name         TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','frozen','closed')),
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (owner_type, owner_id, type, currency)
);
```

Platform accounts (one per currency): `hold_escrow`, `fee_revenue`, `platform_revenue`, plus one `rail_bridge` per (rail, currency).

> **Derived balances (confirmed):** `accounts` carries **no** balance columns. Balances are always computed from `entries`; the `accounts` row serves as identity and as the write-mutex (row lock), never as balance storage. The read model lives in `wallet_snapshots` below.

### wallet_snapshots — derived read model (never written by the transaction path)

```sql
CREATE TABLE ledger.wallet_snapshots (
  account_id      UUID PRIMARY KEY REFERENCES ledger.accounts(id),
  currency        CHAR(3) NOT NULL,
  available_minor BIGINT NOT NULL,   -- Σ posted wallet entries (unencumbered)
  held_minor      BIGINT NOT NULL,   -- Σ open holds against this wallet
  total_minor     BIGINT NOT NULL,   -- available + held
  computed_upto   TIMESTAMPTZ NOT NULL,  -- watermark: max entry created_at included
  version         BIGINT NOT NULL,       -- increments on every rebuild
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- Snapshot rows are **only ever populated by recomputation from `entries`** (after-commit refresh of touched accounts + nightly full rebuild + on-demand `AuditAccounts`). The transaction path never writes them.
- Reads (`GetAccount`) serve the snapshot; if stale or missing, recompute on demand.
- Semantics: `available = Σ (debits − credits)` on the wallet account; `held = Σ open holds` (hold journals still `held` — holds move funds into `hold_escrow`, so the wallet's own entries already exclude them); `total = available + held`.

### journals — one group of entries; the audit/ownership record

```sql
CREATE TABLE ledger.journals (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  type              TEXT NOT NULL,  -- hold | capture | release | p2p | topup | cash_in | cash_out |
                                    -- checkout | fee | commission | reversal | refund | adjustment
  status            TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','posted','void')),
  currency          CHAR(3) NOT NULL,
  amount_minor      BIGINT NOT NULL CHECK (amount_minor > 0),
  -- origin / ownership graph (Monime-style traceability)
  origin_user_id    UUID,
  origin_channel    TEXT,           -- api | web | mobile | agent | merchant_api | system
  origin_session_id TEXT,
  payment_code      TEXT,           -- merchant payment code when applicable
  reference         TEXT,           -- external rail reference once confirmed
  idempotency_scope TEXT NOT NULL,  -- requester id (user / merchant / agent / system)
  idempotency_key   TEXT NOT NULL,
  request_hash      TEXT,           -- durable idempotency payload hash (migration 0004):
                                    -- replay verifies the payload even after the cache row expires
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  posted_at         TIMESTAMPTZ,
  UNIQUE (idempotency_scope, idempotency_key)
);

CREATE INDEX idx_journals_origin    ON ledger.journals (origin_user_id, created_at DESC);
CREATE INDEX idx_journals_reference ON ledger.journals (reference) WHERE reference IS NOT NULL;
```

### entries — immutable double-entry legs

```sql
CREATE TABLE ledger.entries (
  id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  journal_id          UUID NOT NULL REFERENCES ledger.journals(id),
  account_id          UUID NOT NULL REFERENCES ledger.accounts(id),
  direction           TEXT NOT NULL CHECK (direction IN ('debit','credit')),
  amount_minor        BIGINT NOT NULL CHECK (amount_minor > 0),
  currency            CHAR(3) NOT NULL,
  created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_entries_journal  ON ledger.entries (journal_id);
CREATE INDEX idx_entries_account  ON ledger.entries (account_id, created_at DESC);

-- Invariants enforced by the engine and verified nightly by AuditAccounts:
--   Σ(debit)  = Σ(credit)  per journal
--   snapshot available/held/total == recomputation from entries per account
```

### holds — reservations, implemented as real posted entries (see architecture §5.2)

```sql
CREATE TABLE ledger.holds (
  id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  journal_id              UUID NOT NULL REFERENCES ledger.journals(id),   -- the hold journal
  account_id              UUID NOT NULL REFERENCES ledger.accounts(id),   -- wallet held
  hold_escrow_account_id  UUID NOT NULL REFERENCES ledger.accounts(id),
  amount_minor            BIGINT NOT NULL CHECK (amount_minor > 0),
  currency                CHAR(3) NOT NULL,
  status                  TEXT NOT NULL DEFAULT 'held' CHECK (status IN ('held','captured','released','expired')),
  expires_at              TIMESTAMPTZ NOT NULL,
  captured_at             TIMESTAMPTZ,
  released_at             TIMESTAMPTZ,
  created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_holds_open ON ledger.holds (status, expires_at) WHERE status = 'held';
```

### idempotency_keys — replay-safe request log

```sql
CREATE TABLE ledger.idempotency_keys (
  scope        TEXT NOT NULL,
  key          TEXT NOT NULL,
  request_hash TEXT NOT NULL,          -- compared on replay; mismatch => reject (409)
  status       TEXT NOT NULL,          -- in_progress | done
  response     JSONB,                  -- cached response returned on replay
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at   TIMESTAMPTZ,            -- set to now()+TTL on claim; expired rows are
                                       -- reclaimed or pruned; journals row is the durable backstop
  PRIMARY KEY (scope, key)
);
```

### ledger_events — outbox, written in the same transaction as its journal

```sql
CREATE TABLE ledger.ledger_events (
  id         BIGSERIAL PRIMARY KEY,
  journal_id UUID NOT NULL REFERENCES ledger.journals(id),
  event_type TEXT NOT NULL,           -- journal.posted | hold.captured | hold.released ...
  payload    JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  consumed_at TIMESTAMPTZ             -- set by the Java outbox consumer
);

CREATE INDEX idx_ledger_events_unconsumed ON ledger.ledger_events (created_at) WHERE consumed_at IS NULL;
```

---

## 2. `app` schema (Java-owned)

### users — individuals (incl. merchant/agent logins; merchants are single-login per brief)

```sql
CREATE TABLE app.users (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email             CITEXT UNIQUE,
  phone             VARCHAR(20) UNIQUE,          -- E.164, required for P2P/rails
  full_name         TEXT NOT NULL,
  password_hash     TEXT NOT NULL,               -- bcrypt/argon2
  pin_hash          TEXT,                        -- argon2; required before transacting
  kyc_status        TEXT NOT NULL DEFAULT 'unverified' CHECK (kyc_status IN ('unverified','pending','verified','rejected')),
  id_type           TEXT CHECK (id_type IN ('national_id','passport','driver_license','voter_card')),
  id_number         TEXT,
  id_verified_at    TIMESTAMPTZ,
  status            TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','closed')),
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### merchants, agents, admin roles

```sql
CREATE TABLE app.merchants (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id        UUID NOT NULL UNIQUE REFERENCES app.users(id),   -- single-login model
  legal_name     TEXT NOT NULL,
  api_key_hash   TEXT,                        -- for server-to-server API auth
  webhook_url    TEXT,
  webhook_secret TEXT,                        -- HMAC signing of webhook payloads
  status         TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','closed')),
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.agents (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id          UUID NOT NULL UNIQUE REFERENCES app.users(id),
  float_account_id UUID NOT NULL REFERENCES ledger.accounts(id),
  commission_bps   INTEGER NOT NULL DEFAULT 50,     -- platform cost, paid into float (0.5% default, configurable)
  status           TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','closed')),
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.user_roles (
  user_id  UUID NOT NULL REFERENCES app.users(id),
  role     TEXT NOT NULL CHECK (role IN ('admin','ops','support')),
  PRIMARY KEY (user_id, role)
);
```

### sessions & OTPs

```sql
CREATE TABLE app.refresh_tokens (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id    UUID NOT NULL REFERENCES app.users(id),
  token_hash TEXT NOT NULL,
  ip         INET,
  user_agent TEXT,
  expires_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.otp_codes (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id     UUID NOT NULL REFERENCES app.users(id),
  purpose     TEXT NOT NULL,               -- transfer_high_value | profile_change | password_reset
  code_hash   TEXT NOT NULL,               -- never store plaintext
  attempts    INT NOT NULL DEFAULT 0,
  expires_at  TIMESTAMPTZ NOT NULL,
  consumed_at TIMESTAMPTZ,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### KYC documents (files verified manually/stubbed for now)

```sql
CREATE TABLE app.kyc_documents (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id       UUID NOT NULL REFERENCES app.users(id),
  document_type TEXT NOT NULL,
  file_key      TEXT NOT NULL,             -- S3 object key
  status        TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
  reviewed_by   UUID REFERENCES app.users(id),
  reviewed_at   TIMESTAMPTZ,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Domain state machines (Java-owned; ledger truth lives in `ledger.journals`)

```sql
CREATE TABLE app.checkouts (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  merchant_id      UUID NOT NULL REFERENCES app.merchants(id),
  amount_minor     BIGINT NOT NULL CHECK (amount_minor > 0),
  currency         CHAR(3) NOT NULL,
  payment_method   TEXT NOT NULL CHECK (payment_method IN ('wallet','mobile_money','card','bank')),
  status           TEXT NOT NULL DEFAULT 'created'
                   CHECK (status IN ('created','requires_payment','processing','succeeded','failed','cancelled','expired')),
  buyer_email      CITEXT,
  buyer_phone      VARCHAR(20),
  payment_code     TEXT,                    -- merchant/PAY code
  hold_id          UUID REFERENCES ledger.holds(id),
  journal_id       UUID REFERENCES ledger.journals(id),
  idempotency_key  TEXT NOT NULL,
  metadata         JSONB,
  expires_at       TIMESTAMPTZ,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (merchant_id, idempotency_key)
);

CREATE TABLE app.transfers (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  sender_id        UUID NOT NULL REFERENCES app.users(id),
  recipient_id     UUID NOT NULL REFERENCES app.users(id),
  amount_minor     BIGINT NOT NULL CHECK (amount_minor > 0),
  currency         CHAR(3) NOT NULL,
  status           TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','posted','void')),
  otp_required     BOOLEAN NOT NULL DEFAULT false,
  otp_verified_at  TIMESTAMPTZ,
  journal_id       UUID REFERENCES ledger.journals(id),
  idempotency_key  TEXT NOT NULL,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.topups (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id         UUID NOT NULL REFERENCES app.users(id),
  rail            TEXT NOT NULL CHECK (rail IN ('orange_money','afrimoney')),
  destination_phone VARCHAR(20) NOT NULL,   -- mobile money number being topped up
  amount_minor    BIGINT NOT NULL CHECK (amount_minor > 0),
  currency        CHAR(3) NOT NULL,
  status          TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','posted','failed','void')),
  journal_id      UUID REFERENCES ledger.journals(id),
  idempotency_key TEXT NOT NULL,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.cash_movements (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  agent_id        UUID NOT NULL REFERENCES app.agents(id),
  direction       TEXT NOT NULL CHECK (direction IN ('cash_in','cash_out')),
  user_id         UUID NOT NULL REFERENCES app.users(id),   -- counterparty
  amount_minor    BIGINT NOT NULL CHECK (amount_minor > 0),
  currency        CHAR(3) NOT NULL,
  status          TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','posted','failed','void')),
  journal_id      UUID REFERENCES ledger.journals(id),
  idempotency_key TEXT NOT NULL,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Rails link & statements

```sql
CREATE TABLE app.rail_transactions (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  rail             TEXT NOT NULL,            -- orange_money | afrimoney | card | bank
  journal_id       UUID NOT NULL REFERENCES ledger.journals(id),
  rail_reference   TEXT,                     -- external id from the rail
  rail_status      TEXT,
  request_payload  JSONB,
  response_payload JSONB,
  amount_minor     BIGINT NOT NULL,
  currency         CHAR(3) NOT NULL,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (rail, rail_reference)
);
```

### Webhook deliveries (outbound to merchants)

```sql
CREATE TABLE app.webhook_deliveries (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  merchant_id    UUID NOT NULL REFERENCES app.merchants(id),
  event_type     TEXT NOT NULL,              -- checkout.succeeded | transfer.completed | topup.completed ...
  payload        JSONB NOT NULL,
  signature      TEXT NOT NULL,              -- HMAC-SHA256
  status         TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','delivered','failed','dead')),
  attempts       INT NOT NULL DEFAULT 0,
  next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_error     TEXT,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_webhook_pending ON app.webhook_deliveries (status, next_attempt_at) WHERE status = 'pending';
```

### Notifications

```sql
CREATE TABLE app.notifications (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id    UUID NOT NULL REFERENCES app.users(id),
  channel    TEXT NOT NULL CHECK (channel IN ('email','sms')),
  template   TEXT NOT NULL,
  params     JSONB,
  status     TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','sent','failed')),
  sent_at    TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Fraud, limits, reconciliation

```sql
CREATE TABLE app.velocity_limits (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  scope            TEXT NOT NULL,           -- user | merchant | agent | global
  key              TEXT NOT NULL,           -- daily_send_minor | daily_checkout_count | monthly_volume ...
  limit_value      BIGINT NOT NULL,
  window_seconds   INT NOT NULL,
  action           TEXT NOT NULL DEFAULT 'block' CHECK (action IN ('block','challenge_otp')),
  enabled          BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE app.fraud_rule_hits (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id    UUID REFERENCES app.users(id),
  rule       TEXT NOT NULL,
  decision   TEXT NOT NULL CHECK (decision IN ('allow','block','challenge')),
  data       JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.reconciliation_runs (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  rail          TEXT NOT NULL,
  period_start  TIMESTAMPTZ NOT NULL,
  period_end    TIMESTAMPTZ NOT NULL,
  status        TEXT NOT NULL DEFAULT 'running'
                CHECK (status IN ('running','completed','completed_with_errors')),
  counts        JSONB,   -- {matched, missing_internal, missing_external, amount_mismatch}
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at  TIMESTAMPTZ
);

CREATE TABLE app.reconciliation_items (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  run_id        UUID NOT NULL REFERENCES app.reconciliation_runs(id),
  internal_ref  TEXT,                  -- journal id / reference
  external_ref  TEXT,                  -- rail statement line ref
  amount_minor  BIGINT,
  currency      CHAR(3),
  match_status  TEXT NOT NULL CHECK (match_status IN ('matched','missing_internal','missing_external','amount_mismatch')),
  notes         TEXT,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_recon_items_run ON app.reconciliation_items (run_id);
```

### Fees & FX (fee model is percentage-only per brief; table ready for floors/segments)

```sql
CREATE TABLE app.fee_configs (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name              TEXT NOT NULL,
  percentage_bps    INTEGER NOT NULL CHECK (percentage_bps > 0),   -- 50 = 0.5%
  applies_to        TEXT NOT NULL,          -- checkout | p2p | topup | cash_in | cash_out | all
  charged_to        TEXT NOT NULL DEFAULT 'payer' CHECK (charged_to IN ('payer','merchant')),
                                            -- confirmed: platform fee charged to the paying party
  rail              TEXT,                   -- NULL = all rails
  min_amount_minor  BIGINT,                 -- future floor (NULL = none; pure % now)
  max_amount_minor  BIGINT,
  enabled           BOOLEAN NOT NULL DEFAULT true,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app.fx_rates (                 -- future multi-currency cross trades
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  base_currency  CHAR(3) NOT NULL,
  quote_currency CHAR(3) NOT NULL,
  rate           NUMERIC(18,8) NOT NULL,
  effective_from TIMESTAMPTZ NOT NULL,
  effective_to   TIMESTAMPTZ,
  source         TEXT
);
```

---

## 3. Key integrity notes

- **Ledger isolation**: `amber_app` cannot write to `ledger.*` — enforced by DB privileges, not convention.
- **Immutability**: `ledger.entries` has no UPDATE/DELETE path in the codebase; corrections are new journals (`reversal`/`adjustment`) referencing the original via `reference`.
- **Derived balances**: no stored balances anywhere. The engine validates by computing balances from `entries` under the account row lock (lock → compute → validate → post); the `wallet_snapshots` read model is rebuilt from `entries` only, and `AuditAccounts` verifies snapshot == recomputation nightly.
- **Deadlock prevention**: account rows locked in canonical (sorted) order.
- **Minor units**: a `CHECK (amount_minor > 0)` everywhere; no zero/negative amount legs; direction is a separate column so "negative debit" cannot occur.