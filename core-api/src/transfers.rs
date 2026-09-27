//! P2P transfers (roadmap goal 3, docs/api.md §4): the first non-hold money
//! path through the Core API. The flow is money-first, mirroring vault.rs:
//!
//! 1. Validate + authorize (session `Caller` + one-time `PinAuth` token).
//! 2. Resolve the recipient: a phone handle that matches an existing user's
//!    E.164 phone, else an on-ledger account UUID (advanced use).
//! 3. Post the transfer on the ledger (`PostTransfer`, fee computed in Rust
//!    per architecture.md §F3 — 50 bps to the payer, banker's rounding).
//! 4. Only then persist the `transfers` row (COMPLETED + journal id).
//!
//! Idempotency: the client's `Idempotency-Key` header is unique per
//! (user, key) in `transfers`. A retry with the same key returns the stored
//! response (200 replay, docs/api.md §2); a same key with a different body
//! is a 422 conflict. A crash between the gRPC post and the row insert
//! leaves a posted journal with no row — the next retry with the same key
//! posts again with a fresh key... so the ledger-side idempotency scope
//! derives from the caller's key, making that retry a ledger replay too.
//!
//! The pin_token is consumed by `PinAuth` even on validation failure *after*
//! extraction — the client re-verifies the PIN and gets a fresh token; this
//! keeps a captured token worthless (one request, ever).

use crate::auth::{normalize_phone, Caller};
use crate::error::ApiError;
use crate::jobs;
use crate::ledger::pb;
use crate::pin::PinAuth;
use crate::wallets::{get_account, primary_wallet};
use crate::{ApiResult, AppState};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Platform fee for P2P transfers: 0.5% charged to the payer
/// (architecture.md §F3, "confirmed"). 50 basis points.
const P2P_FEE_BPS: u32 = 50;
/// Government transaction tax (e-levy-style): 0 until a market turns it on
/// (architecture.md §F7 — configuration, not schema; never a default).
const P2P_TAX_BPS: u32 = 0;
/// gRPC idempotency scope for P2P payments (ledger keys are (scope, key)).
const IDEMPOTENCY_SCOPE: &str = "p2p.transfer";

#[derive(Debug, Deserialize)]
pub struct CreateTransferRequest {
    pub recipient_email_or_phone: String,
    pub amount_minor: i64,
    pub currency: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub id: Uuid,
    pub status: String,
    pub journal_id: Option<Uuid>,
    pub fee_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
}

/// Extract the client-generated idempotency key (docs/api.md §1).
fn idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    if key.is_empty() {
        return Err(ApiError::Validation(
            "Idempotency-Key header is required for money movement".into(),
        ));
    }
    if key.len() > 255 {
        return Err(ApiError::Validation("Idempotency-Key too long".into()));
    }
    Ok(key.to_string())
}

