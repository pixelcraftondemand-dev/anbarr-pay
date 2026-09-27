//! Scheduled reconciliation: compare the internal ledger against external rail
//! statements (Orange Money, Afrimoney, bank, cards) to catch drift early.
//!
//! Two independent checks run for a (rail, currency, time window):
//!
//! 1. **Position check** — the net movement of the `rail_bridge` account over
//!    the window must equal the sum of the rail's statement lines. The bridge
//!    tracks the platform's real-money position with each rail, so any
//!    divergence means the ledger and the rail disagree about money.
//! 2. **Line matching** — every rail statement line must have a journal whose
//!    `payment_code` equals the line's external reference (and vice versa),
//!    with the same amount. Unmatched lines on either side are suspicious and
//!    are reported for investigation.
//!
//! Sign convention (platform's point of view): **positive = money received by
//! the platform** (a credit to `rail_bridge`), **negative = money paid out**
//! (a debit to `rail_bridge`). Statement lines must use the same convention;
//! adapters for each rail are responsible for converting the rail's own format.
//!
//! The real rail adapters are not wired yet (Orange Money comes first). The
//! [`RailStatementSource::Stub`] variant lets the whole pipeline be exercised
//! end-to-end today; a production source is added with the rail integration.

use crate::money::{Currency, MinorUnits};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// External payment rails the ledger reconciles against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rail {
    OrangeMoney,
    Afrimoney,
    BankTransfer,
    Card,
}

impl Rail {
    pub fn as_str(&self) -> &'static str {
        match self {
            Rail::OrangeMoney => "orange_money",
            Rail::Afrimoney => "afrimoney",
            Rail::BankTransfer => "bank_transfer",
            Rail::Card => "card",
        }
    }
}

/// One line of a rail statement. `amount_minor` is signed (platform view):
/// + = money in, - = money out. See module docs for the convention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatementLine {
    /// Rail-side reference; expected to match `journals.payment_code`.
    pub external_reference: String,
    pub amount_minor: MinorUnits,
    pub currency: Currency,
    pub occurred_at: DateTime<Utc>,
}

/// Ledger-side view of one rail-tied journal (a journal with a `payment_code`
/// and a leg on the `rail_bridge` account). The amount is signed the same way
/// as [`StatementLine`], derived from the bridge leg's direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerRef {
    pub journal_id: Uuid,
    pub payment_code: String,
    pub journal_type: String,
    pub amount_minor: MinorUnits,
    pub occurred_at: DateTime<Utc>,
}

/// A statement line and a ledger journal shared a reference but disagree on
/// amount. A single human investigation can resolve both sides, so these are
/// grouped rather than counted as two unmatched items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmountMismatch {
    pub reference: String,
    pub statement_minor: MinorUnits,
    pub ledger_minor: MinorUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReconcileStatus {
    /// Everything matched and both checks balanced.
    Balanced,
    /// Drift found. `difference_minor` is ledger total minus statement total;
    /// `bridge_difference_minor` (when the bridge exists) is statement total
    /// minus bridge movement.
    Drift {
        difference_minor: MinorUnits,
        bridge_difference_minor: Option<MinorUnits>,
    },
}

/// Result of one reconciliation run. Every field is the full picture of the
/// window, so ops can page through a report without re-querying.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub rail: Rail,
    pub currency: Currency,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub statement_count: usize,
    pub ledger_count: usize,
    pub statement_total_minor: MinorUnits,
    pub ledger_total_minor: MinorUnits,
    /// Net `rail_bridge` movement over the window (None if no bridge account
    /// exists for the currency — a configuration error to alert on).
    pub bridge_movement_minor: Option<MinorUnits>,
    pub matched: usize,
    pub amount_mismatches: Vec<AmountMismatch>,
    /// Journals whose `payment_code` had no statement line.
    pub unmatched_on_ledger: Vec<LedgerRef>,
    /// Statement lines whose reference had no journal.
    pub unmatched_on_rail: Vec<StatementLine>,
    /// References that appear on more than one journal (never legitimate).
    pub duplicate_references: Vec<String>,
    pub status: ReconcileStatus,
}

