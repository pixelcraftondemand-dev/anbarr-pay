//! Ledger domain types. Sum types make illegal states unrepresentable.

use crate::money::{Currency, MinorUnits};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Classification of a journal. `Hold`/`Capture`/`Release` are the holds lifecycle;
/// the rest are business transaction types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JournalType {
    Hold,
    Capture,
    Release,
    P2p,
    Topup,
    CashIn,
    CashOut,
    Checkout,
    Fee,
    /// Platform pays an agent commission out of fee revenue into the agent's float.
    Commission,
    /// Full reversal of a posted journal: every leg negated, reference to the original.
    Reversal,
    /// Partial or full refund of a payment: payee returns principal to the payer,
    /// reference to the original payment journal.
    Refund,
    Adjustment,
}

impl JournalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JournalType::Hold => "hold",
            JournalType::Capture => "capture",
            JournalType::Release => "release",
            JournalType::P2p => "p2p",
            JournalType::Topup => "topup",
            JournalType::CashIn => "cash_in",
            JournalType::CashOut => "cash_out",
            JournalType::Checkout => "checkout",
            JournalType::Fee => "fee",
            JournalType::Commission => "commission",
            JournalType::Reversal => "reversal",
            JournalType::Refund => "refund",
            JournalType::Adjustment => "adjustment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalStatus {
    Pending,
    Posted,
    Void,
}

impl JournalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JournalStatus::Pending => "pending",
            JournalStatus::Posted => "posted",
            JournalStatus::Void => "void",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Debit,
    Credit,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Debit => "debit",
            Direction::Credit => "credit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Wallet,
    Float,
    HoldEscrow,
    FeeRevenue,
    PlatformRevenue,
    RailBridge,
    /// Government transaction tax (e.g. an e-levy-style levy): collected from
    /// the payer and owed to the tax authority. A liability, never revenue.
    TaxPayable,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Wallet => "wallet",
            AccountType::Float => "float",
            AccountType::HoldEscrow => "hold_escrow",
            AccountType::FeeRevenue => "fee_revenue",
            AccountType::PlatformRevenue => "platform_revenue",
            AccountType::RailBridge => "rail_bridge",
            AccountType::TaxPayable => "tax_payable",
        }
    }
}

/// Full origin traceability for every journal (the "ownership graph"):
/// who initiated, through which channel/session, under which payment code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Origin {
    pub user_id: Option<Uuid>,
    pub channel: String,
    pub session_id: Option<String>,
    pub payment_code: Option<String>,
}

/// One double-entry leg. Amounts must be > 0; direction is explicit so
/// "negative debits" cannot occur.
#[derive(Debug, Clone, Copy)]
pub struct Leg {
    pub account_id: Uuid,
    pub direction: Direction,
    pub amount_minor: MinorUnits,
}

/// A journal with its legs, ready for posting.
#[derive(Debug, Clone)]
pub struct JournalSpec {
    pub journal_type: JournalType,
    pub currency: Currency,
    pub origin: Origin,
    pub legs: Vec<Leg>,
}

/// Identity of an account row as read from the DB.
#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: Uuid,
    pub owner_type: String,
    pub owner_id: Option<Uuid>,
    pub account_type: AccountType,
    pub currency: Currency,
    pub name: String,
    pub status: String,
}

/// Result of a successful posting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalResult {
    pub journal_id: Uuid,
    pub status: JournalStatus,
}

impl JournalResult {
    pub fn posted(journal_id: Uuid) -> Self {
        JournalResult {
            journal_id,
            status: JournalStatus::Posted,
        }
    }
}

/// Idempotency cache entry. The cache is an optimization + in-flight marker;
/// the `journals` row (unique on (scope, key), carrying `request_hash`) is the
/// durable source of truth for replay even after the cache row expires/prunes.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    pub status: String, // "in_progress" | "done"
    pub request_hash: String,
    pub response: Option<serde_json::Value>,
    /// NULL means the record never expires (legacy rows).
    pub expires_at: Option<DateTime<Utc>>,
}

/// Hold row as read from the DB.
#[derive(Debug, Clone)]
pub struct HoldRow {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub account_id: Uuid,
    pub hold_escrow_account_id: Uuid,
    pub amount_minor: MinorUnits,
    pub currency: Currency,
    pub status: String,
    pub expires_at: DateTime<Utc>,
}
