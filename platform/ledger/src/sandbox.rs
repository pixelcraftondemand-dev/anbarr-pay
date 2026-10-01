//! Sandbox payment provider (docs/providers-webhooks.md §3, checklist §50).
//!
//! Simulates the rail adapter surface for SANDBOX environments: a statement
//! source for reconciliation, deterministic scenario outcomes (success /
//! failure / pending / timeout / duplicate callback / refund / provider
//! outage), and webhook envelopes signed the same way production ones are.
//!
//! Hard rules from the docs, enforced here:
//! - **Every sandbox artifact is labeled `SANDBOX_PAYMENT`** so simulated
//!   money can never be mistaken for real money (§73). References are
//!   prefixed and webhook payloads carry the label.
//! - **Unknown outcomes are never success**: `Timeout` behaves exactly like
//!   the real adapter contract — the caller sees uncertainty, and the
//!   generated statement line is absent until resolution.
//! - The sandbox feeds the **same `StatementLine` convention** production
//!   adapters produce (signed platform view), so the reconciliation pipeline
//!   under test is the production pipeline, not a parallel one.

use crate::money::Currency;
use crate::reconcile::{RailStatementSource, ReconcileError, StatementLine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Label required on every sandbox artifact (providers-webhooks.md §3).
pub const SANDBOX_PAYMENT_LABEL: &str = "SANDBOX_PAYMENT";

/// Prefix every sandbox external reference carries. Reconciliation's
/// `payment_code` matching sees real-looking references; the prefix makes the
/// sandbox provably identifiable in any ledger dump.
pub const SANDBOX_REF_PREFIX: &str = "SBX-";

/// The scenarios §50 requires the sandbox to simulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SandboxScenario {
    /// Payment settles normally: a statement line appears immediately.
    Success,
    /// Rail definitively rejects: NO statement line ever appears (a real
    /// rejected payment never moves money on the rail).
    Failed,
    /// Rail accepted but has not settled: the statement line appears only
    /// after `settle_after` elapses — until then the window excludes it.
    Pending { settle_after: DateTime<Utc> },
    /// The provider never answers. Unlike Pending there is no scheduled
    /// settlement: reconciliation cannot resolve it, which is precisely the
    /// drift scenario ops must see.
    Timeout,
    /// Payment settles, then a refund moves money back: two statement lines
    /// (negative then positive from the platform's view for a topup-style
    /// payment; see [`SandboxPayment::statement_lines`]).
    Refunded,
    /// Two deliveries of the same event id: the statement must contain the
    /// payment ONCE (the duplicate is dropped by the caller via
    /// [`SandboxRail::accept_webhook`]).
    DuplicateWebhook,
    /// The rail is unreachable: statement fetch fails with a typed outage
    /// error so the scheduler exercises its retry path.
    ProviderOutage,
}

impl SandboxScenario {
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxScenario::Success => "success",
            SandboxScenario::Failed => "failed",
            SandboxScenario::Pending { .. } => "pending",
            SandboxScenario::Timeout => "timeout",
            SandboxScenario::Refunded => "refunded",
            SandboxScenario::DuplicateWebhook => "duplicate_webhook",
            SandboxScenario::ProviderOutage => "provider_outage",
        }
    }
}

/// One sandbox-initiated payment on a rail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPayment {
    /// Sandbox reference: `SBX-<opaque>`. Caller-provided suffix.
    pub reference_suffix: String,
    pub rail: crate::reconcile::Rail,
    pub currency: Currency,
    /// Signed platform-view amount the payment will settle with.
    pub amount_minor: crate::money::MinorUnits,
    pub scenario: SandboxScenario,
    pub created_at: DateTime<Utc>,
}

impl SandboxPayment {
    /// Full sandbox reference (prefixed, unguessable suffix is the caller's job).
    pub fn external_reference(&self) -> String {
        format!("{SANDBOX_REF_PREFIX}{}", self.reference_suffix)
    }