/// Pure matching + aggregation. Given both sides of the window, decide whether
/// the ledger and the rail agree. No I/O — easy to test and reason about.
pub fn reconcile(
    rail: Rail,
    currency: Currency,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    statement: &[StatementLine],
    ledger_refs: &[LedgerRef],
    bridge_movement_minor: Option<MinorUnits>,
) -> ReconcileReport {
    let statement_total: MinorUnits = statement.iter().map(|l| l.amount_minor).sum();
    let ledger_total: MinorUnits = ledger_refs.iter().map(|l| l.amount_minor).sum();

    // Index ledger refs by payment_code; a duplicate reference is an anomaly.
    let mut by_code: std::collections::HashMap<&str, Vec<&LedgerRef>> = Default::default();
    let mut duplicate_references: Vec<String> = Vec::new();
    for lr in ledger_refs {
        by_code
            .entry(lr.payment_code.as_str())
            .or_default()
            .push(lr);
    }
    for (code, refs) in &by_code {
        if refs.len() > 1 {
            duplicate_references.push((*code).to_string());
        }
    }

    let mut matched = 0usize;
    let mut amount_mismatches: Vec<AmountMismatch> = Vec::new();
    let mut unmatched_on_rail: Vec<StatementLine> = Vec::new();
    let mut matched_codes: std::collections::HashSet<&str> = Default::default();

    for line in statement {
        match by_code.get(line.external_reference.as_str()) {
            Some(refs) if refs.len() == 1 => {
                let lr = refs[0];
                matched_codes.insert(lr.payment_code.as_str());
                if lr.amount_minor == line.amount_minor {
                    matched += 1;
                } else {
                    amount_mismatches.push(AmountMismatch {
                        reference: line.external_reference.clone(),
                        statement_minor: line.amount_minor,
                        ledger_minor: lr.amount_minor,
                    });
                }
            }
            _ => unmatched_on_rail.push(line.clone()),
        }
    }

    let unmatched_on_ledger: Vec<LedgerRef> = ledger_refs
        .iter()
        .filter(|lr| !matched_codes.contains(lr.payment_code.as_str()))
        .cloned()
        .collect();

    let difference_minor = ledger_total - statement_total;
    let bridge_difference_minor = bridge_movement_minor.map(|b| statement_total - b);
    let balanced = unmatched_on_ledger.is_empty()
        && unmatched_on_rail.is_empty()
        && amount_mismatches.is_empty()
        && duplicate_references.is_empty()
        && difference_minor == 0
        && bridge_difference_minor.unwrap_or(0) == 0;

    ReconcileReport {
        status: if balanced {
            ReconcileStatus::Balanced
        } else {
            ReconcileStatus::Drift {
                difference_minor,
                bridge_difference_minor,
            }
        },
        rail,
        currency,
        window_start,
        window_end,
        statement_count: statement.len(),
        ledger_count: ledger_refs.len(),
        statement_total_minor: statement_total,
        ledger_total_minor: ledger_total,
        bridge_movement_minor,
        matched,
        amount_mismatches,
        unmatched_on_ledger,
        unmatched_on_rail,
        duplicate_references,
    }
}

/// Raw Orange Money statement line from the provider. The provider-specific
/// payload is normalized to the signed platform convention in the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrangeMoneyStatementLine {
    pub external_reference: String,
    pub amount_minor: MinorUnits,
    pub currency: Currency,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct OrangeMoneySource {
    lines: Vec<OrangeMoneyStatementLine>,
}

impl OrangeMoneySource {
    pub fn new(lines: Vec<OrangeMoneyStatementLine>) -> Self {
        Self { lines }
    }

    pub fn lines_in_window(
        &self,
        currency: Currency,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Vec<StatementLine> {
        self.lines
            .iter()
            .filter(|line| line.currency == currency)
            .filter(|line| line.occurred_at >= since && line.occurred_at < until)
            .map(|line| StatementLine {
                external_reference: line.external_reference.clone(),
                amount_minor: line.amount_minor,
                currency: line.currency,
                occurred_at: line.occurred_at,
            })
            .collect()
    }
}

/// Where statement lines come from. The real rail adapters are modeled here;
/// the stub remains for simple local pipeline tests and smoke runs, and the
/// sandbox source (docs/providers-webhooks.md §3) drives the full §50
/// scenario matrix (success / failure / pending / timeout / duplicate
/// webhook / refund / outage) with `SANDBOX_PAYMENT`-labeled references.
#[derive(Debug, Clone)]
pub enum RailStatementSource {
    Stub {
        lines: Vec<StatementLine>,
    },
    OrangeMoney {
        source: OrangeMoneySource,
    },
    Sandbox {
        sandbox: crate::sandbox::SandboxSource,
    },
}

impl RailStatementSource {
    pub async fn fetch(
        &self,
        rail: Rail,
        currency: Currency,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<StatementLine>, ReconcileError> {
        match self {
            RailStatementSource::Stub { lines } => Ok(lines
                .iter()
                .filter(|line| line.currency == currency)
                .filter(|line| line.occurred_at >= since && line.occurred_at < until)
                .cloned()
                .collect()),
            RailStatementSource::OrangeMoney { source } => {
                if rail != Rail::OrangeMoney {
                    return Err(ReconcileError::StatementFetch(format!(
                        "OrangeMoney source cannot fetch {} statements",
                        rail.as_str()
                    )));
                }
                Ok(source.lines_in_window(currency, since, until))
            }
            RailStatementSource::Sandbox { sandbox } => Ok(sandbox
                .lines
                .iter()
                .filter(|line| line.currency == currency)
                .filter(|line| line.occurred_at >= since && line.occurred_at < until)
                .cloned()
                .collect()),
        }
    }
}

/// Runs reconciliation against the ledger database.
#[derive(Clone)]
pub struct ReconcileService {
    pool: PgPool,
}

impl ReconcileService {
    pub fn new(pool: PgPool) -> Self {
        ReconcileService { pool }
    }