/// Validate the transfer amount (positive, bounded).
fn validate_amount(amount_minor: i64) -> Result<(), ApiError> {
    if amount_minor <= 0 {
        return Err(ApiError::Validation(
            "amount_minor must be greater than zero".into(),
        ));
    }
    if amount_minor > 100_000_000_000 {
        return Err(ApiError::Validation(
            "amount_minor exceeds the maximum".into(),
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

/// Resolve a recipient handle to an on-ledger account.
///
/// - A phone (normalized to E.164) maps to a `users` row; the recipient must
///   have linked a wallet — transfers to wallet-less users are 404 for now
///   (the "invite + credit-on-signup" flow is a later roadmap item).
/// - Otherwise the handle must be a ledger account UUID (an advanced /
///   internal use of the same endpoint).
async fn resolve_recipient(
    state: &AppState,
    handle: &str,
    currency: &str,
) -> Result<(Uuid, String, String), ApiError> {
    // Phone path — never surface *which* check failed (no user enumeration):
    // an unknown phone and a wallet-less user get the same 404 message.
    if let Ok(phone) = normalize_phone(handle) {
        let row: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
            "SELECT u.id, u.display_name,
                    (SELECT w.account_id FROM wallet_links w
                     WHERE w.caller = u.id::text AND w.currency = $2
                     ORDER BY w.is_primary DESC, w.created_at
                     LIMIT 1)
             FROM users u WHERE u.phone = $1",
        )
        .bind(&phone)
        .bind(currency)
        .fetch_optional(&state.pool)
        .await?;
        if let Some((_user_id, name, account_id)) = row {
            return match account_id {
                Some(account_id) => Ok((account_id, phone, name)),
                None => Err(ApiError::NotFound(
                    "recipient has no wallet for this currency yet".to_string(),
                )),
            };
        }
    }

    // Account-UUID path.
    let account_id = Uuid::parse_str(handle.trim())
        .map_err(|_| ApiError::NotFound("no recipient matches this address".to_string()))?;
    let account = get_account(&state.ledger, account_id).await?;
    if account.currency != currency {
        return Err(ApiError::Validation(format!(
            "recipient account is {0} — transfer currency is {1}",
            account.currency, currency
        )));
    }
    Ok((account_id, handle.trim().to_string(), String::new()))
}

/// POST /v1/transfers — session + PIN authorized, money-first.
pub async fn create_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    caller: Caller,
    // Extracted as an Option so enforcement can happen *after* the
    // idempotent-replay branch: a retried transfer must not demand a fresh
    // PIN (the token is single-use and may already be spent by attempt 1).
    pin: Option<PinAuth>,
    Json(req): Json<CreateTransferRequest>,
) -> ApiResult<Json<TransferResponse>> {
    let key = idempotency_key(&headers)?;
    validate_amount(req.amount_minor)?;
    let currency = validate_currency(&req.currency)?;
    let note = req.note.trim().to_string();
    if note.chars().count() > 140 {
        return Err(ApiError::Validation("note must be ≤ 140 characters".into()));
    }

    // Idempotent replay: same (user, key) → the stored response verbatim
    // (200; docs/api.md §2 — replay of an identical request is not an error).
    // The same key with a *different* request is a client bug: 409, never a
    // silent replay of the wrong payment.
    /// Replay/conflict fields: id, status, journal_id, fee, tax, amount,
    /// currency, recipient (E.164 or account UUID, as stored).
    type StoredTransfer = (Uuid, String, Option<Uuid>, i64, i64, i64, String, String);
    let existing: Option<StoredTransfer> = sqlx::query_as(
        "SELECT id, status, journal_id, fee_minor, tax_minor,
                amount_minor, currency, recipient_phone
         FROM transfers WHERE user_id = $1 AND idempotency_key = $2",
    )
    .bind(caller.user_id)
    .bind(&key)
    .fetch_optional(&state.pool)
    .await?;
    if let Some((
        id,
        status,
        journal_id,
        fee_minor,
        tax_minor,
        amount_minor,
        stored_currency,
        stored_recipient,
    )) = existing
    {
        let same_recipient = normalize_phone(&req.recipient_email_or_phone)
            .map(|p| p == stored_recipient)
            .unwrap_or_else(|_| req.recipient_email_or_phone.trim() == stored_recipient);
        if amount_minor != req.amount_minor || stored_currency != currency || !same_recipient {
            return Err(ApiError::Conflict(
                "Idempotency-Key was already used for a different transfer".into(),
            ));
        }
        return Ok(Json(TransferResponse {
            id,
            status,
            journal_id,
            fee_minor,
            tax_minor,
            total_minor: amount_minor + fee_minor + tax_minor,
        }));
    }

    // PIN step-up (docs/api.md §1) — after the replay branch, per the
    // comment above. A missing/spent token is 401; a token minted by a
    // different user is 403 (never acceptable, even for the caller's own key).
    let pin = pin.ok_or(ApiError::Unauthorized)?;
    if pin.user_id != caller.user_id {
        return Err(ApiError::Forbidden(
            "pin_token does not belong to this session".to_string(),
        ));
    }

    let caller_str = caller.user_id.to_string();
    let wallet = primary_wallet(&state, &caller_str).await?;
    let account = get_account(&state.ledger, wallet).await?;
    if account.currency != currency {
        return Err(ApiError::Validation(format!(
            "wallet currency {} does not match the transfer currency {currency}",
            account.currency
        )));
    }

    let (payee_account, recipient_phone, recipient_name) =
        resolve_recipient(&state, &req.recipient_email_or_phone, &currency).await?;
    if payee_account == wallet {
        return Err(ApiError::Validation(
            "a transfer cannot fund itself".to_string(),
        ));
    }

    // Post the payment on the ledger. The gRPC scope is the caller's key, so
    // a retry after a crash between post and persist replays the journal.
    let mut client = state.ledger.client();
    let grpc_req = state
        .ledger
        .prepare(tonic::Request::new(pb::TransferRequest {
            idempotency_scope: IDEMPOTENCY_SCOPE.to_string(),
            idempotency_key: format!("{}:{key}", caller.user_id),
            journal_type: "p2p".to_string(),
            currency: currency.clone(),
            payer_account_id: wallet.to_string(),
            payee_account_id: payee_account.to_string(),
            amount_minor: req.amount_minor,
            fee_bps: P2P_FEE_BPS,
            tax_bps: P2P_TAX_BPS,
            origin: Some(pb::Origin {
                user_id: caller.user_id.to_string(),
                channel: "core-api".to_string(),
                session_id: String::new(),
                payment_code: String::new(),
            }),
        }));
    let resp = client
        .post_transfer(grpc_req)
        .await
        .map_err(ApiError::from)?
        .into_inner();

    let journal_id = Uuid::parse_str(&resp.journal_id).map_err(|_| ApiError::Internal)?;
    let total_minor = req.amount_minor + resp.fee_minor + resp.tax_minor;

    // Persist after the money moved (crash between here and the insert →
    // the retry replays the same ledger journal — no double debit).
    let transfer_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO transfers
             (id, user_id, wallet_account_id, recipient_phone, recipient_name,
              recipient_account_id, amount_minor, currency, fee_minor, tax_minor,
              total_minor, journal_id, status, note, idempotency_key)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'COMPLETED', $13, $14)",
    )
    .bind(transfer_id)
    .bind(caller.user_id)
    .bind(wallet)
    .bind(&recipient_phone)
    .bind(&recipient_name)
    .bind(payee_account)
    .bind(req.amount_minor)
    .bind(&currency)
    .bind(resp.fee_minor)
    .bind(resp.tax_minor)
    .bind(total_minor)
    .bind(journal_id)
    .bind(&note)
    .bind(&key)
    .execute(&state.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => ApiError::Unprocessable(
            "this transfer was already submitted with the same key but a different request"
                .to_string(),
        ),
        _ => ApiError::from(e),
    })?;

    jobs::audit(
        &state.pool,
        Some(caller.user_id),
        "PAYMENT_COMPLETED",
        "transfer",
        "success",
        serde_json::json!({
            "transfer_id": transfer_id,
            "journal_id": journal_id,
            "amount_minor": req.amount_minor,
            "fee_minor": resp.fee_minor,
            "currency": currency,
        }),
    );

    tracing::info!(
        user_id = %caller.user_id,
        %transfer_id,
        %journal_id,
        amount_minor = req.amount_minor,
        fee_minor = resp.fee_minor,
        "p2p transfer posted"
    );

    Ok(Json(TransferResponse {
        id: transfer_id,
        status: "COMPLETED".to_string(),
        journal_id: Some(journal_id),
        fee_minor: resp.fee_minor,
        tax_minor: resp.tax_minor,
        total_minor,
    }))
}
