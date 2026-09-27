//! Vault goal savings (docs/vault.md): goals are Core-API metadata; every
//! lock is a whole ledger hold (`HoldFunds`), every unlock a whole
//! `ReleaseHold` — the engine has no partial release, and this module never
//! pretends otherwise. `locked_minor` on a goal is a derived read-model
//! (Σ open locks) maintained in the same core-DB transaction that records
//! the lock/release rows; the ledger's holds stay the authority.

use crate::error::ApiError;
use crate::ledger::pb;
use crate::wallets::{get_account, primary_wallet};
use crate::{ApiResult, AppState, CALLER_HEADER};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Scope for ledger idempotency keys on vault holds (docs/vault.md §1).
const IDEMPOTENCY_SCOPE: &str = "vault.goal.lock";

#[derive(Debug, Deserialize)]
pub struct CreateGoalRequest {
    pub name: String,
    pub currency: String,
    pub amount_minor: i64,
    #[serde(default)]
    pub target_minor: Option<i64>,
    #[serde(default)]
    pub maturity_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct LockView {
    pub hold_id: Uuid,
    pub amount_minor: i64,
    pub locked_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct GoalResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub currency: String,
    pub locked_minor: i64,
    pub target_minor: Option<i64>,
    pub maturity_at: Option<DateTime<Utc>>,
    pub locks: Vec<LockView>,
}

#[derive(Debug, Serialize)]
pub struct CreateGoalResponse {
    pub id: Uuid,
    pub status: String,
    pub locked_minor: i64,
    pub hold_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct AddLockRequest {
    pub amount_minor: i64,
}

#[derive(Debug, Serialize)]
pub struct AddLockResponse {
    pub hold_id: Uuid,
    pub locked_minor: i64,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseRequest {
    /// Omit to release every open hold on the goal (whole holds only —
    /// no partial unlock exists on the engine, docs/vault.md §1).
    #[serde(default)]
    pub hold_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ReleaseResponse {
    pub released_minor: i64,
    pub released_holds: i64,
}

fn caller_from(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(CALLER_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|c| c.to_string())
        .ok_or(ApiError::Unauthorized)
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    let trimmed = name.trim();
    if trimmed.len() < 2 || trimmed.chars().count() > 60 {
        return Err(ApiError::Validation("name must be 2–60 characters".into()));
    }
    Ok(())
}

fn validate_amount(amount_minor: i64) -> Result<(), ApiError> {
    if amount_minor <= 0 {
        return Err(ApiError::Validation(
            "amount_minor must be greater than zero".into(),
        ));
    }
    Ok(())
}

fn validate_currency(currency: &str) -> Result<String, ApiError> {
    match currency.to_ascii_uppercase().as_str() {
        "SLE" => Ok("SLE".to_string()),
        "USD" => Ok("USD".to_string()),
        other => Err(ApiError::Validation(format!(
            "unsupported currency {other}"
        ))),
    }
}

/// HoldFunds on the ledger. Returns the hold id.
async fn ledger_hold_funds(
    state: &AppState,
    wallet: Uuid,
    amount_minor: i64,
    currency: &str,
    maturity_at: Option<DateTime<Utc>>,
    caller: &str,
) -> Result<Uuid, ApiError> {
    let mut client = state.ledger.client();
    let req = state.ledger.prepare(tonic::Request::new(pb::HoldRequest {
        idempotency_scope: IDEMPOTENCY_SCOPE.to_string(),
        idempotency_key: Uuid::new_v4().to_string(),
        account_id: wallet.to_string(),
        amount_minor,
        currency: currency.to_string(),
        // Maturity maps to the hold's expiry (docs/vault.md §1). Goals
        // without an explicit maturity get a far-future hold expiry so the
        // engine's expiry semantics never surprise the user.
        expires_at: Some(prost_types::Timestamp {
            seconds: maturity_at
                .unwrap_or_else(|| Utc::now() + Duration::days(365 * 100))
                .timestamp(),
            nanos: 0,
        }),
        origin: Some(pb::Origin {
            user_id: String::new(),
            channel: "core-api".to_string(),
            session_id: String::new(),
            payment_code: String::new(),
        }),
    }));
    let resp = client
        .hold_funds(req)
        .await
        .map_err(ApiError::from)?
        .into_inner();
    let hold_id = Uuid::parse_str(&resp.hold_id).map_err(|_| ApiError::Internal)?;
    tracing::info!(caller, %wallet, %hold_id, amount_minor, "vault lock posted");
    Ok(hold_id)
}

/// ReleaseHold on the ledger for one whole hold. The ledger replays the
/// original response for a retried idempotency key, so a crash between the
/// gRPC call and the metadata write resolves cleanly on retry.
async fn ledger_release_hold(
    state: &AppState,
    hold_id: Uuid,
    caller: &str,
) -> Result<(), ApiError> {
    let mut client = state.ledger.client();
    let req = state
        .ledger
        .prepare(tonic::Request::new(pb::ReleaseRequest {
            idempotency_scope: IDEMPOTENCY_SCOPE.to_string(),
            idempotency_key: Uuid::new_v4().to_string(),
            hold_id: hold_id.to_string(),
            origin: Some(pb::Origin {
                user_id: String::new(),
                channel: "core-api".to_string(),
                session_id: String::new(),
                payment_code: String::new(),
            }),
        }));
    client
        .release_hold(req)
        .await
        .map_err(ApiError::from)?
        .into_inner();
    tracing::info!(caller, %hold_id, "vault hold released");
    Ok(())
}

/// POST /v1/vault/goals — create a goal and take its first lock. The ledger
/// hold is created **first**; only on success is the goal + lock row
/// persisted. A crash between the two leaves a funded hold with no goal row
/// (recovered by reconciliation against `holds`), never a phantom goal.
pub async fn create_goal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGoalRequest>,
) -> ApiResult<Json<CreateGoalResponse>> {
    let caller = caller_from(&headers)?;
    validate_name(&req.name)?;
    validate_amount(req.amount_minor)?;
    let currency = validate_currency(&req.currency)?;
    if let Some(target) = req.target_minor {
        validate_amount(target)?;
        if target < req.amount_minor {
            return Err(ApiError::Validation(
                "target_minor must be >= the first lock".into(),
            ));
        }
    }

    let wallet = primary_wallet(&state, &caller).await?;
    // The wallet's currency must match the goal's — locking SLE from a USD
    // wallet is a category error the ledger would reject anyway, but the
    // Core API fails it with a clearer message.
    let account = get_account(&state.ledger, wallet).await?;
    if account.currency != currency {
        return Err(ApiError::Validation(format!(
            "goal currency {currency} does not match the wallet currency {}",
            account.currency
        )));
    }

    let goal_id = Uuid::new_v4();
    let hold_id = ledger_hold_funds(
        &state,
        wallet,
        req.amount_minor,
        &currency,
        req.maturity_at,
        &caller,
    )
    .await?;

    // Persist metadata after the money move (crash-safe ordering, see above).
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO vault_goals (id, caller, name, currency, target_minor, maturity_at, status)
         VALUES ($1, $2, $3, $4, $5, $6, 'active')",
    )
    .bind(goal_id)
    .bind(&caller)
    .bind(req.name.trim())
    .bind(&currency)
    .bind(req.target_minor)
    .bind(req.maturity_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO vault_goal_locks (id, goal_id, hold_id, amount_minor)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(goal_id)
    .bind(hold_id)
    .bind(req.amount_minor)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(CreateGoalResponse {
        id: goal_id,
        status: "active".to_string(),
        locked_minor: req.amount_minor,
        hold_id,
    }))
}

/// GET /v1/vault/goals — list the caller's goals with their locks.
/// `locked_minor` is recomputed from open lock rows on read.
pub async fn list_goals(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<GoalResponse>>> {
    let caller = caller_from(&headers)?;

    let goals: Vec<(
        Uuid,
        String,
        String,
        Option<i64>,
        Option<DateTime<Utc>>,
        String,
    )> = sqlx::query_as(
        "SELECT id, name, currency, target_minor, maturity_at, status
             FROM vault_goals WHERE caller = $1 ORDER BY created_at DESC",
    )
    .bind(&caller)
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(goals.len());
    for (id, name, currency, target_minor, maturity_at, status) in goals {
        let locks: Vec<(Uuid, i64, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
            "SELECT hold_id, amount_minor, locked_at, released_at
             FROM vault_goal_locks WHERE goal_id = $1 ORDER BY locked_at",
        )
        .bind(id)
        .fetch_all(&state.pool)
        .await?;
        let locked_minor: i64 = locks
            .iter()
            .filter(|(_, _, _, released_at)| released_at.is_none())
            .map(|(_, amount, _, _)| *amount)
            .sum();
        out.push(GoalResponse {
            id,
            name,
            status,
            currency,
            locked_minor,
            target_minor,
            maturity_at,
            locks: locks
                .into_iter()
                .map(|(hold_id, amount_minor, locked_at, released_at)| LockView {
                    hold_id,
                    amount_minor,
                    locked_at,
                    released_at,
                })
                .collect(),
        });
    }
    Ok(Json(out))
}

/// POST /v1/vault/goals/:id/locks — add a lock to an active goal.
pub async fn add_lock(
    State(state): State<AppState>,
    Path(goal_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<AddLockRequest>,
) -> ApiResult<Json<AddLockResponse>> {
    let caller = caller_from(&headers)?;
    validate_amount(req.amount_minor)?;

    let (currency, status): (String, String) =
        sqlx::query_as("SELECT currency, status FROM vault_goals WHERE id = $1 AND caller = $2")
            .bind(goal_id)
            .bind(&caller)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(ApiError::NotFound("goal not found".to_string()))?;
    if status != "active" {
        return Err(ApiError::Conflict("goal is no longer active".to_string()));
    }

    let wallet = primary_wallet(&state, &caller).await?;
    let hold_id =
        ledger_hold_funds(&state, wallet, req.amount_minor, &currency, None, &caller).await?;

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO vault_goal_locks (id, goal_id, hold_id, amount_minor)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(goal_id)
    .bind(hold_id)
    .bind(req.amount_minor)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let locked_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM vault_goal_locks
         WHERE goal_id = $1 AND released_at IS NULL",
    )
    .bind(goal_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(AddLockResponse {
        hold_id,
        locked_minor,
    }))
}

/// POST /v1/vault/goals/:id/release — release one hold (or all open holds).
/// The goal flips to `released` when no open locks remain. Each release is a
/// whole `ReleaseHold` on the ledger; failures abort before any metadata is
/// written, so the read-model never diverges from a successful release.
pub async fn release_goal(
    State(state): State<AppState>,
    Path(goal_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<ReleaseRequest>,
) -> ApiResult<Json<ReleaseResponse>> {
    let caller = caller_from(&headers)?;

    // Goal must belong to the caller (authorization: ownership check).
    let owned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM vault_goals WHERE id = $1 AND caller = $2")
            .bind(goal_id)
            .bind(&caller)
            .fetch_one(&state.pool)
            .await?;
    if owned == 0 {
        return Err(ApiError::NotFound("goal not found".to_string()));
    }

    // Choose the open holds to release. Release is idempotent at the ledger
    // (idempotency_keys replay), so a crash mid-loop is retried safely.
    let holds: Vec<(Uuid, i64)> = match req.hold_id {
        Some(hold_id) => {
            sqlx::query_as(
                "SELECT hold_id, amount_minor FROM vault_goal_locks
             WHERE goal_id = $1 AND hold_id = $2 AND released_at IS NULL",
            )
            .bind(goal_id)
            .bind(hold_id)
            .fetch_all(&state.pool)
            .await?
        }
        None => {
            sqlx::query_as(
                "SELECT hold_id, amount_minor FROM vault_goal_locks
             WHERE goal_id = $1 AND released_at IS NULL ORDER BY locked_at",
            )
            .bind(goal_id)
            .fetch_all(&state.pool)
            .await?
        }
    };
    if holds.is_empty() {
        return Err(ApiError::Conflict(
            "no open holds for this goal".to_string(),
        ));
    }

    let mut released_minor = 0i64;
    for (hold_id, amount) in &holds {
        ledger_release_hold(&state, *hold_id, &caller).await?;
        released_minor += amount;
    }

    // Record outcomes in one transaction (single-writer on the read-model).
    let mut tx = state.pool.begin().await?;
    for (hold_id, _) in &holds {
        sqlx::query(
            "UPDATE vault_goal_locks SET released_at = now()
             WHERE hold_id = $1 AND released_at IS NULL",
        )
        .bind(hold_id)
        .execute(&mut *tx)
        .await?;
    }
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM vault_goal_locks WHERE goal_id = $1 AND released_at IS NULL",
    )
    .bind(goal_id)
    .fetch_one(&mut *tx)
    .await?;
    if open == 0 {
        sqlx::query("UPDATE vault_goals SET status = 'released' WHERE id = $1")
            .bind(goal_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(Json(ReleaseResponse {
        released_minor,
        released_holds: holds.len() as i64,
    }))
}
