//! gRPC boundary (architecture §8): the versioned protobuf contract the Java
//! application layer talks to. This module is a *mapping layer only* — every
//! financial decision stays in `engine.rs`. Error mapping is centralized in
//! `to_status`, so internal details never leak to callers.
//!
//! Transport auth: the server requires a shared secret on every call
//! (`x-ledger-token`), checked in an interceptor. The secret comes from the
//! `LEDGER_GRPC_TOKEN` env (secrets manager in deployment — never the repo);
//! mTLS is the production upgrade path (architecture §7, service-to-service
//! security).

use crate::balances;
use crate::engine::{
    CaptureRequest, EngineError, HoldRequest, LedgerEngine, RefundRequest, ReleaseRequest,
    ReversalRequest,
};
// tonic-build generates a flat namespace: the `grpc_proto` module *is*
// `anbarr.ledger.v1`.
use crate::grpc_proto as pb;
use crate::grpc_proto::ledger_server::Ledger;
use crate::money::Currency;
use crate::types::{AccountType, JournalType, Origin};
use chrono::{DateTime, Utc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub use crate::grpc_proto::ledger_client::LedgerClient;
pub use crate::grpc_proto::ledger_server::{Ledger as GrpcLedger, LedgerServer};

/// Interceptor: reject calls without the required shared secret.
/// Mirrors main.rs `grpc_token_from_env`: with APP_ENV=dev and no explicit
/// token, the well-known local-dev secret is accepted so the server does not
/// start successfully yet reject every request.
fn grpc_token() -> Result<String, Status> {
    match std::env::var("LEDGER_GRPC_TOKEN") {
        Ok(token) if !token.trim().is_empty() => Ok(token),
        Ok(_) => Err(Status::unauthenticated(
            "LEDGER_GRPC_TOKEN is set but empty; refusing to authenticate calls",
        )),
        Err(_) if std::env::var("APP_ENV").as_deref() == Ok("dev") => {
            Ok("anbarrpay-internal-dev".to_string())
        }
        Err(_) => Err(Status::unauthenticated(
            "missing ledger token configuration: set LEDGER_GRPC_TOKEN (or APP_ENV=dev for local-only testing)",
        )),
    }
}

fn check_auth<T>(req: &Request<T>) -> Result<(), Status> {
    let expected = grpc_token()?;
    let provided = req
        .metadata()
        .get("x-ledger-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided.is_empty() {
        return Err(Status::unauthenticated("missing ledger token"));
    }
    // Near-constant-time compare to reduce timing side channels.
    let (a, b) = (provided.as_bytes(), expected.as_bytes());
    let ok = a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0;
    if !ok {
        return Err(Status::unauthenticated("invalid ledger token"));
    }
    Ok(())
}

/// Engine errors → gRPC status codes, safe for internal callers: the message
/// carries the engine's domain text (no SQL, no stack), and the code is
/// machine-actable so the Java layer can translate to its own error contract.
pub fn to_status(err: EngineError) -> Status {
    let text = err.to_string();
    match &err {
        EngineError::InvalidJournal(_)
        | EngineError::InvalidAccount(_)
        | EngineError::Unbalanced { .. } => Status::invalid_argument(text),
        EngineError::IdempotencyMismatch { .. }
        | EngineError::IdempotencyInProgress { .. }
        | EngineError::InsufficientFunds { .. } => Status::failed_precondition(text),
        EngineError::AccountNotFound(_)
        | EngineError::JournalNotFound(_)
        | EngineError::HoldNotFound(_) => Status::not_found(text),
        EngineError::AccountNotActive(_)
        | EngineError::JournalNotPosted { .. }
        | EngineError::HoldNotHeld { .. }
        | EngineError::HoldExpired(_)
        | EngineError::AlreadyReversed(_)
        | EngineError::HasRefunds(_)
        | EngineError::JournalReversed(_)
        | EngineError::NotReversible { .. }
        | EngineError::NotRefundable { .. }
        | EngineError::RefundFromMismatch { .. }
        | EngineError::RefundExceedsPrincipal { .. } => Status::failed_precondition(text),
        EngineError::PlatformAccountMissing { .. } => Status::internal(text),
        // Never surface SQL details over the wire.
        EngineError::Db(_) => Status::unavailable("ledger storage unavailable"),
    }
}

/// Read-model queries surface raw sqlx errors; same rule as the engine's
/// `Db` variant: never leak SQL details over the wire.
fn db_status(_err: sqlx::Error) -> Status {
    Status::unavailable("ledger storage unavailable")
}

fn parse_uuid(field: &str, value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| Status::invalid_argument(format!("{field} must be a UUID")))
}

