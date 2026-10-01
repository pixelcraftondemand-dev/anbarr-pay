//! Integration tests for the gRPC boundary (architecture §8): a real tonic
//! server on an ephemeral port, exercised through the generated client.
//! Verifies the §8 contract end-to-end (auth, transfer, idempotent replay,
//! mismatch rejection, error-code mapping) — the same guarantees the engine
//! tests pin, now proven across the wire.
//!
//! Same DB discipline as the other suites: per-binary schema (`test_grpc`)
//! and truncate under a lock before each test.

mod common;

use anbarr_ledger::grpc::LedgerGrpc;
use anbarr_ledger::grpc_proto::ledger_client::LedgerClient;
use anbarr_ledger::grpc_proto::{AccountRequest, TransferRequest as PbTransferRequest};
use anbarr_ledger::money::Currency;
use common::{pool, truncate_all};
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint, Server};
use uuid::Uuid;

static GRPC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const TOKEN: &str = "anbarrpay-internal-dev";

/// Start an in-process server on an ephemeral port; return the client with
/// the auth header attached to every call.
///
/// Let tonic bind the address itself via `serve(addr)` (the canonical path in
/// every tonic example). Hand-rolling the acceptor with
/// `TcpListenerStream::from_std` / `TcpListenerStream::new` proved
/// Windows-flaky here: the stream ended immediately and the server died after
/// accepting its first connection. Port choice uses a throwaway bind to :0,
/// then releases it for tonic's own bind (tests are serialized by GRPC_LOCK,
/// so the reuse race is theoretical).
async fn spawn_server() -> LedgerClient<Channel> {
    let p = pool().await;
    let engine = anbarr_ledger::engine::LedgerEngine::new(p);
    let server = LedgerGrpc::new(engine).into_server();

    // Reserve an ephemeral port, note it, release it for tonic's own bind.
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = probe.local_addr().expect("local addr");
    drop(probe);

    // The server task lives for the whole test; it ends when the test's
    // runtime shuts down. No shutdown channel is needed at this scope.
    tokio::spawn(async move {
        if let Err(e) = Server::builder().add_service(server).serve(addr).await {
            eprintln!("[grpc-test] serve error: {e}");
        }
    });

    // The released port can be re-bound by tonic a beat later; poll until the
    // server answers before handing the client to the test.
    let mut up = false;
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            up = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(up, "in-process ledger server never came up on {addr}");

    let channel = Endpoint::from_shared(format!("http://{addr}"))
        .expect("endpoint")
        .connect()
        .await
        .expect("connect to in-process ledger server");
    LedgerClient::new(channel)
}

fn auth<T>(mut req: tonic::Request<T>) -> tonic::Request<T> {
    req.metadata_mut().insert(
        "x-ledger-token",
        MetadataValue::try_from(TOKEN).expect("token header"),
    );
    req
}

fn no_auth<T>(req: tonic::Request<T>) -> tonic::Request<T> {
    req
}

fn set_dev_test_token() {
    std::env::set_var("APP_ENV", "dev");
    std::env::set_var("LEDGER_GRPC_TOKEN", TOKEN);
}

fn clear_server_secret() {
    std::env::remove_var("APP_ENV");
    std::env::remove_var("LEDGER_GRPC_TOKEN");
}

async fn balance_of(
    client: &mut LedgerClient<Channel>,
    id: String,
) -> anbarr_ledger::grpc_proto::AccountResponse {
    client
        .get_account(auth(tonic::Request::new(AccountRequest { account_id: id })))
        .await
        .expect("get account")
        .into_inner()
}

fn transfer_req(
    key: &str,
    payer: &str,
    payee: &str,
    amount: i64,
) -> tonic::Request<PbTransferRequest> {
    auth(tonic::Request::new(transfer_req_inner(
        key, payer, payee, amount,
    )))
}

