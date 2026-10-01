//! ANBARR PAY ledger engine.
//!
//! The Rust component owns the double-entry ledger. Everything here is
//! correctness-critical: amounts are integers, balances are derived, writes are
//! serialized by row locks in canonical order, and idempotency is enforced at the
//! database level.

pub mod api;
pub mod balances;
pub mod db;
pub mod engine;

/// Generated protobuf/gRPC code for the ledger contract. `build.rs` compiles
/// `proto/ledger.proto` via protox (pure Rust — no system protoc needed).
pub mod grpc_proto {
    include!(concat!(env!("OUT_DIR"), "/anbarr.ledger.v1.rs"));
}
pub mod grpc;

pub mod money;
pub mod outbox;
pub mod reconcile;
pub mod sandbox;
pub mod service;
pub mod types;

pub use api::{create_app, TransferApiRequest, TransferApiResponse};
pub use engine::{
    AgentCommission, CaptureRequest, CreateAccountRequest, CreateAccountResult, FeePolicy,
    HoldRequest, HoldResult, LedgerEngine, PaymentRequest, PaymentResult, RefundRequest,
    ReleaseRequest, ReversalRequest,
};
pub use money::{round_fee, BasisPoints, Currency, MinorUnits};
pub use outbox::{EventAudit, EventWorker, LedgerEvent, LedgerEventConsumer};
pub use reconcile::{
    AlertSink, Rail, RailStatementSource, ReconcileReport, ReconcileScheduler, ReconcileService,
    ReconcileTask,
};
pub use sandbox::{
    SandboxPayment, SandboxRail, SandboxScenario, SandboxSource, SandboxWebhook, WebhookAcceptance,
    SANDBOX_PAYMENT_LABEL, SANDBOX_REF_PREFIX,
};
pub use service::{LedgerService, TransferRequest, TransferResult};
pub use types::{
    AccountType, Direction, JournalResult, JournalSpec, JournalStatus, JournalType, Leg, Origin,
};

/// Build the legs of a payment journal with the platform fee charged to the
/// paying party (see `engine::payment_legs`).
pub use engine::payment_legs;

/// The engine's funds rule as a pure function (see `engine::check_funds`).
pub use engine::check_funds;
