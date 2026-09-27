//! Authentication & sessions (goal 1 of docs/roadmap.md) — the server half
//! of docs/authentication.md as it applies today: phone-first sign-in with
//! single-use hashed OTPs (§10), access tokens (§3, 15-min HMAC stateless),
//! rotating refresh tokens stored hashed (§3), and reuse detection that
//! revokes the whole session family on replay of a rotated token.
//!
//! Deliberately out of scope for this goal (land later per the roadmap):
//! passwords/TOTP (§2/§4), the transaction PIN (§5, goal 2), and the device
//! registry (§6). Rules that already hold: OTPs and refresh tokens are never
//! stored in plaintext, and every failure at sign-in is deliberately vague —
//! the same message and status whether or not the account exists
//! (docs/authentication.md preamble: never leak whether an account exists).

use crate::error::ApiError;
use crate::{ApiResult, AppState};
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::Json;
use chrono::Duration;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Bearer scheme for the `Authorization` header (docs/api.md preamble).
const BEARER: &str = "Bearer ";

/// Access tokens are short-lived (docs/authentication.md §3: 15 min).
const ACCESS_TTL: Duration = Duration::minutes(15);
/// Refresh rotation: sliding inactivity timeout (14 d) and absolute session
/// lifetime (30 d), per docs/authentication.md §3.
const REFRESH_INACTIVITY: Duration = Duration::days(14);
const REFRESH_ABSOLUTE: Duration = Duration::days(30);
/// OTP codes live ~5 min (docs/authentication.md preamble: TTL ≈ 5 min).
const OTP_TTL: Duration = Duration::minutes(5);
/// Attempt limits (docs/authentication.md §5 profile: 5 failed attempts,
/// then lockout — enforced here as an all-attempts cap on the row).
const OTP_MAX_ATTEMPTS: i32 = 5;
/// OTP re-issue rate limit (§4: max ~3 per 10 min).
const OTP_ISSUE_MAX: i64 = 3;

/// The authenticated principal. Extractors replace the old `x-amber-caller`
/// dev header everywhere: handlers now take `caller: Caller` and never see
/// transport headers at all.
#[derive(Debug, Clone)]
pub struct Caller {
    pub user_id: Uuid,
    pub phone: String,
}

/// OTP echo in dev. Delivery providers are a deployment concern (SMS gateway
/// / WhatsApp sender per docs/authentication.md §12 open items); until one
/// is configured, the code is written to the server log only when
/// `AMBER_DEV_OTP_LOG=1`, and returned in the response when
/// `AMBER_DEV_OTP_ECHO=1` — dev builds, never production.
fn otp_echo_enabled() -> bool {
    std::env::var("AMBER_DEV_OTP_ECHO").ok().as_deref() == Some("1")
}

fn dev_otp_log_enabled() -> bool {
    std::env::var("AMBER_DEV_OTP_LOG").ok().as_deref() == Some("1")
}

/// HMAC-SHA256 over the token with a server-side secret. Access tokens are
/// `user_id.expiry_unix.sig` — stateless and verifiable without a DB hit;
/// refresh tokens are 32 random bytes, opaque, and only ever stored hashed.
fn signing_key() -> Vec<u8> {
    let secret = std::env::var("AMBER_AUTH_SECRET")
        .unwrap_or_else(|_| "amberpay-dev-auth-secret-do-not-deploy".to_string());
    // Domain-separate from any future use of the same env var.
    Sha256::digest(format!("amber.auth.v1.{secret}").as_bytes()).to_vec()
}

fn sign(data: &[u8]) -> Vec<u8> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&signing_key()).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

/// Mint a stateless access token for the user. Shape:
/// `{user}.{expiry}.{nonce}.{sig}` — the nonce makes every mint unique
/// (two tokens issued in the same second are never equal) and the sig
/// covers all three fields. Safe to drop on the floor: a leaked access
/// token dies within 15 minutes; refresh tokens are the durable half and
/// are stored hashed server-side.
fn mint_access_token(user_id: Uuid) -> (String, i64) {
    use rand::RngCore;
    let exp = (Utc::now() + ACCESS_TTL).timestamp();
    let mut buf = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let nonce = hex::encode(buf);
    let payload = format!("{user_id}.{exp}.{nonce}");
    let sig = hex::encode(sign(payload.as_bytes()));
    (format!("{payload}.{sig}"), ACCESS_TTL.num_seconds())
}

