//! App-layer audit log (docs/security-controls.md §4, app half): who did
//! what, from where, with what outcome. The `audit_log` table is append-only
//! by convention; this helper is the single writer so event names stay
//! consistent (what the ops queries and the nightly drift check rely on).
//!
//! Audit writes never fail the request that produced them: a lost audit row
//! is an operational problem, not a reason to 500 a completed money
//! movement. Failures are logged loudly instead.

use sqlx::PgPool;
use uuid::Uuid;

/// Record one audit event. Fire-and-forget by design: callers never `?` the
/// result. `subject` names the thing acted on (e.g. "pin", a goal id);
/// `outcome` is "success" | "failure"; `details` carries structured,
/// non-sensitive context (counts and amounts, never tokens or PINs).
pub fn audit(
    pool: &PgPool,
    user_id: Option<Uuid>,
    event: &str,
    subject: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let event = event.to_string();
    let subject = subject.to_string();
    let outcome = outcome.to_string();
    let pool = pool.clone(); // pools are Arc-cheap; the task owns its handle
                             // Own task: the caller (often a request handler) must not block on the
                             // audit insert, and a slow audit path must not slow the money path.
    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT INTO audit_log (id, user_id, event, subject, outcome, details)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&event)
        .bind(&subject)
        .bind(&outcome)
        .bind(details)
        .execute(&pool)
        .await;
        if let Err(err) = result {
            // Loud, but non-fatal — see the module docs.
            tracing::error!(error = %err, %event, "audit_log insert failed");
        }
    });
}
