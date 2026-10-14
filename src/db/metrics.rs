//! Daily metrics: counters and sums per day, never who or where.

use sqlx::SqlitePool;

use crate::domain::fare::Cup;
use crate::domain::request::State;

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
pub struct Day {
    pub day: String,
    pub published: i64,
    pub assigned: i64,
    pub completed: i64,
    pub cancelled: i64,
    pub expired: i64,
    pub km_sum: f64,
    pub price_sum: i64,
    pub final_sum: i64,
    pub suggested_sum: i64,
}

impl Day {
    pub fn avg_km(&self) -> Option<f64> {
        (self.published > 0).then(|| self.km_sum / self.published as f64)
    }

    pub fn avg_price(&self) -> Option<f64> {
        (self.published > 0).then(|| self.price_sum as f64 / self.published as f64)
    }

    /// Final price over suggested price, on assigned trips (1.10 = 10 % above).
    pub fn final_over_suggested(&self) -> Option<f64> {
        (self.suggested_sum > 0).then(|| self.final_sum as f64 / self.suggested_sum as f64)
    }
}

async fn ensure(pool: &SqlitePool, day: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO daily_metrics (day) VALUES (?)").bind(day).execute(pool).await?;
    Ok(())
}

pub async fn published(pool: &SqlitePool, day: &str, km: f64, price: Cup) -> sqlx::Result<()> {
    ensure(pool, day).await?;
    sqlx::query("UPDATE daily_metrics SET published = published + 1, km_sum = km_sum + ?, price_sum = price_sum + ? WHERE day = ?")
        .bind(km)
        .bind(price)
        .bind(day)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn assigned(pool: &SqlitePool, day: &str, price: Cup, suggested: Cup) -> sqlx::Result<()> {
    ensure(pool, day).await?;
    sqlx::query(
        "UPDATE daily_metrics SET assigned = assigned + 1, final_sum = final_sum + ?, suggested_sum = suggested_sum + ?
         WHERE day = ?",
    )
    .bind(price)
    .bind(suggested)
    .bind(day)
    .execute(pool)
    .await?;
    Ok(())
}

/// A request that reached a final state.
pub async fn closed(pool: &SqlitePool, day: &str, state: State) -> sqlx::Result<()> {
    let sql = match state {
        State::Completed => "UPDATE daily_metrics SET completed = completed + 1 WHERE day = ?",
        State::Cancelled => "UPDATE daily_metrics SET cancelled = cancelled + 1 WHERE day = ?",
        State::Expired => "UPDATE daily_metrics SET expired = expired + 1 WHERE day = ?",
        State::Open | State::Confirming | State::Assigned => return Ok(()),
    };
    ensure(pool, day).await?;
    sqlx::query(sql).bind(day).execute(pool).await?;
    Ok(())
}

/// The last `days` days with data, newest first.
pub async fn last(pool: &SqlitePool, days: i64) -> sqlx::Result<Vec<Day>> {
    sqlx::query_as(
        "SELECT day, published, assigned, completed, cancelled, expired, km_sum, price_sum, final_sum, suggested_sum
         FROM daily_metrics ORDER BY day DESC LIMIT ?",
    )
    .bind(days)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn days_add_up() {
        let (pool, _dir) = test_pool().await;
        published(&pool, "2026-10-14", 3.0, 600).await.unwrap();
        published(&pool, "2026-10-14", 5.0, 800).await.unwrap();
        assigned(&pool, "2026-10-14", 660, 600).await.unwrap();
        closed(&pool, "2026-10-14", State::Completed).await.unwrap();
        closed(&pool, "2026-10-14", State::Expired).await.unwrap();
        closed(&pool, "2026-10-15", State::Cancelled).await.unwrap();

        let days = last(&pool, 7).await.unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].day, "2026-10-15");
        let d = &days[1];
        assert_eq!((d.published, d.assigned, d.completed, d.expired), (2, 1, 1, 1));
        assert_eq!(d.avg_km(), Some(4.0));
        assert_eq!(d.avg_price(), Some(700.0));
        assert_eq!(d.final_over_suggested(), Some(1.1));
    }
}