/// Verify an access token: parse, check expiry, check the MAC in constant
/// time. Any failure is a bare 401 — no reason is disclosed.
fn verify_access_token(token: &str) -> Option<Uuid> {
    let (payload, sig_hex) = token.rsplit_once('.')?;
    let (user_exp, nonce) = payload.rsplit_once('.')?;
    let (user, exp_str) = user_exp.split_once('.')?;
    let user = Uuid::parse_str(user).ok()?;
    let _ = nonce; // covered by the MAC; not needed server-side
    let exp: i64 = exp_str.parse().ok()?;
    if Utc::now().timestamp() >= exp {
        return None;
    }
    let expected = sign(payload.as_bytes()); // raw MAC bytes
    let given = hex::decode(sig_hex).ok()?; // decode the token's hex sig
                                            // Constant-time comparison (docs/authentication.md preamble).
    if expected.len() != given.len() || expected.ct_eq(&given).unwrap_u8() != 1 {
        return None;
    }
    Some(user)
}

/// POST /v1/auth/otp/request — mint + store a hashed single-use OTP. The
/// response is deliberately identical whether or not the account exists.
pub async fn request_otp(
    State(state): State<AppState>,
    Json(req): Json<OtpRequestRequest>,
) -> ApiResult<Json<OtpResponse>> {
    let phone = normalize_phone(&req.phone)?;
    let purpose = validate_purpose(&req.purpose)?;

    // Re-issue rate limit (docs/authentication.md §4: ~3 per 10 min).
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM otp_codes
         WHERE phone = $1 AND purpose = $2 AND created_at > now() - interval '10 minutes'",
    )
    .bind(&phone)
    .bind(purpose)
    .fetch_one(&state.pool)
    .await?;
    if recent >= OTP_ISSUE_MAX {
        return Err(ApiError::RateLimited(
            "too many codes requested — try again later".to_string(),
        ));
    }

    // 6-digit numeric code, crypto-random (rand's OsRng).
    use rand::RngCore;
    let mut buf = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let code = format!("{:06}", u64::from_be_bytes(buf) % 1_000_000);

    sqlx::query(
        "INSERT INTO otp_codes (id, phone, code_hash, purpose, expires_at)
         VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))",
    )
    .bind(Uuid::new_v4())
    .bind(&phone)
    .bind(hash_token(&code))
    .bind(purpose)
    .bind(OTP_TTL.num_seconds() as f64)
    .execute(&state.pool)
    .await?;

    // Dev delivery: log and/or echo. Production delivery (SMS/WhatsApp
    // sender) lands with the provider integration — until then this flow
    // is honest about being incomplete (docs/authentication.md §10).
    if dev_otp_log_enabled() {
        tracing::info!(%phone, purpose, "OTP issued (dev log)");
    }
    let dev_code = if otp_echo_enabled() { Some(code) } else { None };

    Ok(Json(OtpResponse {
        sent: true,
        expires_in: OTP_TTL.num_seconds(),
        dev_code,
    }))
}

/// POST /v1/auth/otp/verify + POST /v1/auth/signin — verify the code, then
/// either create the user on the fly (first sign-in, matching the
/// `shouldCreateUser` flow in docs/authentication.md §10.1) or require it to
/// exist. Both paths consume exactly one code and issue the same token pair;
/// the response never reveals which branch ran.
pub async fn verify_otp(
    State(state): State<AppState>,
    Json(req): Json<OtpVerifyRequest>,
) -> ApiResult<Json<TokenPair>> {
    let phone = normalize_phone(&req.phone)?;
    let purpose = validate_purpose(&req.purpose)?;
    let code = req.code.trim();
    if code.is_empty() {
        return Err(ApiError::Validation("code must not be empty".into()));
    }

    // Single latest code for (phone, purpose): newest unconsumed row wins.
    let row: Option<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT id, code_hash, attempts FROM otp_codes
         WHERE phone = $1 AND purpose = $2 AND consumed_at IS NULL AND expires_at > now()
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&phone)
    .bind(purpose)
    .fetch_optional(&state.pool)
    .await?;

    let (otp_id, code_hash, attempts) = match row {
        Some(row) => row,
        None => {
            return Err(ApiError::Unauthorized);
        }
    };

    if attempts >= OTP_MAX_ATTEMPTS {
        return Err(ApiError::RateLimited(
            "too many attempts — request a new code".to_string(),
        ));
    }

    // Constant-time compare: hash the presented code, compare against the
    // stored hash. Both sides are fixed-length lowercase hex strings, so
    // comparing their bytes in constant time is equivalent to comparing
    // the digests in constant time.
    let supplied = hash_token(code);
    let ok = supplied.len() == code_hash.len()
        && supplied.as_bytes().ct_eq(code_hash.as_bytes()).unwrap_u8() == 1;
    if !ok {
        // Count the failure; after OTP_MAX_ATTEMPTS the row is dead.
        sqlx::query("UPDATE otp_codes SET attempts = attempts + 1 WHERE id = $1")
            .bind(otp_id)
            .execute(&state.pool)
            .await?;
        return Err(ApiError::Unauthorized);
    }

    // Burn the code — single use, purpose-bound, non-replayable.
    sqlx::query("UPDATE otp_codes SET consumed_at = now() WHERE id = $1")
        .bind(otp_id)
        .execute(&state.pool)
        .await?;

    // Find-or-create the user (docs/authentication.md §10.1
    // shouldCreateUser: sign-in and registration are the same flow).
    let display_name = req.display_name.as_deref().map(str::trim).unwrap_or("");
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (id, phone, display_name)
         VALUES ($1, $2, $3)
         ON CONFLICT (phone) DO UPDATE SET display_name = CASE
             WHEN excluded.display_name <> '' THEN excluded.display_name
             ELSE users.display_name END
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(&phone)
    .bind(display_name)
    .fetch_one(&state.pool)
    .await?;

    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
    if status == "suspended" {
        // Generic failure — suspended accounts get no special signal beyond
        // the status code (docs/authentication.md §2 step 5).
        return Err(ApiError::Forbidden("account unavailable".to_string()));
    }

    let pair = issue_session(&state, user_id).await?;
    tracing::info!(%user_id, "session issued");
    Ok(Json(pair))
}

