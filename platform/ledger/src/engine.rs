//! The ledger engine — the only component that mutates ledger state.
//!
//! Correctness model:
//! - **Single writer**: every balance mutation happens in one DB transaction here.
//! - **Pessimistic locking**: `accounts` rows are locked (`FOR UPDATE`) in canonical
//!   (sorted) order, so concurrent operations on shared accounts serialize and
//!   deadlocks are structurally impossible.
//! - **Derived balances**: no stored balances; available funds are summed from
//!   `entries` under the account locks before validating and posting.
//! - **Idempotency**: unique `(scope, key)` on `journals` is the hard backstop; the
//!   `idempotency_keys` cache additionally replays the stored response. Duplicates
//!   can never double-post.
//! - **Immutability**: entries are never updated or deleted; corrections are new
//!   journals.

use crate::balances;
use crate::money::{round_fee, BasisPoints, Currency, MinorUnits};
use crate::types::{
    AccountRow, AccountType, Direction, HoldRow, IdempotencyRecord, JournalResult, JournalSpec,
    JournalType, Leg, Origin,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

/// Idempotency cache keys expire after this long. Expiry is not a safety
/// boundary — the `journals` (scope, key) row is the durable backstop and
/// replay falls back to it — it only bounds the cache's lifetime.
const IDEMPOTENCY_KEY_TTL: chrono::Duration = chrono::Duration::hours(24);

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("idempotency key {scope}:{key} is already being processed")]
    IdempotencyInProgress { scope: String, key: String },
    #[error("idempotency key {scope}:{key} was reused with a different request")]
    IdempotencyMismatch { scope: String, key: String },
    #[error("journal {0} not found")]
    JournalNotFound(Uuid),
    #[error("journal {journal_id} is not posted (status: {status})")]
    JournalNotPosted { journal_id: Uuid, status: String },
    #[error("journal {0} has already been reversed")]
    AlreadyReversed(Uuid),
    #[error("journal {0} has refunds and cannot be reversed")]
    HasRefunds(Uuid),
    #[error("journal {0} was reversed and cannot be refunded")]
    JournalReversed(Uuid),
    #[error("journal {journal_id} of type {journal_type} cannot be reversed")]
    NotReversible {
        journal_id: Uuid,
        journal_type: String,
    },
    #[error("journal {journal_id} of type {journal_type} cannot be refunded")]
    NotRefundable {
        journal_id: Uuid,
        journal_type: String,
    },
    #[error(
        "refund source for journal {journal_id} must be its payee ({expected}), got {provided}"
    )]
    RefundFromMismatch {
        journal_id: Uuid,
        expected: Uuid,
        provided: Uuid,
    },
    #[error("refund of {requested} exceeds remaining principal (already refunded {refunded} of {principal})")]
    RefundExceedsPrincipal {
        requested: MinorUnits,
        refunded: MinorUnits,
        principal: MinorUnits,
    },
    #[error("account {0} not found")]
    AccountNotFound(Uuid),
    #[error("account {0} is not active")]
    AccountNotActive(Uuid),
    #[error("platform account missing: type={account_type:?} currency={currency:?}")]
    PlatformAccountMissing {
        account_type: AccountType,
        currency: Currency,
    },
    #[error("invalid journal: {0}")]
    InvalidJournal(String),
    #[error("journal does not balance: debits {debits} != credits {credits}")]
    Unbalanced {
        debits: MinorUnits,
        credits: MinorUnits,
    },
    #[error("insufficient funds on account {account}: need {need}, available {available}")]
    InsufficientFunds {
        account: Uuid,
        need: MinorUnits,
        available: MinorUnits,
    },
    #[error("hold {0} not found")]
    HoldNotFound(Uuid),
    #[error("hold {hold_id} is not in state held (current: {current})")]
    HoldNotHeld { hold_id: Uuid, current: String },
    #[error("hold {0} has expired")]
    HoldExpired(Uuid),
    #[error("invalid account: {0}")]
    InvalidAccount(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// A hold-lifecycle mutation applied alongside a journal.
///
/// Capture/Release/Expire carry `wallet_account_id`: the hold's status change
/// affects the payer wallet's held balance even though the journal doesn't touch
/// the wallet (a capture credits escrow + merchant only). The wallet must be
/// locked and its snapshot rebuilt in the same transaction, or its held_minor
/// goes stale (the audit job would flag it).
enum HoldAction {
    Insert {
        hold_id: Uuid,
        escrow_account_id: Uuid,
        expires_at: DateTime<Utc>,
    },
    Capture {
        hold_id: Uuid,
        wallet_account_id: Uuid,
    },
    Release {
        hold_id: Uuid,
        wallet_account_id: Uuid,
    },
    Expire {
        hold_id: Uuid,
        wallet_account_id: Uuid,
    },
}

/// The engine. Cheap to clone; wrap one `PgPool`.
#[derive(Clone)]
pub struct LedgerEngine {
    pool: PgPool,
}

impl LedgerEngine {
    /// Read access to the backing pool for read-model queries (balance
    /// reads, statement paging, audit recompute) that don't mutate ledger
    /// state. Writes still go through the engine methods only.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn new(pool: PgPool) -> Self {
        LedgerEngine { pool }
    }

    /// Idempotently post a journal. On a duplicate `(scope, key)` with the same
    /// request, returns the original result without posting again.
    pub async fn post_journal(
        &self,
        idempotency_scope: &str,
        idempotency_key: &str,
        spec: JournalSpec,
    ) -> Result<JournalResult, EngineError> {
        validate_spec(&spec)?;

        let mut tx = self.pool.begin().await?;
        match self
            .run_with_idempotency(&mut tx, idempotency_scope, idempotency_key, &spec, None)
            .await
        {
            Ok(result) => {
                tx.commit().await?;
                Ok(result)
            }
            Err(e) => {
                tx.rollback().await?;
                Err(e)
            }
        }
    }

    /// Reserve funds on a wallet. Creates a `hold` journal (debit wallet /
    /// credit hold_escrow) and a `holds` row. The hold expires at `expires_at`.
    pub async fn hold_funds(&self, req: HoldRequest) -> Result<HoldResult, EngineError> {
        if req.amount_minor <= 0 {
            return Err(EngineError::InvalidJournal(
                "hold amount must be > 0".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;

        // The hold journal needs the platform escrow account for this currency.
        let escrow = self
            .find_platform_account(&mut tx, AccountType::HoldEscrow, req.currency)
            .await?;

        let spec = JournalSpec {
            journal_type: JournalType::Hold,
            currency: req.currency,
            origin: req.origin,
            legs: vec![
                Leg {
                    account_id: req.account_id,
                    direction: Direction::Debit,
                    amount_minor: req.amount_minor,
                },
                Leg {
                    account_id: escrow.id,
                    direction: Direction::Credit,
                    amount_minor: req.amount_minor,
                },
            ],
        };
        validate_spec(&spec)?;

        let hold_id = Uuid::new_v4(); // tentative; only used on the fresh path
        let action = HoldAction::Insert {
            hold_id,
            escrow_account_id: escrow.id,
            expires_at: req.expires_at,
        };
        let result = self
            .run_with_idempotency(
                &mut tx,
                &req.idempotency_scope,
                &req.idempotency_key,
                &spec,
                Some(action),
            )
            .await?;

        // The real hold id (fresh path: the one we generated; replay: the original).
        let actual_hold_id: Uuid = sqlx::query_scalar("SELECT id FROM holds WHERE journal_id = $1")
            .bind(result.journal_id)
            .fetch_one(&mut *tx)
            .await?;

        // Overwrite the generic response with one including the hold id.
        set_idempotency_response(
            &mut tx,
            &req.idempotency_scope,
            &req.idempotency_key,
            json!({ "hold_id": actual_hold_id, "journal_id": result.journal_id }),
        )
        .await?;
        tx.commit().await?;
        Ok(HoldResult {
            hold_id: actual_hold_id,
            journal_id: result.journal_id,
        })
    }

    /// Settle a hold: debit hold_escrow, credit `target_account_id`, mark the
    /// hold captured. The hold must still be `held` and unexpired.
    pub async fn capture_hold(&self, req: CaptureRequest) -> Result<JournalResult, EngineError> {
        let mut tx = self.pool.begin().await?;

        let hold = self.lock_hold(&mut tx, req.hold_id).await?;
        match hold.status.as_str() {
            "held" => {}
            other => {
                tx.rollback().await?;
                return Err(EngineError::HoldNotHeld {
                    hold_id: req.hold_id,
                    current: other.into(),
                });
            }
        }
        if hold.expires_at <= Utc::now() {
            tx.rollback().await?;
            return Err(EngineError::HoldExpired(req.hold_id));
        }

        let spec = JournalSpec {
            journal_type: JournalType::Capture,
            currency: hold.currency,
            origin: req.origin,
            legs: vec![
                Leg {
                    account_id: hold.hold_escrow_account_id,
                    direction: Direction::Debit,
                    amount_minor: hold.amount_minor,
                },
                Leg {
                    account_id: req.target_account_id,
                    direction: Direction::Credit,
                    amount_minor: hold.amount_minor,
                },
            ],
        };
        validate_spec(&spec)?;

        let result = match self
            .run_with_idempotency(
                &mut tx,
                &req.idempotency_scope,
                &req.idempotency_key,
                &spec,
                Some(HoldAction::Capture {
                    hold_id: req.hold_id,
                    wallet_account_id: hold.account_id,
                }),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tx.rollback().await?;
                return Err(e);
            }
        };
        tx.commit().await?;
        Ok(result)
    }

    /// Cancel a hold: debit hold_escrow, credit the original wallet, mark the
    /// hold released.
    pub async fn release_hold(&self, req: ReleaseRequest) -> Result<JournalResult, EngineError> {
        let mut tx = self.pool.begin().await?;

        let hold = self.lock_hold(&mut tx, req.hold_id).await?;
        match hold.status.as_str() {
            "held" => {}
            other => {
                tx.rollback().await?;
                return Err(EngineError::HoldNotHeld {
                    hold_id: req.hold_id,
                    current: other.into(),
                });
            }
        }

        let spec = JournalSpec {
            journal_type: JournalType::Release,
            currency: hold.currency,
            origin: req.origin,
            legs: vec![
                Leg {
                    account_id: hold.hold_escrow_account_id,
                    direction: Direction::Debit,
                    amount_minor: hold.amount_minor,
                },
                Leg {
                    account_id: hold.account_id,
                    direction: Direction::Credit,
                    amount_minor: hold.amount_minor,
                },
            ],
        };
        validate_spec(&spec)?;

        let result = match self
            .run_with_idempotency(
                &mut tx,
                &req.idempotency_scope,
                &req.idempotency_key,
                &spec,
                Some(HoldAction::Release {
                    hold_id: req.hold_id,
                    wallet_account_id: hold.account_id,
                }),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tx.rollback().await?;
                return Err(e);
            }
        };
        tx.commit().await?;
        Ok(result)
    }

    /// Provision a customer/merchant/agent account (wallet or float).
    /// Idempotent: the UNIQUE (owner_type, owner_id, type, currency) constraint
    /// makes a repeated request return the existing account.
    ///
    /// Platform accounts (hold_escrow, fee_revenue, platform_revenue,
    /// rail_bridge) are deliberately NOT creatable here: they are seeded by
    /// migration 0002 and must stay singletons, or the engine's platform
    /// account resolution (find by type + currency) would become ambiguous.
    pub async fn create_account(
        &self,
        req: CreateAccountRequest,
    ) -> Result<CreateAccountResult, EngineError> {
        match req.owner_type.as_str() {
            "user" | "merchant" | "agent" => {}
            other => {
                return Err(EngineError::InvalidAccount(format!(
                    "owner_type must be user|merchant|agent, got {other}"
                )));
            }
        }
        match req.account_type {
            AccountType::Wallet | AccountType::Float => {}
            other => {
                return Err(EngineError::InvalidAccount(format!(
                    "{other:?} accounts are platform-managed; only wallet|float can be created via this API"
                )));
            }
        }
        if req.name.is_empty() {
            return Err(EngineError::InvalidAccount("name must not be empty".into()));
        }

        // Existing account (idempotent replay)?
        let existing = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, status FROM accounts
             WHERE owner_type = $1 AND owner_id IS NOT DISTINCT FROM $2
               AND type = $3 AND currency = $4",
        )
        .bind(&req.owner_type)
        .bind(req.owner_id)
        .bind(req.account_type.as_str())
        .bind(req.currency.as_str())
        .fetch_optional(&self.pool)
        .await?;
        if let Some((id, status)) = existing {
            return Ok(CreateAccountResult {
                account_id: id,
                created: false,
                status,
            });
        }

        let id = Uuid::new_v4();
        let inserted = sqlx::query(
            "INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status)
             VALUES ($1, $2, $3, $4, $5, $6, 'active')
             ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(&req.owner_type)
        .bind(req.owner_id)
        .bind(req.account_type.as_str())
        .bind(req.currency.as_str())
        .bind(&req.name)
        .execute(&self.pool)
        .await?;

        if inserted.rows_affected() == 1 {
            return Ok(CreateAccountResult {
                account_id: id,
                created: true,
                status: "active".into(),
            });
        }

        // Lost a race with a concurrent create of the same owner: return theirs.
        let (id, status) = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, status FROM accounts
             WHERE owner_type = $1 AND owner_id IS NOT DISTINCT FROM $2
               AND type = $3 AND currency = $4",
        )
        .bind(&req.owner_type)
        .bind(req.owner_id)
        .bind(req.account_type.as_str())
        .bind(req.currency.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(CreateAccountResult {
            account_id: id,
            created: false,
            status,
        })
    }

    /// Idempotently post a payment with the platform fee charged to the paying
    /// party (percentage only, banker's rounding), and — when an agent
    /// facilitated it — pay the agent commission out of fee revenue into the
    /// agent's float. Both journals commit atomically or not at all.
    ///
    /// Fee model (docs/architecture.md, confirmed decisions):
    /// - payer pays `amount + fee + tax`; payee receives `amount`;
    ///   `fee_revenue` receives `fee` and the `tax_payable` liability receives
    ///   `tax` (single journal, all legs). Tax legs ride in the payment
    ///   journal, so a reversal automatically returns tax as well.
    /// - a transaction tax (req.tax) is configuration, not schema: enabling
    ///   an e-levy-style levy in a jurisdiction sets bps on the request.
    /// - agent commission is the platform's cost, absorbed from fee revenue
    ///   (`fee_revenue` -> agent float, its own journal). It is never deducted
    ///   from the customer or the merchant.
    /// - dust transactions (fee rounds to 0) simply carry no fee leg; the same
    ///   applies to a zero commission.
    ///
    /// Idempotency: the commission journal derives its key from the payment
    /// key (`{key}:commission`) in the same scope, and everything commits in
    /// one transaction, so a retry can never double-post either side.
    pub async fn post_payment(&self, req: PaymentRequest) -> Result<PaymentResult, EngineError> {
        if req.amount_minor <= 0 {
            return Err(EngineError::InvalidJournal(
                "payment amount must be > 0".into(),
            ));
        }
        if req.fee.bps > 10_000 {
            return Err(EngineError::InvalidJournal(format!(
                "fee bps {} exceeds 100%",
                req.fee.bps
            )));
        }
        if let Some(c) = &req.agent_commission {
            if c.bps > 10_000 {
                return Err(EngineError::InvalidJournal(format!(
                    "commission bps {} exceeds 100%",
                    c.bps
                )));
            }
        }
        if let Some(t) = &req.tax {
            if t.bps > 10_000 {
                return Err(EngineError::InvalidJournal(format!(
                    "tax bps {} exceeds 100%",
                    t.bps
                )));
            }
        }

        let fee = round_fee(req.amount_minor, req.fee.bps);
        let tax = req
            .tax
            .map(|t| round_fee(req.amount_minor, t.bps))
            .unwrap_or(0);
        let commission = req
            .agent_commission
            .map(|c| round_fee(req.amount_minor, c.bps))
            .unwrap_or(0);

        let mut tx = self.pool.begin().await?;
        let fee_revenue = self
            .find_platform_account(&mut tx, AccountType::FeeRevenue, req.currency)
            .await?;
        // Resolved even when this journal carries no tax (a None/dust tax
        // skips the leg): the tax_payable singleton is seeded per currency, so
        // a missing row is a deployment error worth failing on.
        let tax_payable = self
            .find_platform_account(&mut tx, AccountType::TaxPayable, req.currency)
            .await?;

        // Payment journal: payer pays principal + fee; payee gets principal;
        // platform keeps the fee (leg skipped when it rounds to zero).
        let payment_spec = JournalSpec {
            journal_type: req.journal_type,
            currency: req.currency,
            origin: req.origin.clone(),
            legs: payment_legs(
                req.amount_minor,
                req.fee.bps,
                req.payer_account_id,
                req.payee_account_id,
                fee_revenue.id,
                req.tax.map(|t| t.bps).unwrap_or(0),
                tax_payable.id,
            ),
        };
        validate_spec(&payment_spec)?;

        let payment_result = match self
            .run_with_idempotency(
                &mut tx,
                &req.idempotency_scope,
                &req.idempotency_key,
                &payment_spec,
                None,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tx.rollback().await?;
                return Err(e);
            }
        };

        // Commission journal: fee_revenue -> agent float, same transaction.
        // On an idempotent replay of the payment key this also replays (its own
        // key was recorded in the original run), so the result is identical.
        let mut commission_journal_id = None;
        if commission > 0 {
            let float_id = req
                .agent_commission
                .expect("commission > 0 implies an agent commission")
                .agent_float_account_id;
            let commission_spec = JournalSpec {
                journal_type: JournalType::Commission,
                currency: req.currency,
                origin: req.origin,
                legs: vec![
                    Leg {
                        account_id: fee_revenue.id,
                        direction: Direction::Debit,
                        amount_minor: commission,
                    },
                    Leg {
                        account_id: float_id,
                        direction: Direction::Credit,
                        amount_minor: commission,
                    },
                ],
            };
            validate_spec(&commission_spec)?;

            let commission_key = format!("{}:commission", req.idempotency_key);
            let cresult = match self
                .run_with_idempotency(
                    &mut tx,
                    &req.idempotency_scope,
                    &commission_key,
                    &commission_spec,
                    None,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tx.rollback().await?;
                    return Err(e);
                }
            };
            commission_journal_id = Some(cresult.journal_id);
        }

        tx.commit().await?;
        Ok(PaymentResult {
            journal_id: payment_result.journal_id,
            fee_minor: fee,
            tax_minor: tax,
            commission_minor: commission,
            commission_journal_id,
        })
    }

    /// Reverse a posted journal: post a new `reversal` journal whose legs
    /// negate every leg of the original, returning all money (principal **and**
    /// fee) to the accounts that paid it. The original is never edited; the
    /// reversal references it via `journals.reference`.
    ///
    /// Guards (all checked under the original journal's row lock, so
    /// concurrent reversals/refunds of the same journal serialize):
    /// - the original must exist and be `posted`;
    /// - its type must be reversible (not a hold/capture/release — the holds
    ///   lifecycle owns those — and not itself a reversal or refund);
    /// - it must not already have a reversal;
    /// - it must not have any refunds (a partially refunded payment is settled
    ///   by the refunds; reversing it too would double-return money).
    ///
    /// A payment with an agent commission posts a **separate** commission
    /// journal; reversing the payment does not reverse the commission. The
    /// caller must reverse the commission journal as well (its id is returned
    /// by `post_payment` as `commission_journal_id`).
    pub async fn reverse_journal(
        &self,
        req: ReversalRequest,
    ) -> Result<JournalResult, EngineError> {
        let mut tx = self.pool.begin().await?;

        // Lock the original journal row first: every guard below must read a
        // stable picture, and concurrent reversals/refunds of the same journal
        // serialize here.
        let original = self.load_journal(&mut tx, req.original_journal_id).await?;

        // The reversal spec is deterministic: the original's immutable entries,
        // with directions flipped.
        let legs = sqlx::query_as::<_, (Uuid, String, i64)>(
            "SELECT account_id, direction, amount_minor FROM entries WHERE journal_id = $1",
        )
        .bind(req.original_journal_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|(account_id, direction, amount_minor)| Leg {
            account_id,
            direction: match direction.as_str() {
                "debit" => Direction::Credit,
                _ => Direction::Debit,
            },
            amount_minor,
        })
        .collect::<Vec<_>>();
        let spec = JournalSpec {
            journal_type: JournalType::Reversal,
            currency: original.currency,
            origin: req.origin,
            legs,
        };

        // Idempotent replay short-circuits the guards: a retry of a successful
        // reversal must replay, not fail AlreadyReversed.
        if let Some(result) = self
            .check_idempotency_replay(&mut tx, &req.idempotency_scope, &req.idempotency_key, &spec)
            .await?
        {
            tx.commit().await?;
            return Ok(result);
        }

        if original.status != "posted" {
            tx.rollback().await?;
            return Err(EngineError::JournalNotPosted {
                journal_id: req.original_journal_id,
                status: original.status,
            });
        }
        if !is_reversible(&original.journal_type) {
            tx.rollback().await?;
            return Err(EngineError::NotReversible {
                journal_id: req.original_journal_id,
                journal_type: original.journal_type,
            });
        }
        if self
            .journal_has_child(&mut tx, req.original_journal_id, "reversal")
            .await?
        {
            tx.rollback().await?;
            return Err(EngineError::AlreadyReversed(req.original_journal_id));
        }
        if self
            .journal_has_child(&mut tx, req.original_journal_id, "refund")
            .await?
        {
            tx.rollback().await?;
            return Err(EngineError::HasRefunds(req.original_journal_id));
        }

        let result = self
            .run_with_idempotency(
                &mut tx,
                &req.idempotency_scope,
                &req.idempotency_key,
                &spec,
                None,
            )
            .await?;
        self.set_journal_reference(&mut tx, result.journal_id, req.original_journal_id)
            .await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Refund a payment: post a new `refund` journal that debits the payee
    /// (the account that received the money) and credits the payer, principal
    /// only — the platform fee stays earned. Partial refunds are supported;
    /// cumulative refunds of a journal can never exceed the principal the
    /// payee received. The original is never edited; the refund references it.
    ///
    /// Guards (under the original journal's row lock, as with reversals):
    /// - the original must exist, be `posted`, and be a refundable type;
    /// - it must not have been reversed;
    /// - `refund_from` must be the original's credited customer/merchant
    ///   account (the payee);
    /// - `refunded_so_far + amount <= payee principal`.
    pub async fn refund_payment(&self, req: RefundRequest) -> Result<JournalResult, EngineError> {
        if req.amount_minor <= 0 {
            return Err(EngineError::InvalidJournal(
                "refund amount must be > 0".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;

        let original = self.load_journal(&mut tx, req.original_journal_id).await?;
        let spec = JournalSpec {
            journal_type: JournalType::Refund,
            currency: original.currency,
            origin: req.origin,
            legs: vec![
                Leg {
                    account_id: req.refund_from_account_id,
                    direction: Direction::Debit,
                    amount_minor: req.amount_minor,
                },
                Leg {
                    account_id: req.refund_to_account_id,
                    direction: Direction::Credit,
                    amount_minor: req.amount_minor,
                },
            ],
        };

        // Idempotent replay short-circuits the guards: a retry of a successful
        // refund must replay, not fail RefundExceedsPrincipal.
        if let Some(result) = self
            .check_idempotency_replay(&mut tx, &req.idempotency_scope, &req.idempotency_key, &spec)
            .await?
        {
            tx.commit().await?;
            return Ok(result);
        }

        if original.status != "posted" {
            tx.rollback().await?;
            return Err(EngineError::JournalNotPosted {
                journal_id: req.original_journal_id,
                status: original.status,
            });
        }
        if !is_refundable(&original.journal_type) {
            tx.rollback().await?;
            return Err(EngineError::NotRefundable {
                journal_id: req.original_journal_id,
                journal_type: original.journal_type,
            });
        }
        if self
            .journal_has_child(&mut tx, req.original_journal_id, "reversal")
            .await?
        {
            tx.rollback().await?;
            return Err(EngineError::JournalReversed(req.original_journal_id));
        }

        // The payee of the original: its credited customer/merchant leg.
        let (payee_account_id, payee_principal) = sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT e.account_id, e.amount_minor
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             WHERE e.journal_id = $1 AND e.direction = 'credit' AND a.owner_id IS NOT NULL
             LIMIT 1",
        )
        .bind(req.original_journal_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            EngineError::InvalidJournal("original journal has no customer-payable leg".into())
        })?;
        if payee_account_id != req.refund_from_account_id {
            tx.rollback().await?;
            return Err(EngineError::RefundFromMismatch {
                journal_id: req.original_journal_id,
                expected: payee_account_id,
                provided: req.refund_from_account_id,
            });
        }

        let refunded: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0)::bigint
             FROM journals WHERE reference = $1::text AND type = 'refund'",
        )
        .bind(req.original_journal_id)
        .fetch_one(&mut *tx)
        .await?;
        if refunded.saturating_add(req.amount_minor) > payee_principal {
            tx.rollback().await?;
            return Err(EngineError::RefundExceedsPrincipal {
                requested: req.amount_minor,
                refunded,
                principal: payee_principal,
            });
        }

        let result = self
            .run_with_idempotency(
                &mut tx,
                &req.idempotency_scope,
                &req.idempotency_key,
                &spec,
                None,
            )
            .await?;
        self.set_journal_reference(&mut tx, result.journal_id, req.original_journal_id)
            .await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Release every hold past its `expires_at` (scheduled sweep). Returns the
    /// number of holds expired. Each release runs in its own transaction.
    pub async fn expire_holds(&self) -> Result<usize, EngineError> {
        let mut expired = 0usize;
        loop {
            let mut tx = self.pool.begin().await?;
            let due: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM holds WHERE status = 'held' AND expires_at <= now()
                 ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED",
            )
            .fetch_optional(&mut *tx)
            .await?;

            let Some(hold_id) = due else {
                tx.rollback().await?;
                return Ok(expired);
            };

            let hold = self.lock_hold(&mut tx, hold_id).await?;
            let spec = JournalSpec {
                journal_type: JournalType::Release,
                currency: hold.currency,
                origin: Origin {
                    channel: "system".into(),
                    ..Default::default()
                },
                legs: vec![
                    Leg {
                        account_id: hold.hold_escrow_account_id,
                        direction: Direction::Debit,
                        amount_minor: hold.amount_minor,
                    },
                    Leg {
                        account_id: hold.account_id,
                        direction: Direction::Credit,
                        amount_minor: hold.amount_minor,
                    },
                ],
            };
            let result = self
                .execute_journal(
                    &mut tx,
                    &spec,
                    Some(HoldAction::Expire {
                        hold_id,
                        wallet_account_id: hold.account_id,
                    }),
                    "system",
                    &format!("expire-{hold_id}"),
                )
                .await;
            match result {
                Ok(_) => {
                    tx.commit().await?;
                    expired += 1;
                }
                Err(e) => {
                    tx.rollback().await?;
                    return Err(e);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Internals
    // ------------------------------------------------------------------

    /// Idempotency-guarded journal execution: checks the (scope, key) cache,
    /// runs `execute_journal` (optionally with a hold action), and records the
    /// response. Returns the journal result, or the replayed original result.
    ///
    /// Two layers of protection:
    /// 1. the `idempotency_keys` cache replays the stored response and marks
    ///    in-flight requests (claim is a unique INSERT);
    /// 2. the `journals` (scope, key) unique row — carrying `request_hash` —
    ///    is the durable backstop: even if the cache row expired or was pruned,
    ///    a genuine duplicate replays from the journal and can never re-post.
    async fn run_with_idempotency(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        scope: &str,
        key: &str,
        spec: &JournalSpec,
        hold_action: Option<HoldAction>,
    ) -> Result<JournalResult, EngineError> {
        if let Some(result) = self.check_idempotency_replay(tx, scope, key, spec).await? {
            return Ok(result);
        }

        // Claim the key. If another request already owns it (race), treat as
        // in-progress; the caller can retry. An expired/stale row (e.g. a
        // crashed request) is reclaimed instead of blocking forever.
        let request_hash = hash_spec(spec);
        let expires_at = Utc::now() + IDEMPOTENCY_KEY_TTL;
        let inserted = sqlx::query(
            "INSERT INTO idempotency_keys (scope, key, request_hash, status, expires_at)
             VALUES ($1, $2, $3, 'in_progress', $4)
             ON CONFLICT (scope, key) DO NOTHING",
        )
        .bind(scope)
        .bind(key)
        .bind(&request_hash)
        .bind(expires_at)
        .execute(&mut **tx)
        .await?;
        if inserted.rows_affected() == 0 {
            let reclaimed = sqlx::query(
                "UPDATE idempotency_keys
                 SET request_hash = $3, status = 'in_progress', response = NULL, expires_at = $4
                 WHERE scope = $1 AND key = $2
                   AND expires_at IS NOT NULL AND expires_at <= now()",
            )
            .bind(scope)
            .bind(key)
            .bind(&request_hash)
            .bind(expires_at)
            .execute(&mut **tx)
            .await?;
            if reclaimed.rows_affected() == 0 {
                return Err(EngineError::IdempotencyInProgress {
                    scope: scope.into(),
                    key: key.into(),
                });
            }
        }

        let result = self
            .execute_journal(tx, spec, hold_action, scope, key)
            .await?;
        set_idempotency_response(tx, scope, key, json!({ "journal_id": result.journal_id }))
            .await?;
        Ok(result)
    }

    /// Decide whether (scope, key) already executed, and with what result.
    ///
    /// - A **valid** cache record (`done`, not expired): replay the stored
    ///   response, but only if the stored request hash matches the incoming
    ///   spec — the same key with a different payload is rejected
    ///   (`IdempotencyMismatch`), per the idempotency contract.
    /// - A valid `in_progress` record: another request owns the key
    ///   (`IdempotencyInProgress`).
    /// - A missing or **expired** cache record: fall back to the durable
    ///   `journals` row (scope, key). If a journal exists, replay it (verifying
    ///   its `request_hash` when present); if not, the key is free.
    async fn check_idempotency_replay(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        scope: &str,
        key: &str,
        spec: &JournalSpec,
    ) -> Result<Option<JournalResult>, EngineError> {
        if let Some(record) = fetch_idempotency(tx, scope, key).await? {
            let expired = record.expires_at.is_some_and(|e| e <= Utc::now());
            if !expired {
                return match record.status.as_str() {
                    "done" => {
                        if record.request_hash != hash_spec(spec) {
                            Err(EngineError::IdempotencyMismatch {
                                scope: scope.into(),
                                key: key.into(),
                            })
                        } else {
                            Ok(Some(replay_response(record.response)?))
                        }
                    }
                    _ => Err(EngineError::IdempotencyInProgress {
                        scope: scope.into(),
                        key: key.into(),
                    }),
                };
            }
            // Expired: the durable journals row decides below.
        }

        let existing: Option<(Uuid, Option<String>)> = sqlx::query_as(
            "SELECT id, request_hash FROM journals
             WHERE idempotency_scope = $1 AND idempotency_key = $2",
        )
        .bind(scope)
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?;
        match existing {
            Some((_, Some(h))) if h != hash_spec(spec) => Err(EngineError::IdempotencyMismatch {
                scope: scope.into(),
                key: key.into(),
            }),
            Some((journal_id, _)) => Ok(Some(JournalResult::posted(journal_id))),
            None => Ok(None),
        }
    }

    /// The actual posting: lock accounts, validate, insert journal + entries +
    /// outbox event, rebuild snapshots. Journal first, then hold row, so every
    /// FK is satisfied. Runs inside the caller's transaction.
    #[allow(clippy::too_many_arguments)]
    async fn execute_journal(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        spec: &JournalSpec,
        hold_action: Option<HoldAction>,
        idempotency_scope: &str,
        idempotency_key: &str,
    ) -> Result<JournalResult, EngineError> {
        // 1. Lock all involved accounts in canonical (sorted) order. For
        //    capture/release/expire, the hold's payer wallet must also be locked:
        //    its held_minor changes with the hold status even though no entry
        //    touches it, so its snapshot needs rebuilding under its own lock.
        let mut account_ids: Vec<Uuid> = spec.legs.iter().map(|l| l.account_id).collect();
        if let Some(action) = &hold_action {
            match action {
                HoldAction::Capture {
                    wallet_account_id, ..
                }
                | HoldAction::Release {
                    wallet_account_id, ..
                }
                | HoldAction::Expire {
                    wallet_account_id, ..
                } => account_ids.push(*wallet_account_id),
                HoldAction::Insert { .. } => {}
            }
        }
        account_ids.sort_unstable();
        account_ids.dedup();
        let accounts = lock_accounts(tx, &account_ids).await?;
        let by_id: HashMap<Uuid, AccountRow> = accounts.into_iter().map(|a| (a.id, a)).collect();
        for id in &account_ids {
            if !by_id.contains_key(id) {
                return Err(EngineError::AccountNotFound(*id));
            }
        }

        // 2. Derive current available balances from entries, under the locks.
        let available = balances::available_for(tx, &account_ids).await?;

        // 3. Validate: net debit per account must not exceed its available
        //    balance. Pure logic (see `check_funds`), property-tested.
        let bridge_ids: std::collections::HashSet<Uuid> = by_id
            .iter()
            .filter(|(_, a)| a.account_type == AccountType::RailBridge)
            .map(|(id, _)| *id)
            .collect();
        check_funds(&spec.legs, &available, &bridge_ids)?;

        // 4. Insert the journal (unique (scope, key) here is the hard backstop
        //    against double-posting).
        let journal_id = Uuid::new_v4();
        let debit_total: MinorUnits = spec
            .legs
            .iter()
            .filter(|l| l.direction == Direction::Debit)
            .map(|l| l.amount_minor)
            .sum();

        sqlx::query(
            "INSERT INTO journals (id, type, status, currency, amount_minor, origin_user_id, origin_channel, origin_session_id, payment_code, idempotency_scope, idempotency_key, request_hash, posted_at)
             VALUES ($1, $2, 'posted', $3, $4, $5, $6, $7, $8, $9, $10, $11, now())",
        )
        .bind(journal_id)
        .bind(spec.journal_type.as_str())
        .bind(spec.currency.as_str())
        .bind(debit_total)
        .bind(spec.origin.user_id)
        .bind(&spec.origin.channel)
        .bind(spec.origin.session_id.as_deref())
        .bind(spec.origin.payment_code.as_deref())
        .bind(idempotency_scope)
        .bind(idempotency_key)
        .bind(hash_spec(spec))
        .execute(&mut **tx)
        .await?;

        // 5. Insert the entries (immutable).
        for leg in &spec.legs {
            sqlx::query(
                "INSERT INTO entries (id, journal_id, account_id, direction, amount_minor, currency)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(Uuid::new_v4())
            .bind(journal_id)
            .bind(leg.account_id)
            .bind(leg.direction.as_str())
            .bind(leg.amount_minor)
            .bind(spec.currency.as_str())
            .execute(&mut **tx)
            .await?;
        }

        // 6. Hold action, now that the journal exists.
        match &hold_action {
            Some(HoldAction::Insert {
                hold_id,
                escrow_account_id,
                expires_at,
            }) => {
                // The wallet account is the debit leg of a hold journal.
                let wallet_id = spec
                    .legs
                    .iter()
                    .find(|l| l.direction == Direction::Debit)
                    .map(|l| l.account_id)
                    .expect("hold journal has a debit leg");
                let amount = spec
                    .legs
                    .iter()
                    .find(|l| l.direction == Direction::Debit)
                    .map(|l| l.amount_minor)
                    .expect("hold journal has a debit leg");
                sqlx::query(
                    "INSERT INTO holds (id, journal_id, account_id, hold_escrow_account_id, amount_minor, currency, status, expires_at)
                     VALUES ($1, $2, $3, $4, $5, $6, 'held', $7)",
                )
                .bind(hold_id)
                .bind(journal_id)
                .bind(wallet_id)
                .bind(escrow_account_id)
                .bind(amount)
                .bind(spec.currency.as_str())
                .bind(expires_at)
                .execute(&mut **tx)
                .await?;
            }
            Some(HoldAction::Capture { hold_id, .. }) => {
                set_hold_status(tx, *hold_id, "captured").await?;
            }
            Some(HoldAction::Release { hold_id, .. }) => {
                set_hold_status(tx, *hold_id, "released").await?;
            }
            Some(HoldAction::Expire { hold_id, .. }) => {
                set_hold_status(tx, *hold_id, "expired").await?;
            }
            None => {}
        }

        // 7. Outbox event (same transaction -> exactly-once semantics downstream).
        sqlx::query(
            "INSERT INTO ledger_events (journal_id, event_type, payload) VALUES ($1, $2, $3)",
        )
        .bind(journal_id)
        .bind(format!("journal.{}", spec.journal_type.as_str()))
        .bind(json!({
            "journal_id": journal_id,
            "type": spec.journal_type.as_str(),
            "amount_minor": debit_total,
            "currency": spec.currency.as_str(),
            "origin_user_id": spec.origin.user_id,
            "origin_channel": spec.origin.channel,
            "payment_code": spec.origin.payment_code,
        }))
        .execute(&mut **tx)
        .await?;

        // 8. Rebuild snapshots for every touched account (derived strictly from
        //    entries + holds; the transaction path never writes them directly).
        for account_id in &account_ids {
            balances::rebuild_snapshot(tx, *account_id).await?;
        }

        Ok(JournalResult::posted(journal_id))
    }

    async fn find_platform_account(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_type: AccountType,
        currency: Currency,
    ) -> Result<AccountRow, EngineError> {
        let row =
            sqlx::query_as::<_, (Uuid, String, Option<Uuid>, String, String, String, String)>(
                "SELECT id, owner_type, owner_id, type, currency, name, status
             FROM accounts
             WHERE type = $1 AND currency = $2 AND owner_id IS NULL
             LIMIT 1",
            )
            .bind(account_type.as_str())
            .bind(currency.as_str())
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(EngineError::PlatformAccountMissing {
                account_type,
                currency,
            })?;
        Ok(account_from_row(row))
    }

    async fn lock_hold(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        hold_id: Uuid,
    ) -> Result<HoldRow, EngineError> {
        let row = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, i64, String, String, DateTime<Utc>)>(
            "SELECT id, journal_id, account_id, hold_escrow_account_id, amount_minor, currency, status, expires_at
             FROM holds WHERE id = $1 FOR UPDATE",
        )
        .bind(hold_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(EngineError::HoldNotFound(hold_id))?;

        Ok(HoldRow {
            id: row.0,
            journal_id: row.1,
            account_id: row.2,
            hold_escrow_account_id: row.3,
            amount_minor: row.4,
            currency: Currency::parse(&row.5).expect("valid currency from DB"),
            status: row.6,
            expires_at: row.7,
        })
    }

    /// Load a journal row under its own lock (reversal/refund guards must read
    /// a stable picture, and concurrent reversals/refunds of the same journal
    /// serialize on this lock).
    async fn load_journal(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        journal_id: Uuid,
    ) -> Result<JournalRow, EngineError> {
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT type, status, currency FROM journals WHERE id = $1 FOR UPDATE",
        )
        .bind(journal_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(EngineError::JournalNotFound(journal_id))?;
        Ok(JournalRow {
            journal_type: row.0,
            status: row.1,
            currency: Currency::parse(&row.2).expect("valid currency from DB"),
        })
    }

    /// Does any `child_type` journal reference `journal_id`? (Reversal/refund
    /// journals set `reference` to the original they reverse/refund.)
    async fn journal_has_child(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        journal_id: Uuid,
        child_type: &str,
    ) -> Result<bool, EngineError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM journals WHERE reference = $1::text AND type = $2)",
        )
        .bind(journal_id)
        .bind(child_type)
        .fetch_one(&mut **tx)
        .await?)
    }

    async fn set_journal_reference(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        journal_id: Uuid,
        reference: Uuid,
    ) -> Result<(), EngineError> {
        sqlx::query("UPDATE journals SET reference = $1::text WHERE id = $2")
            .bind(reference)
            .bind(journal_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Delete idempotency cache rows whose expiry has passed. The `journals`
    /// (scope, key) row is the durable backstop — replay falls back to it — so
    /// pruning the cache is always safe and never breaks duplicate detection.
    pub async fn prune_expired_idempotency_keys(&self) -> Result<u64, EngineError> {
        let result = sqlx::query(
            "DELETE FROM idempotency_keys WHERE expires_at IS NOT NULL AND expires_at <= now()",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

// ----------------------------------------------------------------------
// Requests / responses
// ----------------------------------------------------------------------

pub struct HoldRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub account_id: Uuid,
    pub amount_minor: MinorUnits,
    pub currency: Currency,
    pub expires_at: DateTime<Utc>,
    pub origin: Origin,
}

#[derive(Debug, Clone)]
pub struct HoldResult {
    pub hold_id: Uuid,
    pub journal_id: Uuid,
}

pub struct CaptureRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub hold_id: Uuid,
    pub target_account_id: Uuid,
    pub origin: Origin,
}

pub struct ReleaseRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub hold_id: Uuid,
    pub origin: Origin,
}

/// Platform fee charged to the paying party. Percentage only (Monime-style),
/// no flat component. 0.5% = 50 bps.
#[derive(Debug, Clone, Copy)]
pub struct FeePolicy {
    pub bps: BasisPoints,
}

/// A government transaction tax (e.g. an e-levy-style levy), collected from
/// the payer and held in the `tax_payable` liability account until remitted
/// to the authority — never platform revenue, never confused with fees.
///
/// Per-country configuration (master prompt §1/§12.1): a jurisdiction can
/// introduce or change a transaction tax by setting `bps` on payment requests
/// — no schema change, no code change. `None` = no tax (current behavior).
/// Tax is a separate rate from the fee: different rounding case, different
/// rounding boundary, and a different liability so collected tax is exactly
/// reconcilable with the authority's statement.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaxPolicy {
    /// Tax rate in basis points of the transaction amount (1 bp = 0.01%).
    pub bps: BasisPoints,
}

/// Agent facilitation of a payment: the platform pays the agent a commission
/// (bps of the transaction amount) out of its fee revenue into the agent's
/// float. Never deducted from the customer or merchant.
#[derive(Debug, Clone, Copy)]
pub struct AgentCommission {
    pub bps: BasisPoints,
    pub agent_float_account_id: Uuid,
}

/// A payment to post: principal legs plus (optionally) a government
/// transaction tax and agent commission.
#[derive(Debug, Clone)]
pub struct PaymentRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub journal_type: JournalType,
    pub currency: Currency,
    pub origin: Origin,
    pub payer_account_id: Uuid,
    pub payee_account_id: Uuid,
    pub amount_minor: MinorUnits,
    pub fee: FeePolicy,
    /// Government transaction tax collected from the payer (e.g. e-levy).
    /// `None` = no tax in this jurisdiction/configuration.
    pub tax: Option<TaxPolicy>,
    pub agent_commission: Option<AgentCommission>,
}