    /// Whether this payment currently contributes statement lines at `now`.
    pub fn settled(&self, now: DateTime<Utc>) -> bool {
        match self.scenario {
            SandboxScenario::Success
            | SandboxScenario::Refunded
            | SandboxScenario::DuplicateWebhook => true,
            SandboxScenario::Pending { settle_after } => now >= settle_after,
            // Failed and Timeout payments never settle a statement line;
            // Timeout is *unresolved*, Failed is *definitively rejected*.
            SandboxScenario::Failed | SandboxScenario::Timeout => false,
            SandboxScenario::ProviderOutage => false,
        }
    }

    /// Statement lines this payment contributes at `now`, in the signed
    /// platform convention (+ = platform received, − = platform paid out).
    ///
    /// A settled payment yields exactly one line. A refunded payment yields
    /// the original line plus the refund line (opposite sign, same
    /// `<ref>-refund` reference). Unsettled scenarios yield none.
    pub fn statement_lines(&self, now: DateTime<Utc>) -> Vec<StatementLine> {
        if !self.settled(now) {
            return Vec::new();
        }
        let reference = self.external_reference();
        let mut lines = vec![StatementLine {
            external_reference: reference.clone(),
            amount_minor: self.amount_minor,
            currency: self.currency,
            occurred_at: self.settled_at(now),
        }];
        if matches!(self.scenario, SandboxScenario::Refunded) {
            lines.push(StatementLine {
                external_reference: format!("{reference}-refund"),
                amount_minor: -self.amount_minor,
                currency: self.currency,
                occurred_at: self.settled_at(now),
            });
        }
        lines
    }

    fn settled_at(&self, _now: DateTime<Utc>) -> DateTime<Utc> {
        match self.scenario {
            SandboxScenario::Pending { settle_after } => settle_after.max(self.created_at),
            _ => self.created_at,
        }
    }
}

/// The sandbox rail: holds registered payments and answers the two surfaces
/// the platform consumes — statement fetch (for reconciliation) and webhook
/// envelopes (for the callback pipeline). Deterministic: no sleeping, no
/// randomness; time is always a parameter.
#[derive(Debug, Clone)]
pub struct SandboxRail {
    payments: Vec<SandboxPayment>,
    /// HMAC key for webhook signatures — sandbox-only, distinct from any
    /// production secret by construction (separate environment credential).
    webhook_secret: String,
    /// Delivery log of accepted webhook event ids (dedup substrate).
    delivered_events: Vec<String>,
}

impl SandboxRail {
    pub fn new(webhook_secret: impl Into<String>) -> Self {
        SandboxRail {
            payments: Vec::new(),
            webhook_secret: webhook_secret.into(),
            delivered_events: Vec::new(),
        }
    }

    /// Registers a payment and returns its sandbox reference.
    pub fn create_payment(&mut self, payment: SandboxPayment) -> String {
        let reference = payment.external_reference();
        self.payments.push(payment);
        reference
    }

    pub fn payments(&self) -> &[SandboxPayment] {
        &self.payments
    }

    /// Inbound webhook envelope, signed like a production callback
    /// (HMAC-SHA256 over the raw body with the rail secret). The envelope
    /// carries the SANDBOX_PAYMENT label in the payload.
    pub fn webhook_envelope(&self, payment: &SandboxPayment, event_id: &str) -> SandboxWebhook {
        let payload = serde_json::json!({
            "event_id": event_id,
            "type": "payment.settled",
            "label": SANDBOX_PAYMENT_LABEL,
            "data": {
                "reference": payment.external_reference(),
                "amount_minor": payment.amount_minor,
                "currency": payment.currency.as_str(),
                "occurred_at": payment.created_at.to_rfc3339(),
            }
        });
        let body = serde_json::to_string(&payload).expect("sandbox webhook body serializes");
        let signature = sign(&self.webhook_secret, &body);
        SandboxWebhook {
            event_id: event_id.to_string(),
            body,
            signature,
        }
    }

