//! Transaction PIN (docs/authentication.md §5, roadmap goal 2): a separate
//! authorization factor for money movement. Rules implemented here:
//! - PIN stored as salted, iterated HMAC-SHA256 ("pbkdf-style"), never
//!   plaintext; Argon2id is the upgrade path (`argon2` crate).
//! - 5 failed attempts → 60 s lockout (`pin_locked_until`), reset on success.
//! - Verification mints a **one-time** `pin_token` (TTL 2 min); the
//!   [`PinAuth`] extractor consumes it, so a captured token cannot be
//!   replayed even within its TTL.
//! - First set needs only a valid session (SMS OTP delivery does not exist
//!   yet — documented compromise); *changing* an existing PIN requires the
//!   current PIN, matching the §5 re-auth intent.

use crate::auth::Caller;
use crate::error::ApiError;
use crate::{ApiResult, AppState};
use axum::extract::State;
use axum::Json;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const PIN_TOKEN_TTL_SECS: i64 = 120; // docs/api.md §1: 2 minutes
const MAX_ATTEMPTS: i32 = 5;
const LOCKOUT_SECS: i64 = 60;
const HASH_ITERS: u32 = 50_000;

fn hash_pin(pin: &str, salt_hex: &str, iters: u32) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(salt_hex.as_bytes()).expect("hmac accepts any key length");
    mac.update(pin.as_bytes());
    let mut out = mac.finalize().into_bytes();
    for _ in 1..iters {
        let mut m =
            Hmac::<Sha256>::new_from_slice(out.as_slice()).expect("hmac accepts any key length");
        m.update(pin.as_bytes());
        out = m.finalize().into_bytes();
    }
    hex::encode(out)
}

fn validate_pin(pin: &str) -> Result<String, ApiError> {
    let p = pin.trim();
    if !p.chars().all(|c| c.is_ascii_digit()) || !(4..=6).contains(&p.chars().count()) {
        return Err(ApiError::Validation("PIN must be 4–6 digits".to_string()));
    }
    Ok(p.to_string())
}

#[derive(Debug, Deserialize)]
pub struct PinRequest {
    pub pin: String,
}

#[derive(Debug, Serialize)]
pub struct PinTokenResponse {
    pub pin_token: String,
    pub expires_in: i64,
}

