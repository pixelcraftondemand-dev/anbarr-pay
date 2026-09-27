-- ANBARR PAY — ledger schema (owned by the Rust Ledger Service)
-- All monetary amounts are BIGINT minor units (2 dp). Never floats.

CREATE TABLE accounts (
  id           UUID PRIMARY KEY,
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

-- Derived read model: only ever populated by recomputation from entries/holds.
-- The transaction path never writes these; AuditAccounts verifies them.
CREATE TABLE wallet_snapshots (
  account_id      UUID PRIMARY KEY REFERENCES accounts(id),
  currency        CHAR(3) NOT NULL,
  available_minor BIGINT NOT NULL,
  held_minor      BIGINT NOT NULL,
  total_minor     BIGINT NOT NULL,
  computed_upto   TIMESTAMPTZ NOT NULL,
  version         BIGINT NOT NULL,
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE journals (
  id                UUID PRIMARY KEY,
  type              TEXT NOT NULL,  -- hold | capture | release | p2p | topup | cash_in | cash_out | checkout | fee | reversal | adjustment
  status            TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','posted','void')),
  currency          CHAR(3) NOT NULL,
  amount_minor      BIGINT NOT NULL CHECK (amount_minor > 0),
  -- origin / ownership graph
  origin_user_id    UUID,
  origin_channel    TEXT,
  origin_session_id TEXT,
  payment_code      TEXT,
  reference         TEXT,
  idempotency_scope TEXT NOT NULL,
  idempotency_key   TEXT NOT NULL,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  posted_at         TIMESTAMPTZ,
  UNIQUE (idempotency_scope, idempotency_key)
);

CREATE INDEX idx_journals_origin    ON journals (origin_user_id, created_at DESC);
CREATE INDEX idx_journals_reference ON journals (reference) WHERE reference IS NOT NULL;

CREATE TABLE entries (
  id           UUID PRIMARY KEY,
  journal_id   UUID NOT NULL REFERENCES journals(id),
  account_id   UUID NOT NULL REFERENCES accounts(id),
  direction    TEXT NOT NULL CHECK (direction IN ('debit','credit')),
  amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
  currency     CHAR(3) NOT NULL,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_entries_journal ON entries (journal_id);
CREATE INDEX idx_entries_account ON entries (account_id, created_at DESC);

CREATE TABLE holds (
  id                      UUID PRIMARY KEY,
  journal_id              UUID NOT NULL REFERENCES journals(id),
  account_id              UUID NOT NULL REFERENCES accounts(id),
  hold_escrow_account_id  UUID NOT NULL REFERENCES accounts(id),
  amount_minor            BIGINT NOT NULL CHECK (amount_minor > 0),
  currency                CHAR(3) NOT NULL,
  status                  TEXT NOT NULL DEFAULT 'held' CHECK (status IN ('held','captured','released','expired')),
  expires_at              TIMESTAMPTZ NOT NULL,
  captured_at             TIMESTAMPTZ,
  released_at             TIMESTAMPTZ,
  created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_holds_open ON holds (status, expires_at) WHERE status = 'held';

CREATE TABLE idempotency_keys (
  scope        TEXT NOT NULL,
  key          TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  status       TEXT NOT NULL CHECK (status IN ('in_progress','done')),
  response     JSONB,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at   TIMESTAMPTZ,
  PRIMARY KEY (scope, key)
);

-- Outbox: written in the same transaction as its journal; consumed by the Java core service.
CREATE TABLE ledger_events (
  id          BIGSERIAL PRIMARY KEY,
  journal_id  UUID NOT NULL REFERENCES journals(id),
  event_type  TEXT NOT NULL,
  payload     JSONB NOT NULL,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  consumed_at TIMESTAMPTZ
);

CREATE INDEX idx_ledger_events_unconsumed ON ledger_events (created_at) WHERE consumed_at IS NULL;