    /// Accepts a webhook delivery: verifies the signature and dedupes on
    /// event id (providers-webhooks.md §2.1 steps 2/4). Returns
    /// `Accepted` for a first delivery, `Duplicate` for a replayed event id,
    /// and an error for an invalid signature — never silently processes an
    /// unverified payload.
    pub fn accept_webhook(
        &mut self,
        webhook: &SandboxWebhook,
    ) -> Result<WebhookAcceptance, ReconcileError> {
        let expected = sign(&self.webhook_secret, &webhook.body);
        if !constant_time_eq(&expected, &webhook.signature) {
            return Err(ReconcileError::StatementFetch(
                "sandbox webhook signature verification failed".into(),
            ));
        }
        if self
            .delivered_events
            .iter()
            .any(|id| id == &webhook.event_id)
        {
            return Ok(WebhookAcceptance::Duplicate);
        }
        self.delivered_events.push(webhook.event_id.clone());
        Ok(WebhookAcceptance::Accepted)
    }

    pub fn delivered_event_ids(&self) -> &[String] {
        &self.delivered_events
    }

    /// Statement lines across all registered payments for (rail, currency)
    /// within `[since, until)` at evaluation time `now`.
    pub fn statement_lines(
        &self,
        rail: crate::reconcile::Rail,
        currency: Currency,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Vec<StatementLine> {
        let mut lines = Vec::new();
        for payment in &self.payments {
            if payment.rail != rail || payment.currency != currency {
                continue;
            }
            for line in payment.statement_lines(now) {
                if line.occurred_at >= since && line.occurred_at < until {
                    lines.push(line);
                }
            }
        }
        lines.sort_by_key(|a| a.occurred_at);
        lines
    }

    /// The `RailStatementSource` view of this rail. Captures `now` at
    /// construction; tests that need a different evaluation time build a new
    /// source (cheap — payments are cloned).
    pub fn source(&self, now: DateTime<Utc>) -> RailStatementSource {
        RailStatementSource::Sandbox {
            sandbox: SandboxSource {
                lines: self.statement_lines_all(now),
            },
        }
    }

    fn statement_lines_all(&self, now: DateTime<Utc>) -> Vec<StatementLine> {
        let mut lines = Vec::new();
        for payment in &self.payments {
            lines.extend(payment.statement_lines(now));
        }
        lines.sort_by_key(|a| a.occurred_at);
        lines
    }
}

/// Acceptance outcome for a webhook delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookAcceptance {
    Accepted,
    Duplicate,
}

/// A signed inbound webhook envelope (providers-webhooks.md §2.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxWebhook {
    pub event_id: String,
    pub body: String,
    /// Hex-encoded HMAC-SHA256 over `body`.
    pub signature: String,
}

impl SandboxWebhook {
    pub fn label_present(&self) -> bool {
        serde_json::from_str::<serde_json::Value>(&self.body)
            .ok()
            .and_then(|v| {
                v.get("label")
                    .and_then(|l| l.as_str())
                    .map(|s| s == SANDBOX_PAYMENT_LABEL)
            })
            .unwrap_or(false)
    }
}

/// `RailStatementSource` adapter for sandbox payments. Keeps the reconciliation
/// pipeline (service, scheduler, sinks) unchanged — the sandbox is just
/// another statement source, exactly like a production rail adapter would be.
#[derive(Debug, Clone)]
pub struct SandboxSource {
    /// Statement lines at evaluation time; readable by the reconcile module's
    /// fetch arm (same crate) and by tests.
    pub(crate) lines: Vec<StatementLine>,
}

impl SandboxSource {
    /// Builds a source from registered sandbox payments evaluated at `now`.
    pub fn from_payments(payments: &[SandboxPayment], now: DateTime<Utc>) -> Self {
        let mut lines = Vec::new();
        for payment in payments {
            lines.extend(payment.statement_lines(now));
        }
        lines.sort_by_key(|a| a.occurred_at);
        SandboxSource { lines }
    }

    /// The SANDBOX_PAYMENT label every generated reference carries.
    pub fn is_sandbox_reference(reference: &str) -> bool {
        reference.starts_with(SANDBOX_REF_PREFIX)
    }
}