/// One origin per key: a retry with the same idempotency key must be a
/// byte-identical request — the engine (correctly) treats same key + different
/// payload as an IdempotencyMismatch. Deriving origin.user_id from the key
/// keeps every helper call for one key self-consistent without needing the
/// uuid crate's v5 feature.
fn transfer_req_inner(key: &str, payer: &str, payee: &str, amount: i64) -> PbTransferRequest {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let hi = hasher.finish().to_be_bytes();
    let origin_user = Uuid::from_bytes([
        hi[0], hi[1], hi[2], hi[3], hi[4], hi[5], hi[6], hi[7], //
        0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    PbTransferRequest {
        idempotency_scope: "grpc-test".into(),
        idempotency_key: key.into(),
        journal_type: "p2p".into(),
        currency: "SLE".into(),
        payer_account_id: payer.into(),
        payee_account_id: payee.into(),
        amount_minor: amount,
        fee_bps: 50,
        tax_bps: 0,
        origin: Some(anbarr_ledger::grpc_proto::Origin {
            user_id: origin_user.to_string(),
            channel: "grpc-test".into(),
            session_id: String::new(),
            payment_code: String::new(),
        }),
    }
}

#[tokio::test]
async fn grpc_transfer_replay_and_balance() {
    let _guard = GRPC_LOCK.lock().await;
    set_dev_test_token();
    let p = pool().await;
    truncate_all(&p).await;
    let mut client = spawn_server().await;

    // Create payer/payee wallets through the boundary itself, then seed the
    // payer directly (funding is a test-harness concern).
    let create = |name: &str| {
        auth(tonic::Request::new(
            anbarr_ledger::grpc_proto::CreateAccountRequest {
                owner_type: "user".into(),
                owner_id: Uuid::new_v4().to_string(),
                account_type: "wallet".into(),
                currency: "SLE".into(),
                name: name.into(),
            },
        ))
    };
    let payer_id = client
        .create_account(create("payer"))
        .await
        .expect("create payer")
        .into_inner()
        .account_id;
    let payee_id = client
        .create_account(create("payee"))
        .await
        .expect("create payee")
        .into_inner()
        .account_id;
    common::seed_funds(
        &p,
        Uuid::parse_str(&payer_id).unwrap(),
        Currency::Sle,
        100_000,
    )
    .await;

    // Transfer 100.00 SLE with a 0.5% fee.
    let resp = client
        .post_transfer(transfer_req("t-1", &payer_id, &payee_id, 10_000))
        .await
        .expect("transfer")
        .into_inner();
    assert!(!resp.journal_id.is_empty());
    assert_eq!(resp.fee_minor, 50);

    // Idempotent replay under the same key: same journal, no double-post.
    let replay = client
        .post_transfer(transfer_req("t-1", &payer_id, &payee_id, 10_000))
        .await
        .expect("replay")
        .into_inner();
    assert_eq!(replay.journal_id, resp.journal_id);

    // Balances read back through the boundary (server-authoritative).
    let payer_bal = balance_of(&mut client, payer_id.clone()).await;
    let payee_bal = balance_of(&mut client, payee_id).await;
    assert_eq!(payer_bal.available_minor, 100_000 - 10_050);
    assert_eq!(payee_bal.available_minor, 10_000);
    assert_eq!(payee_bal.currency, "SLE");
}

#[tokio::test]
async fn grpc_rejects_missing_token() {
    let _guard = GRPC_LOCK.lock().await;
    set_dev_test_token();
    let p = pool().await;
    truncate_all(&p).await;
    let mut client = spawn_server().await;
    let payer = common::create_wallet(&p, Currency::Sle, 1_000).await;
    let payee = common::create_wallet(&p, Currency::Sle, 0).await;

    let err = client
        .post_transfer(no_auth(tonic::Request::new(transfer_req_inner(
            "t-noauth",
            &payer.to_string(),
            &payee.to_string(),
            100,
        ))))
        .await
        .expect_err("must reject without token");
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn grpc_rejects_legacy_default_token_when_env_missing() {
    let _guard = GRPC_LOCK.lock().await;
    let previous = std::env::var("LEDGER_GRPC_TOKEN").ok();
    clear_server_secret();

    // The app must fail closed when the secret is absent in non-dev mode.
    let p = pool().await;
    truncate_all(&p).await;
    let mut client = spawn_server().await;
    let payer = common::create_wallet(&p, Currency::Sle, 1_000).await;
    let payee = common::create_wallet(&p, Currency::Sle, 0).await;

    let mut req = tonic::Request::new(transfer_req_inner(
        "t-legacy-default",
        &payer.to_string(),
        &payee.to_string(),
        100,
    ));
    req.metadata_mut().insert(
        "x-ledger-token",
        MetadataValue::try_from("anbarrpay-internal-dev").expect("legacy dev token"),
    );

    let err = client
        .post_transfer(req)
        .await
        .expect_err("legacy default token should be rejected when env secret is unset");
    assert!(err.message().contains("ledger token") || err.message().contains("unauthenticated"));

    if let Some(token) = previous {
        std::env::set_var("LEDGER_GRPC_TOKEN", token);
    } else {
        std::env::remove_var("LEDGER_GRPC_TOKEN");
    }
}

#[tokio::test]
async fn grpc_mismatch_and_insufficient_funds_map_to_status_codes() {
    let _guard = GRPC_LOCK.lock().await;
    set_dev_test_token();
    let p = pool().await;
    truncate_all(&p).await;
    let mut client = spawn_server().await;
    let payer = common::create_wallet(&p, Currency::Sle, 1_000).await;
    let payee = common::create_wallet(&p, Currency::Sle, 0).await;
    let (payer, payee) = (payer.to_string(), payee.to_string());

    // Same key, different payload → FAILED_PRECONDITION (IdempotencyMismatch).
    let first = transfer_req("t-2", &payer, &payee, 100);
    client.post_transfer(first).await.expect("first");
    let mismatch = transfer_req("t-2", &payer, &payee, 200);
    let err = client
        .post_transfer(mismatch)
        .await
        .expect_err("mismatch must be rejected");
    assert_eq!(err.code(), tonic::Code::FailedPrecondition);

    // Over the available balance → FAILED_PRECONDITION (InsufficientFunds),
    // not INTERNAL: the caller can act on it.
    let broke = transfer_req("t-3", &payer, &payee, 99_000);
    let err = client
        .post_transfer(broke)
        .await
        .expect_err("insufficient funds");
    assert_eq!(err.code(), tonic::Code::FailedPrecondition);

    // Malformed UUID → INVALID_ARGUMENT before the engine sees anything.
    let bad = transfer_req("t-4", "not-a-uuid", &payee, 100);
    let err = client.post_transfer(bad).await.expect_err("bad uuid");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}
