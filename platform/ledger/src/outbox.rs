//! Ledger event outbox: the transactional stream of committed journal events.
//!
//! The ledger writes these rows in the same transaction as each journal; a
//! separate consumer can then process the events without coupling the money path
//! to the app layer. This keeps the financial engine durable, audit-friendly, and
//! easy to replay without risking duplicate postings.

use crate::engine::{FeePolicy, LedgerEngine, PaymentRequest};
use crate::money::{Currency, MinorUnits};
use crate::types::{JournalType, Origin};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LedgerEvent {
    pub id: i64,
    pub journal_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct EventAudit {
    pub journal_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl From<&LedgerEvent> for EventAudit {
    fn from(event: &LedgerEvent) -> Self {
        EventAudit {
            journal_id: event.journal_id,
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            created_at: event.created_at,
        }
    }
}

#[derive(Clone)]
pub struct LedgerEventConsumer {
    pool: PgPool,
}

impl LedgerEventConsumer {
    pub fn new(pool: PgPool) -> Self {
        LedgerEventConsumer { pool }
    }

    pub async fn fetch_unconsumed(&self, limit: i32) -> Result<Vec<LedgerEvent>, sqlx::Error> {
        sqlx::query_as::<_, LedgerEvent>(
            "SELECT id, journal_id, event_type, payload, created_at, consumed_at
             FROM ledger_events
             WHERE consumed_at IS NULL
             ORDER BY created_at ASC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn consume_one(&self, event_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE ledger_events SET consumed_at = now() WHERE id = $1 AND consumed_at IS NULL",
        )
        .bind(event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn poll_once(&self, limit: i32) -> Result<Vec<EventAudit>, sqlx::Error> {
        let events = self.fetch_unconsumed(limit).await?;
        for event in &events {
            self.consume_one(event.id).await?;
        }
        Ok(events.iter().map(EventAudit::from).collect())
    }
}

#[derive(Debug, Clone)]
pub struct TransferRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub currency: Currency,
    pub origin: Origin,
    pub sender_account_id: Uuid,
    pub recipient_account_id: Uuid,
    pub amount_minor: MinorUnits,
    pub fee_bps: u32,
}

#[derive(Debug, Clone)]
pub struct TransferResult {
    pub journal_id: Uuid,
    pub fee_minor: MinorUnits,
}

#[derive(Clone)]
pub struct EventWorker {
    engine: LedgerEngine,
    consumer: LedgerEventConsumer,
    interval: std::time::Duration,
}

impl EventWorker {
    pub fn new(
        engine: LedgerEngine,
        consumer: LedgerEventConsumer,
        interval: std::time::Duration,
    ) -> Self {
        EventWorker {
            engine,
            consumer,
            interval,
        }
    }

    pub async fn post_transfer(
        &self,
        req: TransferRequest,
    ) -> Result<TransferResult, crate::engine::EngineError> {
        let payment = self
            .engine
            .post_payment(PaymentRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                journal_type: JournalType::P2p,
                currency: req.currency,
                origin: req.origin,
                payer_account_id: req.sender_account_id,
                payee_account_id: req.recipient_account_id,
                amount_minor: req.amount_minor,
                fee: FeePolicy { bps: req.fee_bps },
                tax: None,
                agent_commission: None,
            })
            .await?;

        Ok(TransferResult {
            journal_id: payment.journal_id,
            fee_minor: payment.fee_minor,
        })
    }

    pub async fn run_once(&self, limit: i32) -> Result<Vec<EventAudit>, sqlx::Error> {
        self.consumer.poll_once(limit).await
    }

    pub fn spawn(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(self.interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let _ = self.run_once(100).await;
                    }
                    _ = shutdown.changed() => break,
                }
            }
        })
    }
}
