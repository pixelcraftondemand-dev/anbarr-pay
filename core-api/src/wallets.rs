//! Wallet registry (docs/vault.md §4). Until sign-in ships, the gateway
//! names the caller with `x-amber-caller`; the registry maps that caller to
//! a ledger account that has been **verified against the ledger** before the
//! link is stored. Balances shown by `GET /v1/wallets` are always live
//! ledger responses — the registry stores no amounts.

use crate::error::ApiError;
use crate::ledger::pb;
use crate::{ApiResult, AppState, LedgerClient, CALLER_HEADER};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct LinkWalletRequest {
    pub account_id: Uuid,
    #[serde(default)]
    pub label: String,
    /// Mark as the caller's primary wallet (used to resolve goals' wallet).
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Serialize)]
pub struct WalletResponse {
    pub id: Uuid,
    pub currency: String,
    pub label: String,
    pub is_primary: bool,
    pub status: String,
    pub available_minor: i64,
    pub held_minor: i64,
    pub total_minor: i64,
}

fn caller_from(headers: &HeaderMap) -> Result<String, ApiError> {
    let caller = headers
        .get(CALLER_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or(ApiError::Unauthorized)?;
    Ok(caller.to_string())
}

/// POST /v1/wallets — link a caller to a ledger account. The account must
/// exist and be active per the ledger (`GetAccount`); the link row stores no
/// balances, so nothing here can go stale in a way money depends on.
pub async fn link_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LinkWalletRequest>,
) -> ApiResult<Json<WalletResponse>> {
    let caller = caller_from(&headers)?;

    // Verify against the ledger before storing anything (docs/vault.md §4:
    // "verified against ledger GetAccount"). Errors map through the shared
    // problem+json contract (404 for an unknown account).
    let account = get_account(&state.ledger, req.account_id).await?;

    // First link for a caller becomes primary automatically; afterwards only
    // an explicit `primary: true` moves the flag.
    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM wallet_links WHERE caller = $1")
        .bind(&caller)
        .fetch_one(&state.pool)
        .await?;
    let primary = req.primary || existing == 0;

    if primary {
        sqlx::query("UPDATE wallet_links SET is_primary = FALSE WHERE caller = $1")
            .bind(&caller)
            .execute(&state.pool)
            .await?;
    }

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO wallet_links (id, caller, account_id, currency, label, is_primary)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (caller, account_id)
         DO UPDATE SET label = EXCLUDED.label, is_primary = EXCLUDED.is_primary
         RETURNING id",
    )
    .bind(id)
    .bind(&caller)
    .bind(req.account_id)
    .bind(&account.currency)
    .bind(req.label.trim())
    .bind(primary)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(WalletResponse {
        id: req.account_id,
        currency: account.currency,
        label: req.label.trim().to_string(),
        is_primary: primary,
        status: "active".to_string(),
        available_minor: account.available_minor,
        held_minor: account.held_minor,
        total_minor: account.total_minor,
    }))
}

/// GET /v1/wallets — the registry's links with **live** balances fetched from
/// the ledger per request. The web client already calls this endpoint
/// (web/src/screens/Home.tsx) and renders server numbers verbatim.
pub async fn list_wallets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<WalletResponse>>> {
    let caller = caller_from(&headers)?;

    let links: Vec<(Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT account_id, currency, label, is_primary
         FROM wallet_links WHERE caller = $1 ORDER BY is_primary DESC, created_at",
    )
    .bind(&caller)
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(links.len());
    for (account_id, _currency, label, is_primary) in links {
        let account = get_account(&state.ledger, account_id).await?;
        out.push(WalletResponse {
            id: account_id,
            currency: account.currency,
            label,
            is_primary,
            status: "active".to_string(),
            available_minor: account.available_minor,
            held_minor: account.held_minor,
            total_minor: account.total_minor,
        });
    }
    Ok(Json(out))
}

/// Resolve the caller's primary wallet (goal operations lock against it).
pub async fn primary_wallet(state: &AppState, caller: &str) -> Result<Uuid, ApiError> {
    sqlx::query_scalar(
        "SELECT account_id FROM wallet_links WHERE caller = $1 AND is_primary LIMIT 1",
    )
    .bind(caller)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound(
        "no wallet linked for this caller — link one first".to_string(),
    ))
}

pub(crate) async fn get_account(
    ledger: &LedgerClient,
    account_id: Uuid,
) -> Result<pb::AccountResponse, ApiError> {
    let mut client = ledger.client();
    let req = ledger.prepare(tonic::Request::new(pb::AccountRequest {
        account_id: account_id.to_string(),
    }));
    let resp = client
        .get_account(req)
        .await
        .map_err(|s| match s.code() {
            tonic::Code::NotFound => {
                ApiError::NotFound(format!("account {account_id} does not exist on the ledger"))
            }
            _ => ApiError::from(s),
        })?
        .into_inner();
    Ok(resp)
}
