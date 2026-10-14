//! SQLite persistence. A single file holds both temporary state (open and
//! assigned requests) and the few permanent tables.

pub mod counteroffers;
pub mod dialogs;
pub mod drivers;
pub mod fares;
pub mod invitations;
pub mod requests;
pub mod settings;
pub mod users;

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        // Deleted rows (closed trips, phones) must not stay recoverable in the file.
        .pragma("secure_delete", "ON");
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
pub(crate) async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
    // A real file (not :memory:) so concurrent connections share the database.
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", dir.path().join("test.db").display());
    (connect(&url).await.unwrap(), dir)
}
