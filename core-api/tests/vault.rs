//! Vault goal savings over the real stack (goal savings = metadata over
//! ledger holds, docs/vault.md). The caller is now an authenticated user —
//! every request carries a bearer access token minted through the real OTP
//! flow (see tests/auth.rs); no `x-amber-caller` header exists anymore.
//!
//! Requires the dev database (docker-compose.yml).

mod common;

use axum::http::StatusCode;
use common::{create_funded_wallet, harness, new_caller, request_json};
use serde_json::json;

#[tokio::test]
async fn vault_goal_lifecycle_end_to_end() {
    let h = harness("vault1").await;
    let (_, token) = new_caller(&h.app, "Vault Tester").await;

    let wallet =
        create_funded_wallet(&h.ledger_pool, anbarr_ledger::money::Currency::Sle, 500_000).await;

    // 1. Link the wallet (verified against the ledger before storing).
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/wallets",
        Some(&token),
        Some(json!({ "account_id": wallet, "label": "Main", "primary": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["available_minor"], 500_000);

    // 2. GET /v1/wallets lists it with live balances.
    let (status, body) = request_json(&h.app, "GET", "/v1/wallets", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[0]["currency"], "SLE");
    assert_eq!(body[0]["available_minor"], 500_000);

    // 3. Create a goal — the first lock holds 200_000.
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/vault/goals",
        Some(&token),
        Some(json!({ "name": "School fees", "currency": "SLE", "amount_minor": 200_000, "target_minor": 400_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let goal_id = body["id"].as_str().unwrap().to_string();

    // Balances moved to held on the ledger.
    let (_, wallets) = request_json(&h.app, "GET", "/v1/wallets", Some(&token), None).await;
    assert_eq!(wallets[0]["available_minor"], 300_000);
    assert_eq!(wallets[0]["held_minor"], 200_000);

    // 4. Add a second lock.
    let (status, body) = request_json(
        &h.app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/locks"),
        Some(&token),
        Some(json!({ "amount_minor": 100_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["locked_minor"], 300_000);

    // 5. List goals: progress is derived from open locks.
    let (_, goals) = request_json(&h.app, "GET", "/v1/vault/goals", Some(&token), None).await;
    assert_eq!(goals[0]["locked_minor"], 300_000);
    assert_eq!(goals[0]["locks"].as_array().unwrap().len(), 2);

    // 6. Release everything — whole holds return to available.
    let (status, body) = request_json(
        &h.app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/release"),
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["released_minor"], 300_000);
    assert_eq!(body["released_holds"], 2);

    let (_, wallets) = request_json(&h.app, "GET", "/v1/wallets", Some(&token), None).await;
    assert_eq!(wallets[0]["available_minor"], 500_000);
    assert_eq!(wallets[0]["held_minor"], 0);

    // 7. The goal is closed out.
    let (_, goals) = request_json(&h.app, "GET", "/v1/vault/goals", Some(&token), None).await;
    assert_eq!(goals[0]["status"], "released");
    assert_eq!(goals[0]["locked_minor"], 0);
}

#[tokio::test]
async fn vault_goal_insufficient_funds_is_422_and_no_goal_row() {
    let h = harness("vault1").await;
    let (_, token) = new_caller(&h.app, "Vault Tester").await;

    let wallet =
        create_funded_wallet(&h.ledger_pool, anbarr_ledger::money::Currency::Sle, 100_000).await;

    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/wallets",
        Some(&token),
        Some(json!({ "account_id": wallet, "primary": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Ask for more than the wallet holds: the ledger's funds rule fires.
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/vault/goals",
        Some(&token),
        Some(json!({ "name": "Too rich", "currency": "SLE", "amount_minor": 900_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // No phantom goal row was persisted (money-first ordering).
    let (_, goals) = request_json(&h.app, "GET", "/v1/vault/goals", Some(&token), None).await;
    assert_eq!(goals.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn vault_goal_currency_mismatch_is_400() {
    let h = harness("vault1").await;
    let (_, token) = new_caller(&h.app, "Vault Tester").await;

    let wallet =
        create_funded_wallet(&h.ledger_pool, anbarr_ledger::money::Currency::Sle, 100_000).await;

    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/wallets",
        Some(&token),
        Some(json!({ "account_id": wallet, "primary": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/vault/goals",
        Some(&token),
        Some(json!({ "name": "USD goal", "currency": "USD", "amount_minor": 10_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn vault_goal_ownership_is_enforced() {
    let h = harness("vault1").await;
    let (_, token) = new_caller(&h.app, "Owner").await;

    let wallet =
        create_funded_wallet(&h.ledger_pool, anbarr_ledger::money::Currency::Sle, 400_000).await;

    let (status, _) = request_json(
        &h.app,
        "POST",
        "/v1/wallets",
        Some(&token),
        Some(json!({ "account_id": wallet, "primary": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = request_json(
        &h.app,
        "POST",
        "/v1/vault/goals",
        Some(&token),
        Some(json!({ "name": "Mine", "currency": "SLE", "amount_minor": 50_000 })),
    )
    .await;
    assert!(body["id"].is_string(), "goal creation failed: {body}");
    let goal_id = body["id"].as_str().unwrap().to_string();

    // A foreign authenticated caller sees no goals.
    let (_, other_token) = new_caller(&h.app, "Foreigner").await;
    let (_, goals) = request_json(&h.app, "GET", "/v1/vault/goals", Some(&other_token), None).await;
    assert_eq!(goals.as_array().unwrap().len(), 0);

    // A foreign caller cannot release them either.
    let (status, _) = request_json(
        &h.app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/release"),
        Some(&other_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The owner still can.
    let (status, _) = request_json(
        &h.app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/release"),
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

/// Unauthenticated requests are rejected everywhere — the old dev header is
/// gone, so "anonymous" no longer means "a caller name".
#[tokio::test]
async fn vault_and_wallets_reject_anonymous_callers() {
    let h = harness("vault1").await;

    for (method, uri) in [
        ("GET", "/v1/wallets"),
        ("POST", "/v1/wallets"),
        ("GET", "/v1/vault/goals"),
        ("POST", "/v1/vault/goals"),
    ] {
        let (status, _) = request_json(&h.app, method, uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}
