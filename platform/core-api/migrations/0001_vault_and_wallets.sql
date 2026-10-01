-- Core API metadata (app domain). No ledger money tables here: goals and
-- locks reference ledger hold ids, but the holds themselves live in the
-- ledger's own database and are created only through its gRPC contract
-- (docs/vault.md §1).

-- Caller → wallet registry (docs/vault.md §4). Until sign-in ships, the
-- gateway names the caller with `x-amber-caller`; the registry maps that
-- caller to a ledger account that has been *verified against the ledger*
-- (GetAccount) before the link is stored.
CREATE TABLE IF NOT EXISTS wallet_links (
    id              UUID PRIMARY KEY,
    caller          TEXT NOT NULL,
    account_id      UUID NOT NULL,
    currency        TEXT NOT NULL,
    label           TEXT NOT NULL DEFAULT '',
    is_primary      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- One primary wallet per caller; repeated links to the same account
    -- update rather than duplicate.
    UNIQUE (caller, account_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS one_primary_wallet_per_caller
    ON wallet_links (caller) WHERE is_primary;

-- Vault goals (docs/vault.md §1): display metadata + status. `locked_minor`
-- is a *derived* read-model of the open holds' amounts (maintained in the
-- same transaction that records lock/release outcomes), never an authority —
-- the ledger's holds are the truth.
CREATE TABLE IF NOT EXISTS vault_goals (
    id              UUID PRIMARY KEY,
    caller          TEXT NOT NULL,
    name            TEXT NOT NULL,
    currency        TEXT NOT NULL,
    target_minor    BIGINT,
    maturity_at     TIMESTAMPTZ,
    status          TEXT NOT NULL DEFAULT 'active', -- active|released
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_vault_goals_caller ON vault_goals (caller, created_at DESC);

-- Goal ↔ ledger hold association. One row per HoldFunds call; released_at
-- is set when ReleaseHold succeeds for that hold.
CREATE TABLE IF NOT EXISTS vault_goal_locks (
    id              UUID PRIMARY KEY,
    goal_id         UUID NOT NULL REFERENCES vault_goals (id) ON DELETE CASCADE,
    hold_id         UUID NOT NULL,
    amount_minor    BIGINT NOT NULL,
    locked_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at     TIMESTAMPTZ,
    UNIQUE (hold_id)
);
CREATE INDEX IF NOT EXISTS idx_vault_goal_locks_goal ON vault_goal_locks (goal_id);
