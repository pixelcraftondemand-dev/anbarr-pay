//! Tests for account provisioning (`LedgerEngine::create_account`).
//!
//! Each test provisions accounts with fresh owner ids, so no shared singleton
//! is touched and no serialization is needed between tests.

mod common;

use anbarr_ledger::engine::{CreateAccountRequest, LedgerEngine};
use anbarr_ledger::money::Currency;
use anbarr_ledger::types::AccountType;
use sqlx::Acquire;
use std::sync::Arc;
use uuid::Uuid;

use common::pool;

fn wallet_req(owner: Uuid, name: &str) -> CreateAccountRequest {
    CreateAccountRequest {
        owner_type: "user".into(),
        owner_id: Some(owner),
        account_type: AccountType::Wallet,
        currency: Currency::Sle,
        name: name.into(),
    }
}

#[tokio::test]
async fn creates_wallet_account() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());
    let owner = Uuid::new_v4();

    let res = engine
        .create_account(wallet_req(owner, "alice"))
        .await
        .expect("create wallet");
    assert!(res.created);
    assert_eq!(res.status, "active");

    let (owner_type, account_type, currency): (String, String, String) =
        sqlx::query_as("SELECT owner_type, type, currency FROM accounts WHERE id = $1")
            .bind(res.account_id)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(owner_type, "user");
    assert_eq!(account_type, "wallet");
    assert_eq!(currency, "SLE");

    // Fresh wallet starts at zero, and the engine reports it.
    let bal = anbarr_ledger::balances::get_balance(&p, res.account_id)
        .await
        .unwrap();
    assert_eq!(bal.available_minor, 0);
    assert_eq!(bal.total_minor, 0);
}

#[tokio::test]
async fn create_is_idempotent() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());
    let owner = Uuid::new_v4();

    let first = engine
        .create_account(wallet_req(owner, "bob"))
        .await
        .unwrap();
    let second = engine
        .create_account(wallet_req(owner, "bob"))
        .await
        .unwrap();
    assert!(first.created);
    assert!(!second.created, "replay must report existing");
    assert_eq!(first.account_id, second.account_id, "same account returned");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE owner_id = $1")
        .bind(owner)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn creates_agent_float() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());
    let owner = Uuid::new_v4();

    let res = engine
        .create_account(CreateAccountRequest {
            owner_type: "agent".into(),
            owner_id: Some(owner),
            account_type: AccountType::Float,
            currency: Currency::Sle,
            name: "agent float".into(),
        })
        .await
        .expect("create agent float");
    assert!(res.created);

    let (owner_type, account_type): (String, String) =
        sqlx::query_as("SELECT owner_type, type FROM accounts WHERE id = $1")
            .bind(res.account_id)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(owner_type, "agent");
    assert_eq!(account_type, "float");
}

#[tokio::test]
async fn platform_accounts_cannot_be_created_through_the_api() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());

    // owner_type platform is not provisionable.
    let err = engine
        .create_account(CreateAccountRequest {
            owner_type: "platform".into(),
            owner_id: None,
            account_type: AccountType::Wallet,
            currency: Currency::Sle,
            name: "nope".into(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, anbarr_ledger::engine::EngineError::InvalidAccount(_)),
        "got {err:?}"
    );

    // Platform-managed account types are not provisionable either.
    let err = engine
        .create_account(CreateAccountRequest {
            owner_type: "user".into(),
            owner_id: Some(Uuid::new_v4()),
            account_type: AccountType::HoldEscrow,
            currency: Currency::Sle,
            name: "nope".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidAccount(_)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_creates_of_same_owner_yield_one_account() {
    let p = pool().await;
    let engine = Arc::new(LedgerEngine::new(p.clone()));
    let owner = Uuid::new_v4();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let engine = engine.clone();
        let req = wallet_req(owner, "race");
        handles.push(tokio::spawn(
            async move { engine.create_account(req).await },
        ));
    }

    let mut created = 0;
    let mut ids = std::collections::HashSet::new();
    for h in handles {
        let res = h.await.expect("join").expect("create succeeds");
        if res.created {
            created += 1;
        }
        ids.insert(res.account_id);
    }
    assert_eq!(created, 1, "exactly one request wins the insert");
    assert_eq!(ids.len(), 1, "every caller receives the same account");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE owner_id = $1")
        .bind(owner)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn create_account_rejects_empty_name() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());

    let err = engine
        .create_account(CreateAccountRequest {
            owner_type: "user".into(),
            owner_id: Some(Uuid::new_v4()),
            account_type: AccountType::Wallet,
            currency: Currency::Sle,
            name: "".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidAccount(_)
    ));
}

#[tokio::test]
async fn create_account_lost_race_returns_existing() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());
    let owner = Uuid::new_v4();

    // A concurrent create owns the (owner_type, owner_id, type, currency) key
    // in an uncommitted transaction: this create's existence check misses it,
    // then its INSERT loses the ON CONFLICT race and must return the winner.
    let mut conn = p.acquire().await.unwrap();
    let mut winner = conn.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status)
         VALUES ($1, 'user', $2, 'wallet', 'SLE', 'winner', 'active')",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&mut *winner)
    .await
    .unwrap();

    let engine2 = engine.clone();
    let req = wallet_req(owner, "loser");
    let task = tokio::spawn(async move { engine2.create_account(req).await });

    // Wait for the create's INSERT to block on the uncommitted row, then let
    // the winner commit so the loser takes the lost-race path.
    for _ in 0..100 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
             WHERE state = 'active' AND wait_event_type IS NOT NULL",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        if waiting >= 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    winner.commit().await.unwrap();

    let res = task.await.expect("join").expect("create succeeds");
    assert!(!res.created, "the winning account is returned");
    assert_eq!(res.status, "active");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE owner_id = $1")
        .bind(owner)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 1, "exactly one account exists");
}
