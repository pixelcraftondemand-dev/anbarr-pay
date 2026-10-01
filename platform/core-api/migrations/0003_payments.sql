-- Roadmap goals 2–4: payments (transfers/topups), PIN authorization,
-- beneficiaries, and the app-layer audit log (docs/security-controls.md §4,
-- app half). Money truth stays in the ledger; these rows are the Core API's
-- record of *its own* actions and what the app displays.

-- Transaction PIN (docs/authentication.md §5): stored as salted PBKDF2
-- (code side), never plaintext. attempts/locked_until implement the §5
-- lockout ladder; pin_tokens are short-lived (2 min) one-time tokens.
ALTER TABLE users ADD COLUMN IF NOT EXISTS pin_hash TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS pin_locked_until TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS pin_attempts INT NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS pin_tokens (
    token_hash  TEXT PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_pin_tokens_user ON pin_tokens (user_id);

-- P2P transfers (docs/api.md §4). The ledger journal is the money truth;
-- the row records the Core API's action and powers GET /v1/transfers/{id}.
CREATE TABLE IF NOT EXISTS transfers (
    id                   UUID PRIMARY KEY,
    user_id              UUID NOT NULL REFERENCES users (id),
    wallet_account_id    UUID NOT NULL,
    recipient_phone      TEXT NOT NULL,
    recipient_name       TEXT NOT NULL DEFAULT '',
    recipient_account_id UUID,
    amount_minor         BIGINT NOT NULL,
    currency             TEXT NOT NULL,
    fee_minor            BIGINT NOT NULL DEFAULT 0,
    tax_minor            BIGINT NOT NULL DEFAULT 0,
    total_minor          BIGINT NOT NULL,
    journal_id           UUID,
    -- PENDING → COMPLETED | FAILED | REVERSED (docs/transaction-state-machine.md)
    status               TEXT NOT NULL DEFAULT 'PENDING',
    note                 TEXT NOT NULL DEFAULT '',
    idempotency_key      TEXT NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_transfers_idem ON transfers (user_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_transfers_user ON transfers (user_id, created_at DESC);

-- Mobile-money top-ups (docs/api.md §6). Wallet-funded; rails confirm
-- asynchronously, so the row honestly starts PENDING until the rail
-- (or in dev, the confirmation job) settles it.
CREATE TABLE IF NOT EXISTS topups (
    id                UUID PRIMARY KEY,
    user_id           UUID NOT NULL REFERENCES users (id),
    wallet_account_id UUID NOT NULL,
    rail              TEXT NOT NULL,
    destination_phone TEXT NOT NULL,
    amount_minor      BIGINT NOT NULL,
    currency          TEXT NOT NULL,
    rail_reference    TEXT,
    -- PENDING → COMPLETED | FAILED
    status            TEXT NOT NULL DEFAULT 'PENDING',
    idempotency_key   TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_topups_idem ON topups (user_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_topups_user ON topups (user_id, created_at DESC);

-- Beneficiaries (docs/api.md §6c): saved recipients. `source` records how
-- the entry appeared — derived from a paid transfer, or added manually
-- (a §7-sensitive action).
CREATE TABLE IF NOT EXISTS beneficiaries (
    id             UUID PRIMARY KEY,
    user_id        UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    email_or_phone TEXT NOT NULL,
    source         TEXT NOT NULL DEFAULT 'manual', -- manual | derived
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, email_or_phone)
);
CREATE INDEX IF NOT EXISTS idx_beneficiaries_user ON beneficiaries (user_id, created_at DESC);

-- App-layer audit log (docs/security-controls.md §4): who did what, from
-- where, with what outcome. Append-only by convention; retention per doc.
CREATE TABLE IF NOT EXISTS audit_log (
    id          BIGSERIAL PRIMARY KEY,
    user_id     UUID,
    event       TEXT NOT NULL,
    subject     TEXT NOT NULL DEFAULT '',
    outcome     TEXT NOT NULL DEFAULT 'success', -- success | failure
    ip          TEXT NOT NULL DEFAULT '',
    user_agent  TEXT NOT NULL DEFAULT '',
    details     JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_audit_user_time ON audit_log (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_event ON audit_log (event, created_at DESC);
