//! Derived balances. There are no stored balances anywhere: available/held/total
//! are always computed from `entries` (and open `holds`). `wallet_snapshots` is a
//! read-model rebuilt from the same source; the transaction path never writes it
//! directly, only `rebuild_snapshot` does, and it derives strictly from entries/holds.

use crate::engine::EngineError;
use crate::money::{Currency, MinorUnits};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

/// A wallet's balance, derived from the ledger.
#[derive(Debug, Clone)]
pub struct Balance {
    pub account_id: Uuid,
    pub currency: Currency,
    /// Sum of the account's own posted entries (holds already moved funds into
    /// hold_escrow, so this is the unencumbered amount).
    pub available_minor: MinorUnits,
    /// Sum of open holds against this wallet.
    pub held_minor: MinorUnits,
    /// available + held.
    pub total_minor: MinorUnits,
}

/// Compute `balance = Σ entries` for the given accounts. Must be called under the
/// account row locks (single-writer discipline) to be race-free.
pub async fn available_for(
    tx: &mut Transaction<'_, Postgres>,
    account_ids: &[Uuid],
) -> Result<HashMap<Uuid, MinorUnits>, EngineError> {
    if account_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, i64)>(
        // Credit-positive convention: every account type we use (wallets are
        // customer liabilities, hold_escrow is a liability, fee revenue is
        // revenue) grows on credit and shrinks on debit.
        "SELECT account_id,
                COALESCE(SUM(CASE WHEN direction = 'credit' THEN amount_minor ELSE -amount_minor END), 0)::bigint AS balance
         FROM entries
         WHERE account_id = ANY($1::uuid[])
         GROUP BY account_id",
    )
    .bind(account_ids)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Open (held) hold amounts per wallet.
pub async fn held_for(
    tx: &mut Transaction<'_, Postgres>,
    account_ids: &[Uuid],
) -> Result<HashMap<Uuid, MinorUnits>, EngineError> {
    if account_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, i64)>(
        "SELECT account_id, COALESCE(SUM(amount_minor), 0)::bigint
         FROM holds
         WHERE account_id = ANY($1::uuid[]) AND status = 'held'
         GROUP BY account_id",
    )
    .bind(account_ids)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Full derived balance for one account (read path; single query, no locks needed).
pub async fn get_balance(pool: &PgPool, account_id: Uuid) -> Result<Balance, EngineError> {
    let account = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, currency, status FROM accounts WHERE id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?
    .ok_or(EngineError::AccountNotFound(account_id))?;

    let available = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(CASE WHEN direction = 'credit' THEN amount_minor ELSE -amount_minor END), 0)::bigint
         FROM entries WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;

    let held = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(amount_minor), 0)::bigint FROM holds WHERE account_id = $1 AND status = 'held'",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;

    Ok(Balance {
        account_id,
        currency: crate::money::Currency::parse(&account.1).expect("valid currency from DB"),
        available_minor: available,
        held_minor: held,
        total_minor: available + held,
    })
}

/// Rebuild the `wallet_snapshots` row for one account, deriving strictly from
/// entries and open holds. Called inside the posting transaction after commit-time
/// writes; also used by the nightly audit job for a full recompute.
pub async fn rebuild_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
) -> Result<(), EngineError> {
    sqlx::query(
        "INSERT INTO wallet_snapshots (account_id, currency, available_minor, held_minor, total_minor, computed_upto, version, updated_at)
         SELECT a.id,
                a.currency,
                COALESCE((SELECT SUM(CASE WHEN e.direction = 'credit' THEN e.amount_minor ELSE -e.amount_minor END)
                          FROM entries e WHERE e.account_id = a.id), 0),
                COALESCE((SELECT SUM(h.amount_minor) FROM holds h WHERE h.account_id = a.id AND h.status = 'held'), 0),
                COALESCE((SELECT SUM(CASE WHEN e.direction = 'credit' THEN e.amount_minor ELSE -e.amount_minor END)
                          FROM entries e WHERE e.account_id = a.id), 0)
                + COALESCE((SELECT SUM(h.amount_minor) FROM holds h WHERE h.account_id = a.id AND h.status = 'held'), 0),
                now(),
                1,
                now()
         FROM accounts a WHERE a.id = $1
         ON CONFLICT (account_id) DO UPDATE SET
           currency           = EXCLUDED.currency,
           available_minor    = EXCLUDED.available_minor,
           held_minor         = EXCLUDED.held_minor,
           total_minor        = EXCLUDED.total_minor,
           computed_upto      = now(),
           version            = wallet_snapshots.version + 1,
           updated_at         = now()",
    )
    .bind(account_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Audit: verify every snapshot equals a fresh recomputation. Returns account ids
/// whose snapshot drifted (empty = clean).
pub async fn audit_snapshots(pool: &PgPool) -> Result<Vec<Uuid>, EngineError> {
    let drifted = sqlx::query_scalar::<_, Uuid>(
        "SELECT s.account_id
         FROM wallet_snapshots s
         JOIN accounts a ON a.id = s.account_id
         WHERE s.available_minor <> (
                SELECT COALESCE(SUM(CASE WHEN e.direction = 'credit' THEN e.amount_minor ELSE -e.amount_minor END), 0)
                FROM entries e WHERE e.account_id = a.id)
            OR s.held_minor <> (
                SELECT COALESCE(SUM(h.amount_minor), 0)
                FROM holds h WHERE h.account_id = a.id AND h.status = 'held')",
    )
    .fetch_all(pool)
    .await?;
    Ok(drifted)
}