    /// Reconcile one (rail, currency) over a window. Returns the report; the
    /// caller (scheduler) decides what to do with a `Drift` result (alert,
    /// page, open a case).
    pub async fn run(
        &self,
        rail: Rail,
        currency: Currency,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
        source: &RailStatementSource,
    ) -> Result<ReconcileReport, ReconcileError> {
        let statement = source
            .fetch(rail, currency, window_start, window_end)
            .await?;
        let ledger_refs = self
            .fetch_ledger_refs(currency, window_start, window_end)
            .await?;
        let bridge = self
            .bridge_movement(currency, window_start, window_end)
            .await?;
        Ok(reconcile(
            rail,
            currency,
            window_start,
            window_end,
            &statement,
            &ledger_refs,
            bridge,
        ))
    }

    /// Rail-tied journals in the window: those with a `payment_code` and a leg
    /// on the rail_bridge account. Signed amounts come from the bridge leg.
    async fn fetch_ledger_refs(
        &self,
        currency: Currency,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<LedgerRef>, ReconcileError> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, i64, DateTime<Utc>)>(
            "SELECT j.id,
                    j.payment_code,
                    j.type,
                    CASE WHEN e.direction = 'credit' THEN e.amount_minor ELSE -e.amount_minor END,
                    j.created_at
             FROM journals j
             JOIN entries e ON e.journal_id = j.id
             JOIN accounts a ON a.id = e.account_id
             WHERE j.payment_code IS NOT NULL
               AND a.type = 'rail_bridge'
               AND j.currency = $1
               AND j.created_at >= $2 AND j.created_at < $3
             ORDER BY j.created_at",
        )
        .bind(currency.as_str())
        .bind(since)
        .bind(until)
        .fetch_all(&self.pool)
        .await?;

        // A journal may legitimately have more than one leg, but only the
        // bridge leg is wanted; the query above already restricts to it, so a
        // payment_code appearing twice means duplicate rails references.
        let mut by_journal: std::collections::HashMap<Uuid, (String, String, i64, DateTime<Utc>)> =
            Default::default();
        for (jid, code, jtype, amount, occurred) in rows {
            by_journal
                .entry(jid)
                .or_insert((code, jtype, amount, occurred));
        }
        Ok(by_journal
            .into_iter()
            .map(
                |(journal_id, (payment_code, journal_type, amount_minor, occurred_at))| LedgerRef {
                    journal_id,
                    payment_code,
                    journal_type,
                    amount_minor,
                    occurred_at,
                },
            )
            .collect())
    }

