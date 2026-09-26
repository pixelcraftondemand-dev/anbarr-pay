//! Core-database pool and migration runner. This is the *app metadata*
//! database — separate from the ledger's (docs/architecture.md §5). Keeping
//! the URLs separate in dev too (two databases on the same dev Postgres)
//! exercises the real boundary: the Core API physically cannot touch ledger
//! tables with the credentials it is given in production.

use sqlx::postgres::PgPoolOptions;
use sqlx::{migrate::Migrator, PgPool};

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

/// Default local dev connection string for the *core* database
/// (docker-compose.yml maps host 5433 -> container 5432). Same dev Postgres
/// server as the ledger, different database — the boundary is enforced by
/// roles in deployment, and by separate metadata in tests.
pub const DEFAULT_CORE_DATABASE_URL: &str =
    "postgres://amber:amber_dev@localhost:5433/amber_core?sslmode=disable";

pub fn core_database_url_from_env() -> String {
    std::env::var("CORE_DATABASE_URL").unwrap_or_else(|_| DEFAULT_CORE_DATABASE_URL.to_string())
}
