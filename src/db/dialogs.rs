use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::SqlitePool;

use crate::domain::ids::UserId;
use crate::domain::request::UnixTime;

/// A conversation idle for this long is dropped by the sweeper.
pub const DIALOG_TTL_SECS: i64 = 60 * 60;

/// The stored state, or `None` if there is none or it no longer parses
/// (for example after a code change).
pub async fn get<T: DeserializeOwned>(pool: &SqlitePool, user: UserId) -> sqlx::Result<Option<T>> {
    let json: Option<String> = sqlx::query_scalar("SELECT state FROM dialogs WHERE telegram_id = ?")
        .bind(user.0)
        .fetch_optional(pool)
        .await?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

pub async fn set<T: Serialize>(pool: &SqlitePool, user: UserId, state: &T, now: UnixTime) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO dialogs (telegram_id, state, updated_at) VALUES (?, ?, ?)
         ON CONFLICT (telegram_id) DO UPDATE SET state = excluded.state, updated_at = excluded.updated_at",
    )
    .bind(user.0)
    .bind(serde_json::to_string(state)?)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear(pool: &SqlitePool, user: UserId) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM dialogs WHERE telegram_id = ?").bind(user.0).execute(pool).await?;
    Ok(())
}

pub async fn delete_idle(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<u64> {
    let done = sqlx::query("DELETE FROM dialogs WHERE updated_at <= ?")
        .bind(now - DIALOG_TTL_SECS)
        .execute(pool)
        .await?;
    Ok(done.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn set_get_clear_and_expire() {
        let (pool, _dir) = test_pool().await;
        let user = UserId(5);
        set(&pool, user, &vec![1, 2], 0).await.unwrap();
        set(&pool, user, &vec![3], 10).await.unwrap();
        assert_eq!(get::<Vec<i32>>(&pool, user).await.unwrap(), Some(vec![3]));
        assert_eq!(get::<String>(&pool, user).await.unwrap(), None);

        assert_eq!(delete_idle(&pool, 10 + DIALOG_TTL_SECS - 1).await.unwrap(), 0);
        assert_eq!(delete_idle(&pool, 10 + DIALOG_TTL_SECS).await.unwrap(), 1);

        set(&pool, user, &vec![4], 20).await.unwrap();
        clear(&pool, user).await.unwrap();
        assert_eq!(get::<Vec<i32>>(&pool, user).await.unwrap(), None);
    }
}
