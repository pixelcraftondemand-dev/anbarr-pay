//! Ledger gRPC server binary (build-order step 1, architecture §8).
//!
//! Startup: connect to Postgres, run migrations (idempotent), serve the §8
//! Ledger service with graceful shutdown on Ctrl-C.
//!
//! Configuration (environment, never committed):
//! - `DATABASE_URL` — Postgres connection string (see db.rs)
//! - `LEDGER_GRPC_ADDR` — listen address (default 127.0.0.1:50051)
//! - `LEDGER_GRPC_TOKEN` — shared secret callers must present as
//!   `x-ledger-token`; deploy from the secrets manager. The process exits if
//!   this value is missing so startup fails closed.
use anbarr_ledger::db;
use anbarr_ledger::engine::LedgerEngine;
use anbarr_ledger::grpc::LedgerGrpc;
use tonic::transport::Server;

fn grpc_token_from_env() -> anyhow::Result<String> {
    let app_env = std::env::var("APP_ENV").unwrap_or_default();
    let token = std::env::var("LEDGER_GRPC_TOKEN");

    match token {
        Ok(token) if !token.trim().is_empty() => Ok(token),
        Ok(_) => Err(anyhow::anyhow!(
            "LEDGER_GRPC_TOKEN is set but empty; the ledger gRPC service refuses to start without a real secret"
        )),
        Err(_) if app_env == "dev" => Ok("anbarrpay-internal-dev".to_string()),
        Err(_) => Err(anyhow::anyhow!(
            "LEDGER_GRPC_TOKEN must be set before starting the ledger gRPC service (or APP_ENV=dev for local-only testing)"
        )),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ledger_grpc_token = grpc_token_from_env()?;

    let database_url = db::database_url_from_env();
    let pool = db::connect(&database_url).await?;
    db::run_migrations(&pool).await?;

    let addr = std::env::var("LEDGER_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse::<std::net::SocketAddr>()?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(());
    });

    println!("ledger gRPC listening on {addr}");
    Server::builder()
        .add_service(LedgerGrpc::new(LedgerEngine::new(pool)).into_server())
        .serve_with_shutdown(addr, async {
            let _ = shutdown_rx.await;
        })
        .await?;
    println!("ledger gRPC shut down cleanly");
    Ok(())
}
