//! Re-export of the generated `amber.ledger.v1` protobuf/gRPC types from the
//! ledger crate. The ledger crate compiles `proto/ledger.proto` in its
//! `build.rs`; the Core API never redefines the contract — it consumes the
//! same generated module, so the two sides can never drift.

pub use anbarr_ledger::grpc_proto as pb;
pub use anbarr_ledger::grpc_proto::ledger_client::LedgerClient as PbLedgerClient;
