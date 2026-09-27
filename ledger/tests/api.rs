mod common;

use anbarr_ledger::api::{create_app, TransferApiRequest};
use anbarr_ledger::money::Currency;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn transfer_route_replays_same_idempotency_key() {
    let pool = common::pool().await;
    let sender = common::create_wallet(&pool, Currency::Sle, 100_000).await;
    let recipient = common::create_wallet(&pool, Currency::Sle, 0).await;
    let app = create_app(pool.clone());

    let request = TransferApiRequest {
        idempotency_scope: "api-user".into(),
        idempotency_key: "api-transfer-1".into(),
        currency: Currency::Sle,
        sender_account_id: sender,
        recipient_account_id: recipient,
        amount_minor: 2_500,
        fee_bps: 50,
        tax_bps: None,
        channel: "test".into(),
        session_id: Some("sess-1".into()),
        user_id: Some(Uuid::new_v4()),
    };

    let body = serde_json::to_vec(&request).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/transfers")
                .header("content-type", "application/json")
                .header("authorization", "Bearer anbarrpay-demo-token")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response_2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/transfers")
                .header("content-type", "application/json")
                .header("authorization", "Bearer anbarrpay-demo-token")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_2.status(), StatusCode::OK);

    let first: serde_json::Value = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
        .into();
    let second: serde_json::Value = axum::body::to_bytes(response_2.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
        .into();

    assert_eq!(first["journal_id"], second["journal_id"]);
}

#[tokio::test]
async fn transfer_route_rejects_empty_idempotency_key() {
    let pool = common::pool().await;
    let sender = common::create_wallet(&pool, Currency::Sle, 100_000).await;
    let recipient = common::create_wallet(&pool, Currency::Sle, 0).await;
    let app = create_app(pool.clone());

    let request = TransferApiRequest {
        idempotency_scope: "api-user".into(),
        idempotency_key: String::new(),
        currency: Currency::Sle,
        sender_account_id: sender,
        recipient_account_id: recipient,
        amount_minor: 2_500,
        fee_bps: 50,
        tax_bps: None,
        channel: "test".into(),
        session_id: Some("sess-1".into()),
        user_id: Some(Uuid::new_v4()),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/transfers")
                .header("content-type", "application/json")
                .header("authorization", "Bearer anbarrpay-demo-token")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transfer_route_requires_auth_header() {
    let pool = common::pool().await;
    let sender = common::create_wallet(&pool, Currency::Sle, 100_000).await;
    let recipient = common::create_wallet(&pool, Currency::Sle, 0).await;
    let app = create_app(pool.clone());

    let request = TransferApiRequest {
        idempotency_scope: "api-user".into(),
        idempotency_key: "api-transfer-auth".into(),
        currency: Currency::Sle,
        sender_account_id: sender,
        recipient_account_id: recipient,
        amount_minor: 2_500,
        fee_bps: 50,
        tax_bps: None,
        channel: "test".into(),
        session_id: Some("sess-1".into()),
        user_id: Some(Uuid::new_v4()),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/transfers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn transfer_route_accepts_api_key_auth() {
    let pool = common::pool().await;
    let sender = common::create_wallet(&pool, Currency::Sle, 100_000).await;
    let recipient = common::create_wallet(&pool, Currency::Sle, 0).await;
    let app = create_app(pool.clone());

    let request = TransferApiRequest {
        idempotency_scope: "merchant-api".into(),
        idempotency_key: "merchant-transfer-auth".into(),
        currency: Currency::Sle,
        sender_account_id: sender,
        recipient_account_id: recipient,
        amount_minor: 1_000,
        fee_bps: 25,
        tax_bps: None,
        channel: "merchant_api".into(),
        session_id: None,
        user_id: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/transfers")
                .header("content-type", "application/json")
                .header("x-api-key", "anbarrpay-demo-key")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