#[derive(Debug, Clone)]
pub struct PaymentResult {
    pub journal_id: Uuid,
    /// Fee charged to the payer (may be 0 for dust transactions).
    pub fee_minor: MinorUnits,
    /// Government tax collected from the payer (0 when absent or dust).
    pub tax_minor: MinorUnits,
    /// Commission paid to the agent (0 when no agent or it rounded to zero).
    pub commission_minor: MinorUnits,
    pub commission_journal_id: Option<Uuid>,
}

/// Reverse a posted journal. See `LedgerEngine::reverse_journal`.
#[derive(Debug, Clone)]
pub struct ReversalRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub original_journal_id: Uuid,
    pub origin: Origin,
}

/// Refund a payment, fully or partially. See `LedgerEngine::refund_payment`.
#[derive(Debug, Clone)]
pub struct RefundRequest {
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub original_journal_id: Uuid,
    /// The account that received money in the original journal and now returns
    /// it (must be the original's credited customer/merchant account).
    pub refund_from_account_id: Uuid,
    /// The account that originally paid (the buyer).
    pub refund_to_account_id: Uuid,
    pub amount_minor: MinorUnits,
    pub origin: Origin,
}

/// Provision a customer/merchant/agent account. See `LedgerEngine::create_account`.
#[derive(Debug, Clone)]
pub struct CreateAccountRequest {
    pub owner_type: String, // "user" | "merchant" | "agent"
    pub owner_id: Option<Uuid>,
    pub account_type: AccountType, // Wallet | Float
    pub currency: Currency,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CreateAccountResult {
    pub account_id: Uuid,
    /// False when the account already existed (idempotent replay).
    pub created: bool,
    pub status: String,
}

/// A journal row as read by the engine (reversal/refund guards).
#[derive(Debug, Clone)]
struct JournalRow {
    journal_type: String,
    status: String,
    currency: Currency,
}

/// Journal types that a full reversal may negate. The holds lifecycle
/// (hold/capture/release) owns its own journals; a reversal/refund must not
/// itself be reversed (corrections are new journals, not chains).
fn is_reversible(journal_type: &str) -> bool {
    matches!(
        journal_type,
        "p2p" | "topup" | "cash_in" | "cash_out" | "checkout" | "fee" | "commission" | "adjustment"
    )
}

/// Journal types whose payee can return principal to the payer via a refund.
fn is_refundable(journal_type: &str) -> bool {
    matches!(
        journal_type,
        "p2p" | "topup" | "cash_in" | "cash_out" | "checkout"
    )
}

/// Build the legs of a payment journal with the platform fee charged to the
/// paying party: payer pays `amount + fee + tax`, payee receives `amount`,
/// the platform's fee-revenue account receives `fee`, and the government's
/// `tax_payable` liability receives `tax` (an e-levy-style transaction tax,
/// collected from the payer — never platform revenue). The fee and tax legs
/// are each omitted when their percentage rounds to zero (dust transactions
/// carry neither).
pub fn payment_legs(
    amount_minor: MinorUnits,
    fee_bps: BasisPoints,
    payer_account_id: Uuid,
    payee_account_id: Uuid,
    fee_revenue_account_id: Uuid,
    tax_bps: BasisPoints,
    tax_payable_account_id: Uuid,
) -> Vec<Leg> {
    let fee = round_fee(amount_minor, fee_bps);
    let tax = round_fee(amount_minor, tax_bps);
    let mut legs = vec![
        Leg {
            account_id: payer_account_id,
            direction: Direction::Debit,
            amount_minor: amount_minor + fee + tax,
        },
        Leg {
            account_id: payee_account_id,
            direction: Direction::Credit,
            amount_minor,
        },
    ];
    if fee > 0 {
        legs.push(Leg {
            account_id: fee_revenue_account_id,
            direction: Direction::Credit,
            amount_minor: fee,
        });
    }
    if tax > 0 {
        legs.push(Leg {
            account_id: tax_payable_account_id,
            direction: Direction::Credit,
            amount_minor: tax,
        });
    }
    legs
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

/// Structural validation of a journal spec, independent of account state:
/// at least two legs, all amounts > 0, debits == credits. Public so callers
/// (and property tests) can pre-validate without touching the database.
/// The engine's funds rule as a pure function: a journal may not drive any
/// account's balance negative.
///
/// `available` maps account id -> current balance (never negative by
/// construction). Accounts named in `rail_bridge_accounts` are exempt: the
/// bridge is an internal clearing account tracking the platform's position
/// with external rails, which can legitimately run negative between rail
/// settlements — reconciliation monitors it instead of the engine.
///
/// Exposed separately from the posting path so property tests can pin the
/// invariant directly: a journal that passes here can never push a non-bridge
/// account below zero, and one that fails would.
pub fn check_funds(
    legs: &[Leg],
    available: &HashMap<Uuid, MinorUnits>,
    rail_bridge_accounts: &std::collections::HashSet<Uuid>,
) -> Result<(), EngineError> {
    // Net debit per account: positive means the journal takes more than it gives.
    let mut net: HashMap<Uuid, MinorUnits> = HashMap::new();
    for leg in legs {
        let entry = net.entry(leg.account_id).or_insert(0);
        *entry += match leg.direction {
            Direction::Debit => leg.amount_minor,
            Direction::Credit => -leg.amount_minor,
        };
    }
    for (account_id, need) in net {
        if need > 0 && !rail_bridge_accounts.contains(&account_id) {
            let balance = available.get(&account_id).copied().unwrap_or(0);
            if balance < need {
                return Err(EngineError::InsufficientFunds {
                    account: account_id,
                    need,
                    available: balance,
                });
            }
        }
    }
    Ok(())
}

pub fn validate_spec(spec: &JournalSpec) -> Result<(), EngineError> {
    if spec.legs.len() < 2 {
        return Err(EngineError::InvalidJournal(
            "a journal needs at least two legs".into(),
        ));
    }
    let mut debits: MinorUnits = 0;
    let mut credits: MinorUnits = 0;
    for leg in &spec.legs {
        if leg.amount_minor <= 0 {
            return Err(EngineError::InvalidJournal(
                "leg amounts must be > 0".into(),
            ));
        }
        match leg.direction {
            Direction::Debit => debits += leg.amount_minor,
            Direction::Credit => credits += leg.amount_minor,
        }
    }
    if debits != credits {
        return Err(EngineError::Unbalanced { debits, credits });
    }
    Ok(())
}

async fn lock_accounts(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[Uuid],
) -> Result<Vec<AccountRow>, EngineError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<Uuid>, String, String, String, String)>(
        "SELECT id, owner_type, owner_id, type, currency, name, status
         FROM accounts WHERE id = ANY($1::uuid[])
         ORDER BY id FOR UPDATE",
    )
    .bind(ids)
    .fetch_all(&mut **tx)
    .await?;

