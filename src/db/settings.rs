use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?").bind(key).fetch_optional(pool).await
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn set_overwrites_and_get_reads() {
        let (pool, _dir) = test_pool().await;
        assert_eq!(get(&pool, "k").await.unwrap(), None);
        set(&pool, "k", "a").await.unwrap();
        set(&pool, "k", "b").await.unwrap();
        assert_eq!(get(&pool, "k").await.unwrap().as_deref(), Some("b"));
    }
}
