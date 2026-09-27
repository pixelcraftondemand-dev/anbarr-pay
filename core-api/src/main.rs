//! AMBER PAY Core API server binary.
//!
//! Owns app-domain data (vault goals, the wallet registry) in its own
//! database and drives money movement through the ledger's gRPC contract.
//! It never opens ledger tables directly — that boundary is the point
//! (docs/architecture.md §5: single writer, separate roles).
//!
//! Configuration (environment):
//! - `CORE_DATABASE_URL` — Postgres for app metadata
//!   (default: the dev core database on host port 5433)
//! - `LEDGER_ENDPOINT` — ledger gRPC URI (default http://127.0.0.1:50051)
//! - `LEDGER_GRPC_TOKEN` — shared secret presented to the ledger
//!   (`x-ledger-token`; same value the ledger server expects)
mod db;
use anbarr_core_api::{create_app, AppState, LedgerClient};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = db::core_database_url_from_env();
    let pool = db::connect(&database_url).await?;
    db::run_migrations(&pool).await?;

    let ledger_endpoint =
        std::env::var("LEDGER_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let ledger_token =
        std::env::var("LEDGER_GRPC_TOKEN").unwrap_or_else(|_| "amberpay-internal-dev".to_string());
    let channel = tonic::transport::Channel::from_shared(ledger_endpoint.clone())?.connect_lazy();
    let inner = anbarr_core_api::ledger::pb::ledger_client::LedgerClient::new(channel);
    let ledger = LedgerClient::new(inner, ledger_token);

    let addr = std::env::var("CORE_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse::<std::net::SocketAddr>()?;

    // Warm the channel; a downed ledger must not silently serve money paths.
    ledger
        .health_check(Duration::from_secs(5))
        .await
        .map_err(|e| anyhow::anyhow!("ledger gRPC health check failed at startup: {e}"))?;

    let ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let ollama_model =
        std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:latest".to_string());

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let state = AppState::new(pool, ledger, http_client, ollama_url, ollama_model);
    let app = create_app(state);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(());
    });

    println!("core-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        })
        .await?;
    println!("core-api shut down cleanly");
    Ok(())
}
