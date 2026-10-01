//! Minimal app-facing API boundary for the ledger service.
//!
//! This is deliberately narrow: it exposes one endpoint to the application layer,
//! proving the ledger can be reached through an HTTP route with idempotency,
//! validation, and a transaction status response.

use crate::engine::{EngineError, LedgerEngine};
use crate::money::Currency;
use crate::service::{LedgerService, TransferRequest};
use crate::types::Origin;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferApiRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub currency: Currency,
    pub sender_account_id: Uuid,
    pub recipient_account_id: Uuid,
    pub amount_minor: i64,
    pub fee_bps: u32,
    /// Government transaction tax in basis points (e.g. an e-levy-style
    /// levy). Absent/None = no tax. Distinct from `fee_bps`: tax is collected
    /// for the authority into `tax_payable`, never platform revenue.
    #[serde(default)]
    pub tax_bps: Option<u32>,
    pub channel: String,
    pub session_id: Option<String>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferApiResponse {
    pub journal_id: Uuid,
    pub fee_minor: i64,
    /// Tax collected from the payer into the `tax_payable` liability.
    pub tax_minor: i64,
    pub commission_minor: i64,
    pub commission_journal_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct AppState {
    pub service: LedgerService,
}

impl TransferApiRequest {
    fn validate(&self) -> Result<(), String> {
        if self.idempotency_scope.trim().is_empty() {
            return Err("idempotency_scope is required".into());
        }
        if self.idempotency_key.trim().is_empty() {
            return Err("idempotency_key is required".into());
        }
        if self.channel.trim().is_empty() {
            return Err("channel is required".into());
        }
        if self.amount_minor <= 0 {
            return Err("amount_minor must be greater than zero".into());
        }
        if self.fee_bps > 10_000 {
            return Err("fee_bps must be <= 10000".into());
        }
        if self.tax_bps.is_some_and(|bps| bps > 10_000) {
            return Err("tax_bps must be <= 10000".into());
        }
        if self.sender_account_id == self.recipient_account_id {
            return Err("sender_account_id and recipient_account_id must differ".into());
        }
        Ok(())
    }
}

pub fn create_app(pool: PgPool) -> Router {
    let engine = LedgerEngine::new(pool.clone());
    let state = AppState {
        service: LedgerService::new(engine),
    };

    Router::new()
        .route("/v1/transfers", post(handle_transfer))
        .with_state(state)
}

fn require_auth(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let has_bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .strip_prefix("Bearer ")
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .is_some()
        })
        .unwrap_or(false);

    let has_api_key = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .is_some();

    if has_bearer || has_api_key {
        return Ok(());
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "missing or invalid auth token".to_string(),
    ))
}

async fn handle_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TransferApiRequest>,
) -> Result<Json<TransferApiResponse>, (StatusCode, String)> {
    require_auth(&headers)?;

    req.validate()
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;

    let result = state
        .service
        .post_transfer(TransferRequest {
            idempotency_scope: req.idempotency_scope,
            idempotency_key: req.idempotency_key,
            currency: req.currency,
            origin: Origin {
                user_id: req.user_id,
                channel: req.channel,
                session_id: req.session_id,
                payment_code: None,
            },
            sender_account_id: req.sender_account_id,
            recipient_account_id: req.recipient_account_id,
            amount_minor: req.amount_minor,
            fee_bps: req.fee_bps,
            tax_bps: req.tax_bps,
        })
        .await
        .map_err(map_err)?;

    Ok(Json(TransferApiResponse {
        journal_id: result.journal_id,
        fee_minor: result.fee_minor,
        tax_minor: result.tax_minor,
        commission_minor: result.commission_minor,
        commission_journal_id: result.commission_journal_id,
    }))
}

fn map_err(err: EngineError) -> (StatusCode, String) {
    match err {
        EngineError::InvalidJournal(_) => (StatusCode::BAD_REQUEST, err.to_string()),
        EngineError::IdempotencyMismatch { .. } => (StatusCode::CONFLICT, err.to_string()),
        EngineError::InsufficientFunds { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
        }
        EngineError::AccountNotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}