/// POST /v1/auth/pin/set — set (first time) or change (requires current PIN).
pub async fn set_pin(
    State(state): State<AppState>,
    caller: Caller,
    Json(req): Json<SetPinRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let pin = validate_pin(&req.pin)?;

    let existing: Option<String> = sqlx::query_scalar("SELECT pin_hash FROM users WHERE id = $1")
        .bind(caller.user_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();

    if let Some(current_hash) = existing {
        // Change: verify the current PIN (re-authentication, §5).
        let cur = req.current_pin.as_deref().map(str::trim).unwrap_or("");
        let parts: Vec<&str> = current_hash.split(':').collect();
        let ok = parts.len() == 3
            && hash_pin(cur, parts[1], parts[0].parse().unwrap_or(HASH_ITERS)) == parts[2];
        if !ok {
            return Err(ApiError::Forbidden("current PIN is incorrect".to_string()));
        }
    }

    let mut salt = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let salt_hex = hex::encode(salt);
    let stored = format!(
        "pbkdf:{HASH_ITERS}:{salt_hex}:{}",
        hash_pin(&pin, &salt_hex, HASH_ITERS)
    );

    sqlx::query(
        "UPDATE users SET pin_hash = $2, pin_attempts = 0, pin_locked_until = NULL WHERE id = $1",
    )
    .bind(caller.user_id)
    .bind(&stored)
    .execute(&state.pool)
    .await?;

    crate::jobs::audit(
        &state.pool,
        Some(caller.user_id),
        "PIN_SET",
        "pin",
        "success",
        serde_json::json!({}),
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct SetPinRequest {
    pub pin: String,
    /// Required only when a PIN already exists (change flow).
    #[serde(default)]
    pub current_pin: Option<String>,
}

/// POST /v1/auth/pin/verify — verify the PIN, mint a one-time 2-min token.
/// Error code `pin_not_set` (403) lets the client start the first-use flow.
pub async fn verify_pin(
    State(state): State<AppState>,
    caller: Caller,
    Json(req): Json<PinRequest>,
) -> ApiResult<Json<PinTokenResponse>> {
    let (pin_hash, attempts, locked_until): (
        Option<String>,
        i32,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as("SELECT pin_hash, pin_attempts, pin_locked_until FROM users WHERE id = $1")
        .bind(caller.user_id)
        .fetch_one(&state.pool)
        .await?;

    if let Some(until) = locked_until {
        if chrono::Utc::now() < until {
            return Err(ApiError::RateLimited(
                "too many PIN attempts — try again shortly".to_string(),
            ));
        }
    }

    let Some(stored) = pin_hash else {
        return Err(ApiError::Forbidden("pin_not_set".to_string()));
    };

    let parts: Vec<&str> = stored.split(':').collect();
    let iters: u32 = parts
        .first()
        .and_then(|_| stored.split(':').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(HASH_ITERS);
    let salt = stored.split(':').nth(2).unwrap_or("");
    let digest = stored.split(':').nth(3).unwrap_or("");
    let supplied = hash_pin(req.pin.trim(), salt, iters);
    let ok = !digest.is_empty()
        && supplied.len() == digest.len()
        && supplied
            .as_bytes()
            .iter()
            .zip(digest.as_bytes())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            == 0;

    if !ok {
        let attempts = attempts + 1;
        let lock = if attempts >= MAX_ATTEMPTS {
            Some(chrono::Utc::now() + chrono::Duration::seconds(LOCKOUT_SECS))
        } else {
            None
        };
        sqlx::query("UPDATE users SET pin_attempts = $2, pin_locked_until = $3 WHERE id = $1")
            .bind(caller.user_id)
            .bind(attempts)
            .bind(lock)
            .execute(&state.pool)
            .await?;
        crate::jobs::audit(
            &state.pool,
            Some(caller.user_id),
            "PIN_FAILED",
            "pin",
            "failure",
            serde_json::json!({ "attempts": attempts }),
        );
        return Err(ApiError::Unauthorized);
    }

    sqlx::query("UPDATE users SET pin_attempts = 0, pin_locked_until = NULL WHERE id = $1")
        .bind(caller.user_id)
        .execute(&state.pool)
        .await?;

    // One-time token: stored hashed, consumed by PinAuth on first use.
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let token = hex::encode(buf);
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query(
        "INSERT INTO pin_tokens (token_hash, user_id, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))",
    )
    .bind(&token_hash)
    .bind(caller.user_id)
    .bind(PIN_TOKEN_TTL_SECS as f64)
    .execute(&state.pool)
    .await?;

    Ok(Json(PinTokenResponse {
        pin_token: token,
        expires_in: PIN_TOKEN_TTL_SECS,
    }))
}

/// Extractor for money-movement endpoints: consumes the `Pin-Token` header
/// (one-time, 2-min TTL) and proves it belongs to the caller.
#[derive(Debug, Clone)]
pub struct PinAuth {
    pub user_id: Uuid,
}

#[axum::async_trait]
impl axum::extract::FromRequestParts<AppState> for PinAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("pin-token")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(ApiError::Unauthorized)?;
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));

        // Atomically consume: exactly one request ever uses a token.
        let row: Option<(Uuid,)> = sqlx::query_as(
            "UPDATE pin_tokens SET consumed_at = now()
             WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > now()
             RETURNING user_id",
        )
        .bind(&token_hash)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::from)?;

        let (user_id,) = row.ok_or(ApiError::Unauthorized)?;
        Ok(PinAuth { user_id })
    }
}