/// Issue a fresh session row + token pair. Refresh tokens are 32 random
/// bytes (opaque), stored hashed; rotation keeps the previous hash so
/// replay of an already-rotated token is detectable.
async fn issue_session(state: &AppState, user_id: Uuid) -> ApiResult<TokenPair> {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let refresh = hex::encode(buf);

    let (access, expires_in) = mint_access_token(user_id);
    sqlx::query(
        "INSERT INTO sessions (id, user_id, refresh_hash, expires_at, absolute_expires_at)
         VALUES ($1, $2, $3, now() + make_interval(secs => $4), now() + make_interval(secs => $5))",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(hash_token(&refresh))
    .bind(REFRESH_INACTIVITY.num_seconds() as f64)
    .bind(REFRESH_ABSOLUTE.num_seconds() as f64)
    .execute(&state.pool)
    .await?;

    Ok(TokenPair {
        access_token: access,
        refresh_token: refresh,
        expires_in,
    })
}

/// Session row fields for refresh: id, user_id, expiry, revoked_at.
type SessionRow = (
    Uuid,
    Uuid,
    chrono::DateTime<Utc>,
    Option<chrono::DateTime<Utc>>,
);

/// POST /v1/auth/refresh — rotate the refresh token (docs/authentication.md
/// §3: every refresh issues a new token and revokes the old). Replaying a
/// rotated token is a theft signal: the whole session family for the user
/// is revoked.
pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> ApiResult<Json<TokenPair>> {
    let refresh_hash = hash_token(req.refresh_token.trim());

    let row: Option<SessionRow> = sqlx::query_as(
        "SELECT id, user_id, expires_at, revoked_at
             FROM sessions WHERE refresh_hash = $1",
    )
    .bind(&refresh_hash)
    .fetch_optional(&state.pool)
    .await?;

    let (session_id, user_id, expires_at, revoked_at) = match row {
        Some(r) => r,
        None => {
            // Not a live token. If it is a *rotated-out* token (some row's
            // prev_hash) or an already-revoked one, presenting it again is
            // the reuse signal: revoke the whole session family
            // (docs/authentication.md §3 session reuse detection).
            let stale: Option<(Uuid,)> = sqlx::query_as(
                "SELECT user_id FROM sessions
                 WHERE prev_hash = $1 OR (revoked_at IS NOT NULL AND refresh_hash = $1)
                 LIMIT 1",
            )
            .bind(&refresh_hash)
            .fetch_optional(&state.pool)
            .await?;
            if let Some((owner,)) = stale {
                tracing::warn!(user_id = %owner, "refresh token reuse detected — revoking session family");
                sqlx::query(
                    "UPDATE sessions SET revoked_at = now()
                     WHERE user_id = $1 AND revoked_at IS NULL",
                )
                .bind(owner)
                .execute(&state.pool)
                .await?;
            }
            return Err(ApiError::Unauthorized);
        }
    };

    // Replay of a *revoked* current token is also a theft signal.
    if revoked_at.is_some() {
        tracing::warn!(%user_id, "revoked refresh token replay — revoking session family");
        sqlx::query(
            "UPDATE sessions SET revoked_at = now()
             WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&state.pool)
        .await?;
        return Err(ApiError::Unauthorized);
    }
    if Utc::now() >= expires_at {
        return Err(ApiError::Unauthorized);
    }

    // Rotate: new refresh hash, old hash moves to prev_hash, sliding
    // inactivity window resets; the absolute lifetime never extends.
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let new_refresh = hex::encode(buf);

    let (access, expires_in) = mint_access_token(user_id);
    let n = sqlx::query(
        "UPDATE sessions
         SET refresh_hash = $2, prev_hash = $3, last_refreshed_at = now(),
             expires_at = least(now() + make_interval(secs => $4), absolute_expires_at)
         WHERE id = $1 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(session_id)
    .bind(hash_token(&new_refresh))
    .bind(&refresh_hash)
    .bind(REFRESH_INACTIVITY.num_seconds() as f64)
    .execute(&state.pool)
    .await?
    .rows_affected();

    if n == 0 {
        // Lost the race against a concurrent refresh/revoke — treat as
        // reuse and revoke the family (two live tokens, one is a theft).
        sqlx::query(
            "UPDATE sessions SET revoked_at = now()
             WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&state.pool)
        .await?;
        return Err(ApiError::Unauthorized);
    }

    Ok(Json(TokenPair {
        access_token: access,
        refresh_token: new_refresh,
        expires_in,
    }))
}

/// POST /v1/auth/logout — revoke the presented session (docs/authentication.md
/// §3: logout revokes the refresh token; access tokens simply expire).
/// Idempotent: unknown or already-revoked tokens answer the same `{ok}`.
pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let refresh_hash = hash_token(req.refresh_token.trim());
    sqlx::query(
        "UPDATE sessions SET revoked_at = now()
         WHERE refresh_hash = $1 AND revoked_at IS NULL",
    )
    .bind(&refresh_hash)
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// --- extraction ------------------------------------------------------------

/// Extractor for authenticated routes: verifies the `Authorization: Bearer`
/// access token, then loads the user. Suspended accounts are 403 here too —
/// frozen accounts are blocked from financial actions at the authorization
/// layer (docs/authentication.md §2 step 5).
// axum 0.7's FromRequestParts is declared via #[async_trait]; the impl must
// carry the same macro so the signature matches the trait declaration.
#[axum::async_trait]
impl FromRequestParts<AppState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;
        let token = header
            .strip_prefix(BEARER)
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(ApiError::Unauthorized)?;

        let user_id = verify_access_token(token).ok_or(ApiError::Unauthorized)?;

        let row: Option<(Uuid, String, String)> =
            sqlx::query_as("SELECT id, phone, status FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&state.pool)
                .await
                .map_err(ApiError::from)?;

        let (id, phone, status) = row.ok_or(ApiError::Unauthorized)?;
        if status == "suspended" {
            return Err(ApiError::Forbidden("account unavailable".to_string()));
        }
        Ok(Caller { user_id: id, phone })
    }
}

// --- validation helpers -----------------------------------------------------

/// E.164 normalization for SL numbers (mirrors web/src/lib/phone.ts):
/// `076 123456` → `+23276123456`. Full numbers pass through as `+<digits>`.
pub fn normalize_phone(input: &str) -> Result<String, ApiError> {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    let normalized = match digits.as_str() {
        d if d.len() == 9 && d.starts_with('0') => format!("+232{}", &d[1..]),
        d if d.len() == 8 => format!("+232{d}"),
        d if input.trim_start().starts_with('+') && d.len() >= 8 && d.len() <= 15 => {
            format!("+{d}")
        }
        _ => {
            return Err(ApiError::Validation(
                "phone must be a Sierra Leone number (e.g. 076 123456)".into(),
            ))
        }
    };
    Ok(normalized)
}

fn validate_purpose(p: &str) -> Result<&'static str, ApiError> {
    match p {
        "sign_in" => Ok("sign_in"),
        "verify_phone" => Ok("verify_phone"),
        other => Err(ApiError::Validation(format!(
            "unsupported otp purpose {other}"
        ))),
    }
}

// --- request/response types --------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OtpRequestRequest {
    pub phone: String,
    pub purpose: String,
}

#[derive(Debug, Serialize)]
pub struct OtpResponse {
    pub sent: bool,
    pub expires_in: i64,
    /// Present only when `AMBER_DEV_OTP_ECHO=1` (dev builds) — the code is
    /// never echoed by a production configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OtpVerifyRequest {
    pub phone: String,
    pub purpose: String,
    pub code: String,
    /// Only meaningful on first sign-in (user does not exist yet).
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}
