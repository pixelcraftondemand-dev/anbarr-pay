//! P2P transfer integration tests (goal 3, docs/api.md §4) over the real
//! stack: real ledger gRPC, real Postgres, real OTP → token → PIN flows.
//! Covers the docs/transaction-state-machine.md §5 contract — idempotency
//! (replay + conflict), PIN step-up (one-time token consumption), ownership
//! and the ledger funds rule — with amounts in minor units throughout.
//!
//! Requires the dev database (docker-compose.yml).

mod common;

use axum::http::StatusCode;
use common::{create_funded_wallet, harness, new_caller, request_json, request_json_with_headers};
use serde_json::{json, Value};

/// Sign both parties in, link wallets, fund the sender's SLE wallet, and set
/// the sender's PIN. Returns (token, sender phone, recipient phone).
async fn setup_two_parties(
    tag: &str,
    funding_minor: i64,
) -> (common::TestHarness, String, String, String) {
    let h = harness(tag).await;
    let (sender_phone, sender_token) = new_caller(&h.app, "Sender").await;
    let (recipient_phone, recipient_token) = new_caller(&h.app, "Recipient").await;

    // Recipient links a wallet first (receives), then the sender's wallet is
    // created + funded and linked (it becomes their primary automatically).
    let recipient_wallet =
        create_funded_wallet(&h.ledger_pool, anbarr_ledger::money::Currency::Sle, 1_000).await;
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/wallets",
        Some(&recipient_token),
        Some(json!({ "account_id": recipient_wallet })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let sender_wallet = create_funded_wallet(
        &h.ledger_pool,
        anbarr_ledger::money::Currency::Sle,
        funding_minor,
    )
    .await;
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/wallets",
        Some(&sender_token),
        Some(json!({ "account_id": sender_wallet })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // PIN setup for the step-up factor.
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/auth/pin/set",
        Some(&sender_token),
        Some(json!({ "pin": "482913" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    (h, sender_token, sender_phone, recipient_phone)
}

/// Verify the PIN and return the one-time pin_token (docs/api.md §1).
async fn pin_token(h: &common::TestHarness, token: &str) -> String {
    let (status, body) = request_json(
        &h.app,
        "POST",
        "/v1/auth/pin/verify",
        Some(token),
        Some(json!({ "pin": "482913" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["pin_token"].as_str().unwrap().to_string()
}

fn transfer_body(recipient: &str, amount_minor: i64) -> Value {
    json!({
        "recipient_email_or_phone": recipient,
        "amount_minor": amount_minor,
        "currency": "SLE",
        "note": "lunch money"
    })
}

#[tokio::test]
async fn transfer_moves_money_end_to_end() {
    let (h, token, sender_phone, recipient) = setup_two_parties("transfers_t1", 500_000).await;

    let key = uuid::Uuid::new_v4().to_string();
    let pt = pin_token(&h, &token).await;
    let (status, body) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 120_000)),
        &[
            ("idempotency-key", key.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Contract shape: id, COMPLETED, journal_id, fee = 0.5% to the payer
    // (120_000 × 50 / 10000 = 600 exactly), tax = 0.
    assert_eq!(body["status"], "COMPLETED");
    assert!(body["id"].is_string());
    assert!(body["journal_id"].is_string());
    assert_eq!(body["fee_minor"], 600);
    assert_eq!(body["tax_minor"], 0);
    assert_eq!(body["total_minor"], 120_600);

    // Sender's balance: 500_000 − principal − fee (fee charged to the payer).
    let (_, wallets) = request_json(&h.app, "GET", "/v1/wallets", Some(&token), None).await;
    assert_eq!(wallets[0]["available_minor"], 379_400, "{wallets}");

    // Recipient's balance: credited the principal in their own session.
    // (sign_in returns (access_token, refresh_token) — keep the access half.)
    let (recipient_token, _) = common::sign_in(&h.app, &recipient, "Recipient").await;
    let (_, rwallets) =
        request_json(&h.app, "GET", "/v1/wallets", Some(&recipient_token), None).await;
    assert_eq!(rwallets[0]["available_minor"], 121_000, "{rwallets}");

    // A COMPLETED row was persisted, linked to the sender. users.phone is
    // the normalized E.164 form (+232…), not the 076… the client sent.
    let sender_e164 = format!("+232{}", &sender_phone[1..]);
    let row: (String, i64, i64) = sqlx::query_as(
        "SELECT status, amount_minor, fee_minor FROM transfers
         WHERE user_id = (SELECT id FROM users WHERE phone = $1)",
    )
    .bind(&sender_e164)
    .fetch_one(&h.core_pool)
    .await
    .expect("transfer row");
    assert_eq!(row.0, "COMPLETED");
    assert_eq!(row.1, 120_000);
    assert_eq!(row.2, 600);
}

#[tokio::test]
async fn transfer_replays_the_same_key() {
    let (h, token, _sender, recipient) = setup_two_parties("transfers_t2", 500_000).await;

    let key = uuid::Uuid::new_v4().to_string();
    let pt = pin_token(&h, &token).await;
    let (status, first) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 10_000)),
        &[
            ("idempotency-key", key.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");

    // Retry with the SAME key: replayed verbatim, no new row, no double
    // debit. The pin_token is one-time, so the retry legitimately omits it —
    // idempotent replays must not require a fresh PIN (docs/api.md §2).
    let (status, replay) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 10_000)),
        &[("idempotency-key", key.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(first["id"], replay["id"]);
    assert_eq!(first["journal_id"], replay["journal_id"]);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transfers")
        .fetch_one(&h.core_pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "exactly one transfer row for the retried key");
}

#[tokio::test]
async fn transfer_same_key_different_body_is_conflict() {
    let (h, token, _sender, recipient) = setup_two_parties("transfers_t3", 500_000).await;

    let key = uuid::Uuid::new_v4().to_string();
    let pt = pin_token(&h, &token).await;
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 10_000)),
        &[
            ("idempotency-key", key.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Same key, different amount → 409; the replay branch fires first, so
    // the spent pin_token is fine to resend here (it is never consulted).
    let (status, body) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 99_999)),
        &[
            ("idempotency-key", key.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // Still exactly one row — nothing extra moved.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transfers")
        .fetch_one(&h.core_pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn transfer_requires_the_pin_step_up() {
    let (h, token, _sender, recipient) = setup_two_parties("transfers_t4", 500_000).await;
    let key = uuid::Uuid::new_v4().to_string();

    // No Pin-Token at all → 401.
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 10_000)),
        &[("idempotency-key", key.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A garbage Pin-Token → 401 too.
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 10_000)),
        &[("idempotency-key", key.as_str()), ("pin-token", "deadbeef")],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // No transfer rows resulted from the rejected attempts.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transfers")
        .fetch_one(&h.core_pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn transfer_pin_token_is_single_use() {
    let (h, token, _sender, recipient) = setup_two_parties("transfers_t5", 500_000).await;
    let pt = pin_token(&h, &token).await;
    let key1 = uuid::Uuid::new_v4().to_string();
    let key2 = uuid::Uuid::new_v4().to_string();

    // First transfer succeeds and consumes the token...
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 1_000)),
        &[
            ("idempotency-key", key1.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // ...a second transfer with the SAME token is refused (one-time, 2 min).
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 1_000)),
        &[
            ("idempotency-key", key2.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn transfer_insufficient_funds_is_422() {
    let (h, token, _sender, recipient) = setup_two_parties("transfers_t6", 50_000).await;
    let pt = pin_token(&h, &token).await;
    let key = uuid::Uuid::new_v4().to_string();

    let (status, body) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 500_000)),
        &[
            ("idempotency-key", key.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // No row was persisted — money-first ordering.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transfers")
        .fetch_one(&h.core_pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn transfer_unknown_recipient_is_404() {
    let (h, token, _sender, _recipient) = setup_two_parties("transfers_t7", 50_000).await;
    let pt = pin_token(&h, &token).await;
    let key = uuid::Uuid::new_v4().to_string();

    let (status, body) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body("+23277119999", 1_000)),
        &[
            ("idempotency-key", key.as_str()),
            ("pin-token", pt.as_str()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn transfer_validates_amount_and_idempotency_key() {
    let (h, token, _sender, recipient) = setup_two_parties("transfers_t8", 50_000).await;
    let pt = pin_token(&h, &token).await;
    let pt_ref = pt.as_str();
    let key = uuid::Uuid::new_v4().to_string();

    // Zero / negative amounts → 400 (validation precedes the ledger call).
    for amount in [0, -5] {
        let (status, _) = request_json_with_headers(
            &h.app,
            "POST",
            "/v1/transfers",
            Some(&token),
            Some(transfer_body(&recipient, amount)),
            &[("idempotency-key", key.as_str()), ("pin-token", pt_ref)],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{amount}");
    }

    // Missing Idempotency-Key → 400 (money movement must be idempotent).
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(transfer_body(&recipient, 1_000)),
        &[("pin-token", pt_ref)],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Unsupported currency → 400.
    let (status, _) = request_json_with_headers(
        &h.app,
        "POST",
        "/v1/transfers",
        Some(&token),
        Some(json!({
            "recipient_email_or_phone": recipient,
            "amount_minor": 1_000,
            "currency": "GBP"
        })),
        &[("idempotency-key", key.as_str()), ("pin-token", pt_ref)],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transfer_requires_authentication() {
    let h = harness("transfers_t9").await;
    let (status, _) = request_json(&h.app, "POST", "/v1/transfers", None, Some(json!({}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