fn parse_currency(value: &str) -> Result<Currency, Status> {
    Currency::parse(value)
        .ok_or_else(|| Status::invalid_argument(format!("unknown currency {value}")))
}

fn parse_journal_type(value: &str) -> Result<JournalType, Status> {
    match value {
        "p2p" => Ok(JournalType::P2p),
        "topup" => Ok(JournalType::Topup),
        "cash_in" => Ok(JournalType::CashIn),
        "cash_out" => Ok(JournalType::CashOut),
        "checkout" => Ok(JournalType::Checkout),
        other => Err(Status::invalid_argument(format!(
            "unsupported transfer journal_type {other}"
        ))),
    }
}

fn parse_account_type(value: &str) -> Result<AccountType, Status> {
    match value {
        "wallet" => Ok(AccountType::Wallet),
        "float" => Ok(AccountType::Float),
        other => Err(Status::invalid_argument(format!(
            "unsupported account_type {other}"
        ))),
    }
}

fn origin_from(pb_org: Option<pb::Origin>, fallback_channel: &str) -> Origin {
    pb_org
        .map(|o| Origin {
            user_id: Uuid::parse_str(&o.user_id).ok(),
            channel: if o.channel.is_empty() {
                fallback_channel.to_string()
            } else {
                o.channel
            },
            session_id: if o.session_id.is_empty() {
                None
            } else {
                Some(o.session_id)
            },
            payment_code: if o.payment_code.is_empty() {
                None
            } else {
                Some(o.payment_code)
            },
        })
        .unwrap_or(Origin {
            user_id: None,
            channel: fallback_channel.to_string(),
            session_id: None,
            payment_code: None,
        })
}

fn ts_to_datetime(
    ts: Option<prost_types::Timestamp>,
    field: &str,
) -> Result<DateTime<Utc>, Status> {
    let ts = ts.ok_or_else(|| Status::invalid_argument(format!("{field} is required")))?;
    DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
        .ok_or_else(|| Status::invalid_argument(format!("{field} is not a valid time")))
}

