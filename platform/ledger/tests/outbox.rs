mod common;

use anbarr_ledger::engine::LedgerEngine;
use anbarr_ledger::money::Currency;
use anbarr_ledger::outbox::{EventWorker, LedgerEventConsumer};
use anbarr_ledger::types::Origin;

#[tokio::test]
async fn payment_creates_and_consumes_outbox_event() {
    let pool = common::pool().await;
    let engine = LedgerEngine::new(pool.clone());
    let consumer = LedgerEventConsumer::new(pool.clone());
    let worker = EventWorker::new(
        engine,
        consumer.clone(),
        std::time::Duration::from_millis(10),
    );

    let sender = common::create_wallet(&pool, Currency::Sle, 50_000).await;
    let recipient = common::create_wallet(&pool, Currency::Sle, 0).await;

    let result = worker
        .post_transfer(anbarr_ledger::outbox::TransferRequest {
            idempotency_scope: "outbox-user".into(),
            idempotency_key: "outbox-tx-1".into(),
            currency: Currency::Sle,
            origin: Origin {
                channel: "test".into(),
                ..Default::default()
            },
            sender_account_id: sender,
            recipient_account_id: recipient,
            amount_minor: 4_000,
            fee_bps: 50,
        })
        .await
        .expect("transfer");

    let events = consumer.fetch_unconsumed(20).await.expect("events");
    let payment_event = events
        .iter()
        .find(|event| event.journal_id == result.journal_id)
        .expect("payment journal should emit an outbox event");
    assert_eq!(payment_event.event_type, "journal.p2p");

    let consumed = consumer.poll_once(20).await.expect("consume");
    assert!(consumed
        .iter()
        .any(|event| event.journal_id == result.journal_id));

    let remaining = consumer.fetch_unconsumed(20).await.expect("remaining");
    assert!(
        remaining
            .iter()
            .all(|event| event.journal_id != result.journal_id),
        "payment event should be marked consumed"
    );
}
