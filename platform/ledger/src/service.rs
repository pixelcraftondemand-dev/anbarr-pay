//! Minimal runtime boundary for the ledger engine.
//!
//! This is intentionally small: it exposes the engine to the application layer in
//! a way that matches the project’s API design, with a first end-to-end transfer
//! flow and strict idempotency semantics.

use crate::engine::{LedgerEngine, PaymentRequest, PaymentResult};
use crate::money::{BasisPoints, Currency, MinorUnits};
use crate::types::Origin;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TransferRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub currency: Currency,
    pub origin: Origin,
    pub sender_account_id: Uuid,
    pub recipient_account_id: Uuid,
    pub amount_minor: MinorUnits,
    pub fee_bps: BasisPoints,
    /// Government transaction tax (e-levy-style), basis points of the amount.
    /// `None` = no tax in this jurisdiction/configuration.
    pub tax_bps: Option<BasisPoints>,
}

#[derive(Debug, Clone)]
pub struct TransferResult {
    pub journal_id: Uuid,
    pub fee_minor: MinorUnits,
    pub tax_minor: MinorUnits,
    pub commission_minor: MinorUnits,
    pub commission_journal_id: Option<Uuid>,
}

impl From<PaymentResult> for TransferResult {
    fn from(result: PaymentResult) -> Self {
        TransferResult {
            journal_id: result.journal_id,
            fee_minor: result.fee_minor,
            tax_minor: result.tax_minor,
            commission_minor: result.commission_minor,
            commission_journal_id: result.commission_journal_id,
        }
    }
}

#[derive(Clone)]
pub struct LedgerService {
    engine: LedgerEngine,
}

impl LedgerService {
    pub fn new(engine: LedgerEngine) -> Self {
        LedgerService { engine }
    }

    pub async fn post_transfer(
        &self,
        req: TransferRequest,
    ) -> Result<TransferResult, crate::engine::EngineError> {
        let result = self
            .engine
            .post_payment(PaymentRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                journal_type: crate::types::JournalType::P2p,
                currency: req.currency,
                origin: req.origin,
                payer_account_id: req.sender_account_id,
                payee_account_id: req.recipient_account_id,
                amount_minor: req.amount_minor,
                fee: crate::engine::FeePolicy { bps: req.fee_bps },
                tax: req.tax_bps.map(|bps| crate::engine::TaxPolicy { bps }),
                agent_commission: None,
            })
            .await?;

        Ok(result.into())
    }
}
