//! Support codes (what a person gives the human support account) and accepted
//! terms.

use sqlx::SqlitePool;

use crate::domain::ids::UserId;
use crate::domain::request::UnixTime;

/// Support codes are renewed after this.
pub const CODE_TTL_SECS: i64 = 30 * 24 * 60 * 60;

/// The user's current code, if still valid.
pub async fn code_of(pool: &SqlitePool, user: UserId, now: UnixTime) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT code FROM support_codes WHERE telegram_id = ? AND created_at > ?")
        .bind(user.0)
        .bind(now - CODE_TTL_SECS)
        .fetch_optional(pool)
        .await
}

/// Gives the user `code`, replacing an old one. False if the code is taken.
pub async fn set_code(pool: &SqlitePool, user: UserId, code: &str, now: UnixTime) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO support_codes (telegram_id, code, created_at) VALUES (?, ?, ?)
         ON CONFLICT (telegram_id) DO UPDATE SET code = excluded.code, created_at = excluded.created_at",
    )
    .bind(user.0)
    .bind(code)
    .bind(now)
    .execute(pool)
    .await;
    match result {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e),
    }
}

/// Who a valid code belongs to.
pub async fn user_of(pool: &SqlitePool, code: &str, now: UnixTime) -> sqlx::Result<Option<UserId>> {
    let id: Option<i64> = sqlx::query_scalar("SELECT telegram_id FROM support_codes WHERE code = ? AND created_at > ?")
        .bind(code)
        .bind(now - CODE_TTL_SECS)
        .fetch_optional(pool)
        .await?;
    Ok(id.map(UserId))
}

pub async fn delete_expired_codes(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM support_codes WHERE created_at <= ?").bind(now - CODE_TTL_SECS).execute(pool).await?;
    Ok(())
}

/// The version of the terms the user accepted, if any.
pub async fn terms_version(pool: &SqlitePool, user: UserId) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar("SELECT version FROM terms_accepted WHERE telegram_id = ?")
        .bind(user.0)
        .fetch_optional(pool)
        .await
}

pub async fn accept_terms(pool: &SqlitePool, user: UserId, version: i64, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO terms_accepted (telegram_id, version, accepted_at) VALUES (?, ?, ?)
         ON CONFLICT (telegram_id) DO UPDATE SET version = excluded.version, accepted_at = excluded.accepted_at",
    )
    .bind(user.0)
    .bind(version)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn codes_are_unique_found_back_and_expire() {
        let (pool, _dir) = test_pool().await;
        assert!(set_code(&pool, UserId(5), "K7P2", 0).await.unwrap());
        assert!(!set_code(&pool, UserId(6), "K7P2", 0).await.unwrap());
        assert_eq!(code_of(&pool, UserId(5), 10).await.unwrap().as_deref(), Some("K7P2"));
        assert_eq!(user_of(&pool, "K7P2", 10).await.unwrap(), Some(UserId(5)));
        assert_eq!(user_of(&pool, "K7P2", CODE_TTL_SECS).await.unwrap(), None);
        delete_expired_codes(&pool, CODE_TTL_SECS).await.unwrap();
        assert!(set_code(&pool, UserId(6), "K7P2", CODE_TTL_SECS).await.unwrap());
    }

    #[tokio::test]
    async fn terms_versions() {
        let (pool, _dir) = test_pool().await;
        assert_eq!(terms_version(&pool, UserId(5)).await.unwrap(), None);
        accept_terms(&pool, UserId(5), 1, 0).await.unwrap();
        accept_terms(&pool, UserId(5), 2, 10).await.unwrap();
        assert_eq!(terms_version(&pool, UserId(5)).await.unwrap(), Some(2));
    }
}
