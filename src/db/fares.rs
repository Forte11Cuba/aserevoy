use sqlx::SqlitePool;

use crate::domain::fare::Fares;

const BASE: &str = "base";
const PER_KM: &str = "per_km";
const MIN_PERCENT: &str = "min_percent";

/// Copies the `.env` defaults into the table the first time; later changes are
/// made from the bot and survive restarts.
pub async fn seed(pool: &SqlitePool, defaults: &Fares) -> sqlx::Result<()> {
    let rows = [
        (BASE, defaults.base),
        (PER_KM, defaults.per_km),
        (MIN_PERCENT, i64::from(defaults.min_percent)),
    ];
    for (key, value) in rows {
        sqlx::query("INSERT OR IGNORE INTO fares (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn load(pool: &SqlitePool) -> sqlx::Result<Fares> {
    let get = |key: &'static str| async move {
        sqlx::query_scalar::<_, i64>("SELECT value FROM fares WHERE key = ?")
            .bind(key)
            .fetch_one(pool)
            .await
    };
    Ok(Fares {
        base: get(BASE).await?,
        per_km: get(PER_KM).await?,
        min_percent: get(MIN_PERCENT).await?.clamp(0, 100) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_does_not_overwrite_existing_values() {
        let (pool, _dir) = crate::db::test_pool().await;
        let first = Fares { base: 100, per_km: 150, min_percent: 80 };
        seed(&pool, &first).await.unwrap();
        seed(&pool, &Fares { base: 1, per_km: 1, min_percent: 1 }).await.unwrap();
        assert_eq!(load(&pool).await.unwrap(), first);
    }
}
