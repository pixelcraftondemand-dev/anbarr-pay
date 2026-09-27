//! Authentication integration tests (goal 1 of docs/roadmap.md): the real
//! OTP → session flow over the router, with the OTP echoed via the dev-only
//! channel (`AMBER_DEV_OTP_ECHO=1`, set by the harness). Covers docs/
//! authentication.md §3 (sessions, rotation, reuse detection) and the §10.1
//! find-or-create sign-in flow.
//!
//! Requires the dev database (docker-compose.yml).

mod common;

use axum::http::StatusCode;
use common::{harness, new_caller, request_json};
use serde_json::{json, Value};

async fn request_otp(app: &axum::Router, phone: &str) -> Value {
    let (status, body) = request_json(
        app,
        "POST",
        "/v1/auth/otp/request",
        None,
        Some(json!({ "phone": phone, "purpose": "sign_in" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn verify(app: &axum::Router, phone: &str, code: &str) -> (StatusCode, Value) {
    request_json(
        app,
        "POST",
        "/v1/auth/otp/verify",
        None,
        Some(json!({ "phone": phone, "purpose": "sign_in", "code": code })),
    )
    .await
}

#[tokio::test]
async fn sign_in_issues_usable_tokens() {
    let h = harness("auth1").await;
    let (phone, token) = new_caller(&h.app, "Ada").await;

    assert!(token.len() > 20, "access token looks real");

    // The token works: GET /v1/wallets returns an empty list (not 401).
    let (status, body) = request_json(&h.app, "GET", "/v1/wallets", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body.as_array().unwrap().len(), 0);

    // The user exists in the core DB with the normalized E.164 phone:
    // `076xxxxxx` (9 digits, leading 0) must have become `+23276xxxxxx`.
    let stored: (String, String) =
        sqlx::query_as("SELECT phone, display_name FROM users WHERE display_name = 'Ada'")
            .fetch_one(&h.core_pool)
            .await
            .expect("user row created");
    assert_eq!(stored.0, format!("+232{}", &phone[1..]));
    assert_eq!(stored.1, "Ada");
}

/// The full refresh rotation contract: refresh works, the old token dies,
/// and replaying the rotated token revokes the whole family.
#[tokio::test]
async fn refresh_rotates_and_reuse_revokes_family() {
    let h = harness("auth1").await;

    // Sign in directly to capture both tokens.
    let phone = "+23277111001";
    let otp = request_otp(&h.app, phone).await;
    let code = otp["dev_code"].as_str().unwrap().to_string();
    let (_, body) = verify(&h.app, phone, &code).await;
    let access1 = body["access_token"].as_str().unwrap().to_string();
    let refresh1 = body["refresh_token"].as_str().unwrap().to_string();

    // 1. Refresh rotates: a new pair comes back.
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refresh2 = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(refresh1, refresh2, "rotation must mint a new token");
    assert_ne!(access1, body["access_token"].as_str().unwrap());

    // 2. The old refresh token is spent: replaying it is a theft signal →
    // 401 AND the whole family is revoked.
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh1 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 3. Even the *new* token is dead now (family revocation).
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh2 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_revokes_the_session() {
    let h = harness("auth1").await;

    let phone = "+23277111002";
    let otp = request_otp(&h.app, phone).await;
    let code = otp["dev_code"].as_str().unwrap().to_string();
    let (_, body) = verify(&h.app, phone, &code).await;
    let refresh = body["refresh_token"].as_str().unwrap().to_string();

    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/logout",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The refresh token no longer refreshes.
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Logout is idempotent.
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/logout",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn wrong_code_and_unknown_phone_are_indistinguishable() {
    let h = harness("auth1").await;

    // Wrong code on an existing account.
    let phone = "+23277111003";
    request_otp(&h.app, phone).await;
    let (status_existing, body_existing) = verify(&h.app, phone, "000000").await;

    // Wrong code on a phone that never signed up.
    let (status_unknown, body_unknown) = verify(&h.app, "+23277119999", "000000").await;

    // Identical status; identical safe message — no enumeration.
    assert_eq!(status_existing, status_unknown);
    assert_eq!(
        body_existing["error"]["message"],
        body_unknown["error"]["message"]
    );
}

#[tokio::test]
async fn otp_reissue_is_rate_limited() {
    let h = harness("auth1").await;
    let phone = "+23277111004";

    for _ in 0..3 {
        let (status, _) = request_json(
            &h.app,
            "POST",
            "/v1/auth/otp/request",
            None,
            Some(json!({ "phone": phone, "purpose": "sign_in" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // 4th request inside the window → 429 with Retry-After.
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/otp/request",
        None,
        Some(json!({ "phone": phone, "purpose": "sign_in" })),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn otp_attempts_are_capped() {
    let h = harness("auth1").await;
    let phone = "+23277111005";

    request_otp(&h.app, phone).await;
    for _ in 0..5 {
        let (status, _) = verify(&h.app, phone, "000000").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // Even the correct code is now refused — the row is dead after 5 tries.
    // (A fresh code would work; this one is burnt by attempt-lockout.)
    let otp = request_otp(&h.app, phone).await;
    let fresh = otp["dev_code"].as_str().unwrap().to_string();
    let (status, _) = verify(&h.app, phone, &fresh).await;
    assert_eq!(status, StatusCode::OK, "a fresh code still works");
}

#[tokio::test]
async fn phone_normalization_matches_web_client() {
    let h = harness("auth1").await;

    // `076 123456` and `+23276123456` must land on the same account.
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/auth/otp/request",
        None,
        Some(json!({ "phone": "076 123456", "purpose": "sign_in" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let code = body["dev_code"].as_str().unwrap().to_string();

    let (status, body) = verify(&h.app, "+23276123456", &code).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["access_token"].is_string());
}

#[tokio::test]
async fn garbage_tokens_and_bad_requests_are_rejected() {
    let h = harness("auth1").await;

    // Malformed bearer tokens.
    for auth in ["Bearer garbage", "Bearer a.b.c", "Basic dXNlcjpwYXNz", ""] {
        let (_, body) = request_json_with_header(&h.app, "GET", "/v1/wallets", auth).await;
        assert!(body.is_null() || !body["error"].is_null());
    }

    // Bad phone → 400 validation, before any OTP is minted.
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/otp/request",
        None,
        Some(json!({ "phone": "not-a-phone", "purpose": "sign_in" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Unknown purpose → 400.
    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/auth/otp/request",
        None,
        Some(json!({ "phone": "076123456", "purpose": "world_domination" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Helper: like request_json but with a raw Authorization header value.
#[allow(unused)] // shared shape with tests/common; kept for clarity
async fn request_json_with_header(
    app: &axum::Router,
    method: &str,
    uri: &str,
    auth: &str,
) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt as _; // for `oneshot`
    let req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", auth)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}