    let mut accounts = Vec::with_capacity(rows.len());
    let mut found: Vec<Uuid> = Vec::with_capacity(rows.len());
    for row in rows {
        found.push(row.0);
        if row.6 != "active" {
            return Err(EngineError::AccountNotActive(row.0));
        }
        accounts.push(account_from_row(row));
    }
    for id in ids {
        if !found.contains(id) {
            return Err(EngineError::AccountNotFound(*id));
        }
    }
    Ok(accounts)
}

fn account_from_row(
    row: (Uuid, String, Option<Uuid>, String, String, String, String),
) -> AccountRow {
    AccountRow {
        id: row.0,
        owner_type: row.1,
        owner_id: row.2,
        account_type: parse_account_type(&row.3),
        currency: Currency::parse(&row.4).expect("valid currency from DB"),
        name: row.5,
        status: row.6,
    }
}

fn parse_account_type(s: &str) -> AccountType {
    match s {
        "wallet" => AccountType::Wallet,
        "float" => AccountType::Float,
        "hold_escrow" => AccountType::HoldEscrow,
        "fee_revenue" => AccountType::FeeRevenue,
        "platform_revenue" => AccountType::PlatformRevenue,
        "rail_bridge" => AccountType::RailBridge,
        "tax_payable" => AccountType::TaxPayable,
        other => panic!("unknown account type in DB: {other}"),
    }
}