    /// Net rail_bridge movement over the window: sum of (credits - debits),
    /// the platform's real-money position change. None if no rail_bridge
    /// account exists for the currency (alert-worthy configuration error).
    async fn bridge_movement(
        &self,
        currency: Currency,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Option<MinorUnits>, ReconcileError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM accounts WHERE type = 'rail_bridge' AND currency = $1)",
        )
        .bind(currency.as_str())
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Ok(None);
        }
        let movement: i64 = sqlx::query_scalar(
            "SELECT COALESCE(
                      SUM(CASE WHEN e.direction = 'credit' THEN e.amount_minor ELSE -e.amount_minor END),
                      0)::bigint
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             WHERE a.type = 'rail_bridge' AND a.currency = $1
               AND e.created_at >= $2 AND e.created_at < $3",
        )
        .bind(currency.as_str())
        .bind(since)
        .bind(until)
        .fetch_one(&self.pool)
        .await?;
        Ok(Some(movement))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error("statement fetch failed: {0}")]
    StatementFetch(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// Where reconciliation outcomes go.
///
/// - `Log`: human-readable lines on stderr (dev / simple deployments).
/// - `Db`: every run persisted to `reconciliation_runs` (durable audit
///   history; the Java core surfaces drift to ops dashboards and alerting).
#[derive(Debug, Clone)]
pub enum AlertSink {
    Log,
    Db { pool: PgPool },
}

impl AlertSink {
    pub async fn report(&self, run: &ReconcileReport) -> Result<(), ReconcileError> {
        match self {
            AlertSink::Log => {
                match &run.status {
                    ReconcileStatus::Balanced => {
                        eprintln!(
                            "[reconcile] {} {}: balanced (matched {} / {})",
                            run.rail.as_str(),
                            run.currency.as_str(),
                            run.matched,
                            run.statement_count
                        );
                    }
                    ReconcileStatus::Drift {
                        difference_minor,
                        bridge_difference_minor,
                    } => {
                        eprintln!(
                            "[reconcile] ALERT {} {}: DRIFT ledger-statement diff={} bridge-diff={:?} unmatched_ledger={} unmatched_rail={} mismatches={} duplicates={}",
                            run.rail.as_str(),
                            run.currency.as_str(),
                            difference_minor,
                            bridge_difference_minor,
                            run.unmatched_on_ledger.len(),
                            run.unmatched_on_rail.len(),
                            run.amount_mismatches.len(),
                            run.duplicate_references.len()
                        );
                    }
                }
                Ok(())
            }
            AlertSink::Db { pool } => {
                let (status, bridge_diff) = match &run.status {
                    ReconcileStatus::Balanced => ("balanced", None),
                    ReconcileStatus::Drift {
                        difference_minor: _,
                        bridge_difference_minor,
                    } => ("drift", *bridge_difference_minor),
                };
                let difference_minor = match &run.status {
                    ReconcileStatus::Balanced => 0,
                    ReconcileStatus::Drift {
                        difference_minor, ..
                    } => *difference_minor,
                };
                let detail = serde_json::to_value(run).map_err(|e| {
                    ReconcileError::StatementFetch(format!("report serialization failed: {e}"))
                })?;
                sqlx::query(
                    "INSERT INTO reconciliation_runs
                       (rail, currency, window_start, window_end, status, difference_minor, bridge_difference_minor,
                        statement_count, ledger_count, matched, unmatched_on_ledger, unmatched_on_rail,
                        amount_mismatches, duplicate_references, detail)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                )
                .bind(run.rail.as_str())
                .bind(run.currency.as_str())
                .bind(run.window_start)
                .bind(run.window_end)
                .bind(status)
                .bind(difference_minor)
                .bind(bridge_diff)
                .bind(run.statement_count as i32)
                .bind(run.ledger_count as i32)
                .bind(run.matched as i32)
                .bind(run.unmatched_on_ledger.len() as i32)
                .bind(run.unmatched_on_rail.len() as i32)
                .bind(run.amount_mismatches.len() as i32)
                .bind(run.duplicate_references.len() as i32)
                .bind(detail)
                .execute(pool)
                .await?;
                Ok(())
            }
        }
    }
}

/// One (rail, currency) pair the scheduler reconciles on every tick.
#[derive(Debug, Clone, Copy)]
pub struct ReconcileTask {
    pub rail: Rail,
    pub currency: Currency,
}

/// Scheduled reconciliation: on every tick, reconcile each configured
/// (rail, currency) over a rolling window and push every outcome to the sink.
///
/// This is the "scheduled job" — deployers embed it in the ledger service and
/// let it run for the process lifetime (or drive `run_once` from their own
/// cron-like scheduler). Drift is surfaced through [`AlertSink`].
#[derive(Clone)]
pub struct ReconcileScheduler {
    service: ReconcileService,
    source: RailStatementSource,
    sink: AlertSink,
    tasks: Vec<ReconcileTask>,
    window: chrono::Duration,
    interval: std::time::Duration,
}

impl ReconcileScheduler {
    pub fn new(
        service: ReconcileService,
        source: RailStatementSource,
        sink: AlertSink,
        tasks: Vec<ReconcileTask>,
        window: chrono::Duration,
        interval: std::time::Duration,
    ) -> Self {
        ReconcileScheduler {
            service,
            source,
            sink,
            tasks,
            window,
            interval,
        }
    }

    /// Run every configured task once, now. A failing task is logged and does
    /// not stop the rest; the returned reports are the successful runs (each
    /// already reported to the sink).
    pub async fn run_once(&self) -> Vec<ReconcileReport> {
        let now = Utc::now();
        let mut reports = Vec::new();
        for task in &self.tasks {
            match self
                .service
                .run(
                    task.rail,
                    task.currency,
                    now - self.window,
                    now,
                    &self.source,
                )
                .await
            {
                Ok(run) => {
                    if let Err(e) = self.sink.report(&run).await {
                        eprintln!(
                            "[reconcile] sink error for {} {}: {e}",
                            task.rail.as_str(),
                            task.currency.as_str()
                        );
                    }
                    reports.push(run);
                }
                Err(e) => eprintln!(
                    "[reconcile] run failed for {} {}: {e}",
                    task.rail.as_str(),
                    task.currency.as_str()
                ),
            }
        }
        reports
    }

    /// Run on the interval until `shutdown` receives `true`. Errors are logged
    /// and the loop continues (a bad rail must not kill the process).
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
                        self.run_once().await;
                    }
                    _ = shutdown.changed() => break,
                }
            }
        })
    }
}
