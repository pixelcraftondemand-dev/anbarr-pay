mod common;

use anbarr_ledger::money::Currency;
use anbarr_ledger::service::{LedgerService, TransferRequest};
use anbarr_ledger::types::Origin;

#[tokio::test]
async fn transfer_service_replays_same_idempotency_key() {
    let pool = common::pool().await;
    let service = LedgerService::new(anbarr_ledger::engine::LedgerEngine::new(pool.clone()));

    let sender = common::create_wallet(&pool, Currency::Sle, 100_000).await;
    let recipient = common::create_wallet(&pool, Currency::Sle, 0).await;

    let request = TransferRequest {
        idempotency_scope: "user-1".into(),
        idempotency_key: "transfer-1".into(),
        currency: Currency::Sle,
        origin: Origin {
            channel: "test".into(),
            ..Default::default()
        },
        sender_account_id: sender,
        recipient_account_id: recipient,
        amount_minor: 2_000,
        fee_bps: 50,
        tax_bps: None,
    };

    let first = service
        .post_transfer(request.clone())
        .await
        .expect("first transfer");
    let second = service
        .post_transfer(request)
        .await
        .expect("second transfer");

    assert_eq!(first.journal_id, second.journal_id);
    assert_eq!(first.fee_minor, 10);
    assert_eq!(first.commission_minor, 0);
}