async fn fetch_idempotency(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    key: &str,
) -> Result<Option<IdempotencyRecord>, EngineError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<serde_json::Value>,
            Option<DateTime<Utc>>,
        ),
    >(
        "SELECT status, request_hash, response, expires_at
         FROM idempotency_keys WHERE scope = $1 AND key = $2",
    )
    .bind(scope)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(
        |(status, request_hash, response, expires_at)| IdempotencyRecord {
            status,
            request_hash,
            response,
            expires_at,
        },
    ))
}

async fn set_idempotency_response(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    key: &str,
    response: serde_json::Value,
) -> Result<(), EngineError> {
    sqlx::query(
        "UPDATE idempotency_keys SET status = 'done', response = $1 WHERE scope = $2 AND key = $3",
    )
    .bind(response)
    .bind(scope)
    .bind(key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Rebuild a result from a cached response (idempotent replay).
fn replay_response(response: Option<serde_json::Value>) -> Result<JournalResult, EngineError> {
    let response = response.ok_or_else(|| {
        EngineError::InvalidJournal("cached idempotency response is missing".into())
    })?;
    let journal_id: Uuid =
        serde_json::from_value(response["journal_id"].clone()).map_err(|_| {
            EngineError::InvalidJournal("cached idempotency response is malformed".into())
        })?;
    Ok(JournalResult::posted(journal_id))
}

fn hash_spec(spec: &JournalSpec) -> String {
    let mut hasher = DefaultHasher::new();
    spec.journal_type.hash(&mut hasher);
    spec.currency.hash(&mut hasher);
    spec.origin.user_id.hash(&mut hasher);
    spec.origin.channel.hash(&mut hasher);
    spec.origin.session_id.hash(&mut hasher);
    spec.origin.payment_code.hash(&mut hasher);
    for leg in &spec.legs {
        leg.account_id.hash(&mut hasher);
        leg.direction.hash(&mut hasher);
        leg.amount_minor.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

async fn set_hold_status(
    tx: &mut Transaction<'_, Postgres>,
    hold_id: Uuid,
    status: &str,
) -> Result<(), EngineError> {
    let column = match status {
        "captured" => "captured_at",
        "released" | "expired" => "released_at",
        other => panic!("unknown hold status {other}"),
    };
    sqlx::query(&format!(
        "UPDATE holds SET status = $1, {column} = $2 WHERE id = $3"
    ))
    .bind(status)
    .bind(Utc::now())
    .bind(hold_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