fn datetime_to_ts(dt: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

/// The §8 Ledger service, backed by the engine.
pub struct LedgerGrpc {
    engine: LedgerEngine,
}

impl LedgerGrpc {
    pub fn new(engine: LedgerEngine) -> Self {
        LedgerGrpc { engine }
    }

    /// Wrap into a tonic server with the auth interceptor applied to every call.
    pub fn into_server(self) -> LedgerServer<impl GrpcLedger> {
        LedgerServer::new(self)
    }
}

#[tonic::async_trait]
impl Ledger for LedgerGrpc {
    async fn hold_funds(
        &self,
        request: Request<pb::HoldRequest>,
    ) -> Result<Response<pb::HoldResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let expires_at = ts_to_datetime(req.expires_at, "expires_at")?;
        let result = self
            .engine
            .hold_funds(HoldRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                account_id: parse_uuid("account_id", &req.account_id)?,
                amount_minor: req.amount_minor,
                currency: parse_currency(&req.currency)?,
                expires_at,
                origin: origin_from(req.origin, "grpc"),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::HoldResponse {
            hold_id: result.hold_id.to_string(),
            journal_id: result.journal_id.to_string(),
        }))
    }

    async fn capture_hold(
        &self,
        request: Request<pb::CaptureRequest>,
    ) -> Result<Response<pb::JournalResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let result = self
            .engine
            .capture_hold(CaptureRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                hold_id: parse_uuid("hold_id", &req.hold_id)?,
                target_account_id: parse_uuid("target_account_id", &req.target_account_id)?,
                origin: origin_from(req.origin, "grpc"),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::JournalResponse {
            journal_id: result.journal_id.to_string(),
            status: result.status.as_str().to_string(),
        }))
    }

    async fn release_hold(
        &self,
        request: Request<pb::ReleaseRequest>,
    ) -> Result<Response<pb::JournalResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let result = self
            .engine
            .release_hold(ReleaseRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                hold_id: parse_uuid("hold_id", &req.hold_id)?,
                origin: origin_from(req.origin, "grpc"),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::JournalResponse {
            journal_id: result.journal_id.to_string(),
            status: result.status.as_str().to_string(),
        }))
    }

    async fn post_transfer(
        &self,
        request: Request<pb::TransferRequest>,
    ) -> Result<Response<pb::TransferResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let result = self
            .engine
            .post_payment(crate::engine::PaymentRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                journal_type: parse_journal_type(&req.journal_type)?,
                currency: parse_currency(&req.currency)?,
                origin: origin_from(req.origin, "grpc"),
                payer_account_id: parse_uuid("payer_account_id", &req.payer_account_id)?,
                payee_account_id: parse_uuid("payee_account_id", &req.payee_account_id)?,
                amount_minor: req.amount_minor,
                fee: crate::engine::FeePolicy { bps: req.fee_bps },
                tax: if req.tax_bps == 0 {
                    None
                } else {
                    Some(crate::engine::TaxPolicy { bps: req.tax_bps })
                },
                agent_commission: None,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::TransferResponse {
            journal_id: result.journal_id.to_string(),
            fee_minor: result.fee_minor,
            tax_minor: result.tax_minor,
            commission_minor: result.commission_minor,
            commission_journal_id: result
                .commission_journal_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
        }))
    }

    async fn post_fee(
        &self,
        request: Request<pb::FeeRequest>,
    ) -> Result<Response<pb::JournalResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let amount_minor = req.amount_minor;
        let currency = parse_currency(&req.currency)?;
        let fee = crate::money::round_fee(amount_minor, req.fee_bps);
        if fee <= 0 {
            return Err(Status::invalid_argument(
                "fee rounds to zero for the given amount/bps",
            ));
        }
        let result = self
            .engine
            .post_journal(
                &req.idempotency_scope,
                &req.idempotency_key,
                crate::types::JournalSpec {
                    journal_type: JournalType::Fee,
                    currency,
                    origin: origin_from(req.origin, "grpc"),
                    legs: vec![
                        crate::types::Leg {
                            account_id: parse_uuid("debit_account_id", &req.debit_account_id)?,
                            direction: crate::types::Direction::Debit,
                            amount_minor: fee,
                        },
                        crate::types::Leg {
                            account_id: parse_uuid(
                                "fee_revenue_account_id",
                                &req.fee_revenue_account_id,
                            )?,
                            direction: crate::types::Direction::Credit,
                            amount_minor: fee,
                        },
                    ],
                },
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::JournalResponse {
            journal_id: result.journal_id.to_string(),
            status: result.status.as_str().to_string(),
        }))
    }

    async fn reverse_journal(
        &self,
        request: Request<pb::ReverseRequest>,
    ) -> Result<Response<pb::JournalResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let result = self
            .engine
            .reverse_journal(ReversalRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                original_journal_id: parse_uuid("original_journal_id", &req.original_journal_id)?,
                origin: origin_from(req.origin, "grpc"),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::JournalResponse {
            journal_id: result.journal_id.to_string(),
            status: result.status.as_str().to_string(),
        }))
    }

    async fn refund_payment(
        &self,
        request: Request<pb::RefundRequest>,
    ) -> Result<Response<pb::JournalResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let result = self
            .engine
            .refund_payment(RefundRequest {
                idempotency_scope: req.idempotency_scope,
                idempotency_key: req.idempotency_key,
                original_journal_id: parse_uuid("original_journal_id", &req.original_journal_id)?,
                refund_from_account_id: parse_uuid(
                    "refund_from_account_id",
                    &req.refund_from_account_id,
                )?,
                refund_to_account_id: parse_uuid(
                    "refund_to_account_id",
                    &req.refund_to_account_id,
                )?,
                amount_minor: req.amount_minor,
                origin: origin_from(req.origin, "grpc"),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::JournalResponse {
            journal_id: result.journal_id.to_string(),
            status: result.status.as_str().to_string(),
        }))
    }

    async fn create_account(
        &self,
        request: Request<pb::CreateAccountRequest>,
    ) -> Result<Response<pb::CreateAccountResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let owner_id = if req.owner_id.is_empty() {
            None
        } else {
            Some(parse_uuid("owner_id", &req.owner_id)?)
        };
        let result = self
            .engine
            .create_account(crate::engine::CreateAccountRequest {
                owner_type: req.owner_type,
                owner_id,
                account_type: parse_account_type(&req.account_type)?,
                currency: parse_currency(&req.currency)?,
                name: req.name,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::CreateAccountResponse {
            account_id: result.account_id.to_string(),
            created: result.created,
            status: result.status,
        }))
    }

    async fn get_account(
        &self,
        request: Request<pb::AccountRequest>,
    ) -> Result<Response<pb::AccountResponse>, Status> {
        check_auth(&request)?;
        let req = request.into_inner();
        let account_id = parse_uuid("account_id", &req.account_id)?;
        let balance = balances::get_balance(self.engine.pool(), account_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::AccountResponse {
            account_id: balance.account_id.to_string(),
            currency: balance.currency.as_str().to_string(),
            available_minor: balance.available_minor,
            held_minor: balance.held_minor,
            total_minor: balance.total_minor,
        }))
    }

    async fn list_entries(
        &self,
        request: Request<pb::ListRequest>,
    ) -> Result<Response<pb::ListResponse>, Status> {
        check_auth(&request)?;
        const MAX_PAGE: i64 = 200;
        let req = request.into_inner();
        let account_id = parse_uuid("account_id", &req.account_id)?;
        let limit = req.limit.clamp(1, MAX_PAGE as i32) as i64;

        // Keyset pagination on (created_at, id) so paging is stable while new
        // entries land. The token is `<secs>-<nanos>-<uuid>` (opaque to callers).
        let cursor: Option<(DateTime<Utc>, Uuid)> = if req.page_token.is_empty() {
            None
        } else {
            let parts: Vec<&str> = req.page_token.splitn(3, '-').collect();
            if parts.len() != 3 {
                return Err(Status::invalid_argument("invalid page_token"));
            }
            let secs: i64 = parts[0]
                .parse()
                .map_err(|_| Status::invalid_argument("invalid page_token"))?;
            let nanos: u32 = parts[1]
                .parse()
                .map_err(|_| Status::invalid_argument("invalid page_token"))?;
            let id = Uuid::parse_str(parts[2])
                .map_err(|_| Status::invalid_argument("invalid page_token"))?;
            let ts = DateTime::from_timestamp(secs, nanos)
                .ok_or_else(|| Status::invalid_argument("invalid page_token"))?;
            Some((ts, id))
        };

        let rows: Vec<(Uuid, Uuid, String, String, i64, String, chrono::DateTime<Utc>)> =
            match cursor {
                Some((ts, id)) => {
                    sqlx::query_as(
                        "SELECT e.id, e.journal_id, j.type, e.direction, e.amount_minor, e.currency, e.created_at \
                         FROM entries e JOIN journals j ON j.id = e.journal_id \
                         WHERE e.account_id = $1 AND (e.created_at, e.id) < ($2, $3) \
                         ORDER BY e.created_at DESC, e.id DESC LIMIT $4",
                    )
                    .bind(account_id)
                    .bind(ts)
                    .bind(id)
                    .bind(limit)
                    .fetch_all(self.engine.pool())
                    .await
                    .map_err(db_status)?
                }
                None => {
                    sqlx::query_as(
                        "SELECT e.id, e.journal_id, j.type, e.direction, e.amount_minor, e.currency, e.created_at \
                         FROM entries e JOIN journals j ON j.id = e.journal_id \
                         WHERE e.account_id = $1 \
                         ORDER BY e.created_at DESC, e.id DESC LIMIT $2",
                    )
                    .bind(account_id)
                    .bind(limit)
                    .fetch_all(self.engine.pool())
                    .await
                    .map_err(db_status)?
                }
            };

        let next_page_token = rows
            .last()
            .map(|(entry_id, _, _, _, _, _, created_at)| {
                format!(
                    "{}-{}-{}",
                    created_at.timestamp(),
                    created_at.timestamp_subsec_nanos(),
                    entry_id
                )
            })
            .unwrap_or_default();

        Ok(Response::new(pb::ListResponse {
            entries: rows
                .into_iter()
                .map(
                    |(
                        entry_id,
                        journal_id,
                        journal_type,
                        direction,
                        amount_minor,
                        currency,
                        created_at,
                    )| {
                        pb::list_response::Entry {
                            entry_id: entry_id.to_string(),
                            journal_id: journal_id.to_string(),
                            journal_type,
                            direction,
                            amount_minor,
                            currency,
                            created_at: Some(datetime_to_ts(created_at)),
                        }
                    },
                )
                .collect(),
            next_page_token,
        }))
    }

    async fn audit_accounts(
        &self,
        request: Request<pb::AuditRequest>,
    ) -> Result<Response<pb::AuditResponse>, Status> {
        check_auth(&request)?;
        let drifted = balances::audit_snapshots(self.engine.pool())
            .await
            .map_err(to_status)?;
        Ok(Response::new(pb::AuditResponse {
            drifted_account_ids: drifted.into_iter().map(|id| id.to_string()).collect(),
        }))
    }
}