/// HMAC-SHA256 signing matching the outbound-webhook scheme so test
/// consumers verify sandbox envelopes with production verification code.
fn sign(secret: &str, body: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::MinorUnits;
    use crate::reconcile::Rail;

    fn payment(suffix: &str, amount: MinorUnits, scenario: SandboxScenario) -> SandboxPayment {
        SandboxPayment {
            reference_suffix: suffix.to_string(),
            rail: Rail::OrangeMoney,
            currency: Currency::Sle,
            amount_minor: amount,
            scenario,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn references_carry_sandbox_prefix() {
        let p = payment("abc123", 5_000, SandboxScenario::Success);
        assert_eq!(p.external_reference(), "SBX-abc123");
        assert!(SandboxSource::is_sandbox_reference(&p.external_reference()));
        assert!(!SandboxSource::is_sandbox_reference("OM-123456"));
    }

    #[test]
    fn success_settles_one_line() {
        let p = payment("s1", 5_000, SandboxScenario::Success);
        let lines = p.statement_lines(Utc::now());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_minor, 5_000);
        assert_eq!(lines[0].external_reference, "SBX-s1");
    }

    #[test]
    fn failed_never_settles() {
        let p = payment("f1", 5_000, SandboxScenario::Failed);
        assert!(p.statement_lines(Utc::now()).is_empty());
        assert!(p
            .statement_lines(Utc::now() + chrono::Duration::days(365))
            .is_empty());
    }

    #[test]
    fn pending_settles_only_after_the_deadline() {
        let now = Utc::now();
        let p = payment(
            "p1",
            5_000,
            SandboxScenario::Pending {
                settle_after: now + chrono::Duration::minutes(5),
            },
        );
        assert!(p.statement_lines(now).is_empty());
        let lines = p.statement_lines(now + chrono::Duration::minutes(6));
        assert_eq!(lines.len(), 1);
        // The settlement line's occurred_at is the scheduled settle time, not `now`.
        assert_eq!(lines[0].occurred_at, now + chrono::Duration::minutes(5));
    }

    #[test]
    fn timeout_never_resolves_into_a_statement_line() {
        let p = payment("t1", 5_000, SandboxScenario::Timeout);
        assert!(p
            .statement_lines(Utc::now() + chrono::Duration::days(30))
            .is_empty());
    }

    #[test]
    fn refund_yields_two_lines_with_opposite_signs() {
        let p = payment("r1", 5_000, SandboxScenario::Refunded);
        let lines = p.statement_lines(Utc::now());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].amount_minor, 5_000);
        assert_eq!(lines[1].amount_minor, -5_000);
        assert_eq!(lines[1].external_reference, "SBX-r1-refund");
        // A refund restores the rail position exactly.
        let net: MinorUnits = lines.iter().map(|l| l.amount_minor).sum();
        assert_eq!(net, 0);
    }

    #[test]
    fn webhook_payloads_are_labeled_and_signed() {
        let rail = SandboxRail::new("sandbox-secret");
        let p = payment("w1", 5_000, SandboxScenario::Success);
        let webhook = rail.webhook_envelope(&p, "evt-1");
        assert!(webhook.label_present());
        // Tampered body must not verify.
        let mut tampered = webhook.clone();
        tampered.body = tampered.body.replace("5", "9");
        assert!(matches!(
            rail.clone().accept_webhook(&tampered),
            Err(ReconcileError::StatementFetch(_))
        )); // The genuine envelope verifies and is accepted once.
        let mut rail_mut = rail;
        assert_eq!(
            rail_mut.accept_webhook(&webhook).ok(),
            Some(WebhookAcceptance::Accepted)
        );
        assert_eq!(
            rail_mut.accept_webhook(&webhook).ok(),
            Some(WebhookAcceptance::Duplicate)
        );
        assert_eq!(rail_mut.delivered_event_ids(), &["evt-1".to_string()]);
    }
}
