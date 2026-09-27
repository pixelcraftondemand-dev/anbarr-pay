//! AMBER PAY Core API — the application-domain service boundary.
//!
//! Per docs/architecture.md the Core API owns everything that is I/O-bound
//! and state-machine-heavy around the money, while the Rust ledger remains
//! the *only* writer of ledger tables. This crate implements that split in
//! one workspace for operability: its Postgres database (app metadata:
//! goals, wallet registry) is separate from the ledger's, and every money
//! movement goes through the ledger's gRPC contract (`amber.ledger.v1`).
//!
//! Components:
//! - `db` — core-database pool + migrations (`core-api/migrations`)
//! - `ledger` — tonic client wrapper for the ledger gRPC service
//! - `error` — problem+json error contract (docs/api.md preamble)
//! - `wallets` — wallet registry (links a caller to a verified ledger account)
//! - `vault` — goal savings: metadata + lock/release via ledger holds

pub mod ai;
pub mod db;
pub mod error;
pub mod ledger;
pub mod vault;
pub mod wallets;

use error::ApiError;
use std::time::Duration;

/// tonic client to the ledger's gRPC service, pre-configured with the shared
/// secret the ledger's interceptor requires (`x-ledger-token`). The token is
/// attached per request so the stored type stays the plain `LedgerClient`.
#[derive(Clone)]
pub struct LedgerClient {
    inner: ledger::pb::ledger_client::LedgerClient<tonic::transport::Channel>,
    token: String,
}

impl LedgerClient {
    pub fn new(
        inner: ledger::pb::ledger_client::LedgerClient<tonic::transport::Channel>,
        token: String,
    ) -> Self {
        LedgerClient { inner, token }
    }

    /// Attach the auth header to an outgoing request.
    pub fn prepare<T>(&self, mut req: tonic::Request<T>) -> tonic::Request<T> {
        if let Ok(value) = self.token.parse::<tonic::metadata::MetadataValue<_>>() {
            req.metadata_mut().insert("x-ledger-token", value);
        }
        req
    }

    /// Liveness probe used at startup: `GetAccount` on the nil UUID is a
    /// well-formed request the ledger answers — proving auth, wire format
    /// and storage are all reachable. A NotFound for the nil UUID still
    /// proves the service is up; anything else is a real failure.
    pub async fn health_check(&self, timeout: Duration) -> Result<(), String> {
        let mut client = self.inner.clone();
        let req = self.prepare(tonic::Request::new(ledger::pb::AccountRequest {
            account_id: uuid::Uuid::nil().to_string(),
        }));
        match tokio::time::timeout(timeout, client.get_account(req)).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(status)) if status.code() == tonic::Code::NotFound => Ok(()),
            Ok(Err(status)) => Err(status.to_string()),
            Err(_) => Err("timed out".to_string()),
        }
    }

    pub fn client(&self) -> ledger::pb::ledger_client::LedgerClient<tonic::transport::Channel> {
        self.inner.clone()
    }
}

/// Shared handler state.
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub ledger: LedgerClient,
    pub http_client: reqwest::Client,
    pub ollama_url: String,
    pub ollama_model: String,
}

impl AppState {
    pub fn new(
        pool: sqlx::PgPool,
        ledger: LedgerClient,
        http_client: reqwest::Client,
        ollama_url: String,
        ollama_model: String,
    ) -> Self {
        AppState {
            pool,
            ledger,
            http_client,
            ollama_url,
            ollama_model,
        }
    }
}

/// The caller-resolution header (docs/vault.md §4): until sign-in ships,
/// the gateway (or tests) name the caller explicitly. With auth absent this
/// is a development configuration — do not deploy publicly.
pub const CALLER_HEADER: &str = "x-amber-caller";

/// All routes in one router (mounted by `main` and by the tests). Handlers
/// return `Result<Json<T>, ApiError>`; `ApiError: IntoResponse` renders the
/// problem+json body (docs/api.md preamble) — no middleware layer needed.
pub fn create_app(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/v1/ledger", axum::routing::post(ai::chat))
        .route(
            "/v1/ledger/wallets",
            axum::routing::get(wallets::list_wallets).post(wallets::link_wallet),
        )
        .route(
            "/v1/ledger/vault/goals",
            axum::routing::get(vault::list_goals).post(vault::create_goal),
        )
        .route(
            "/v1/ledger/vault/goals/:id/locks",
            axum::routing::post(vault::add_lock),
        )
        .route(
            "/v1/ledger/vault/goals/:id/release",
            axum::routing::post(vault::release_goal),
        )
        .with_state(state)
}

/// Convenience alias used by handlers.
pub type ApiResult<T> = Result<T, ApiError>;
