//! Postgres pool and migration runner.

use sqlx::postgres::PgPoolOptions;
use sqlx::{migrate::Migrator, PgPool};
use std::path::Path;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

/// Apply pending migrations. Idempotent (sqlx tracks applied migrations).
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

/// Default local dev connection string (docker-compose.yml maps host 5433 -> container 5432).
pub const DEFAULT_DATABASE_URL: &str =
    "postgres://anbarr:anbarr_dev@localhost:5433/anbarr?sslmode=disable";

pub fn database_url_from_env() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

/// Directory of the migrations, for tools that need the path (e.g. tests).
pub fn migrations_path() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
}
