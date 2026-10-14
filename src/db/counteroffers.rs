use sqlx::SqlitePool;

use crate::domain::fare::Cup;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{Counteroffer, UnixTime};

/// Saves a driver's counteroffer, replacing their previous one, only while the
/// request is open. Returns false if it is not.
pub async fn upsert(
    pool: &SqlitePool,
    request: &RequestId,
    driver: UserId,
    amount: Cup,
    expires_at: UnixTime,
) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "INSERT INTO counteroffers (request_id, driver_id, amount, expires_at)
         SELECT ?, ?, ?, ? WHERE EXISTS (SELECT 1 FROM requests WHERE id = ? AND state = 'open')
         ON CONFLICT (request_id, driver_id) DO UPDATE SET
             amount = excluded.amount, expires_at = excluded.expires_at",
    )
    .bind(request.as_str())
    .bind(driver.0)
    .bind(amount)
    .bind(expires_at)
    .bind(request.as_str())
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn for_request(pool: &SqlitePool, request: &RequestId) -> sqlx::Result<Vec<Counteroffer>> {
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT driver_id, amount, expires_at FROM counteroffers WHERE request_id = ? ORDER BY amount, driver_id",
    )
    .bind(request.as_str())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(driver, amount, expires_at)| Counteroffer { driver: UserId(driver), amount, expires_at })
        .collect())
}

pub async fn delete(pool: &SqlitePool, request: &RequestId, driver: UserId) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM counteroffers WHERE request_id = ? AND driver_id = ?")
        .bind(request.as_str())
        .bind(driver.0)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn delete_at_or_below(pool: &SqlitePool, request: &RequestId, price: Cup) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM counteroffers WHERE request_id = ? AND amount <= ?")
        .bind(request.as_str())
        .bind(price)
        .execute(pool)
        .await?;
    Ok(())
}

/// Drops all of the driver's counteroffers; returns the requests they were on.
pub async fn withdraw_driver(pool: &SqlitePool, driver: UserId) -> sqlx::Result<Vec<RequestId>> {
    let ids: Vec<String> = sqlx::query_scalar("DELETE FROM counteroffers WHERE driver_id = ? RETURNING request_id")
        .bind(driver.0)
        .fetch_all(pool)
        .await?;
    Ok(ids.iter().filter_map(|id| RequestId::parse(id)).collect())
}

/// Drops expired counteroffers; returns the requests whose lists changed.
pub async fn delete_expired(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<Vec<RequestId>> {
    let mut ids: Vec<String> =
        sqlx::query_scalar("DELETE FROM counteroffers WHERE expires_at <= ? RETURNING request_id")
            .bind(now)
            .fetch_all(pool)
            .await?;
    ids.sort();
    ids.dedup();
    Ok(ids.iter().filter_map(|id| RequestId::parse(id)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::requests::{self, close};
    use crate::db::test_pool;
    use crate::domain::fare::Fares;
    use crate::domain::request::{NewRequest, PaymentMethod, Point, Request, State};

    async fn open_request(pool: &SqlitePool, id: &str, client: i64) -> RequestId {
        let new = NewRequest {
            client: UserId(client),
            client_phone: None,
            pickup: Point { lat: 23.13, lon: -82.38 },
            dropoff: Point { lat: 23.12, lon: -82.40 },
            reference: None,
            km: 2.0,
            price: 400,
            payment: PaymentMethod::Cash,
        };
        let fares = Fares { base: 100, per_km: 150, min_percent: 80 };
        let (r, _) = Request::open(RequestId::parse(id).unwrap(), new, &fares, 0).unwrap();
        requests::insert(pool, &r).await.unwrap();
        r.id
    }

    #[tokio::test]
    async fn a_new_counteroffer_replaces_the_previous_one() {
        let (pool, _dir) = test_pool().await;
        let id = open_request(&pool, "AB7K", 1).await;
        assert!(upsert(&pool, &id, UserId(9), 450, 300).await.unwrap());
        assert!(upsert(&pool, &id, UserId(9), 500, 400).await.unwrap());
        let offers = for_request(&pool, &id).await.unwrap();
        assert_eq!(offers, vec![Counteroffer { driver: UserId(9), amount: 500, expires_at: 400 }]);
        assert!(delete(&pool, &id, UserId(9)).await.unwrap());
        assert!(!delete(&pool, &id, UserId(9)).await.unwrap());
    }

    #[tokio::test]
    async fn only_open_requests_take_counteroffers() {
        let (pool, _dir) = test_pool().await;
        let id = open_request(&pool, "AB7K", 1).await;
        close(&pool, &id, State::Open).await.unwrap();
        assert!(!upsert(&pool, &id, UserId(9), 450, 300).await.unwrap());
    }

    #[tokio::test]
    async fn withdrawing_and_expiring_report_the_affected_requests() {
        let (pool, _dir) = test_pool().await;
        let (a, b) = (open_request(&pool, "AB7K", 1).await, open_request(&pool, "CD8M", 2).await);
        upsert(&pool, &a, UserId(9), 450, 300).await.unwrap();
        upsert(&pool, &b, UserId(9), 450, 300).await.unwrap();
        upsert(&pool, &b, UserId(8), 450, 100).await.unwrap();

        assert_eq!(delete_expired(&pool, 100).await.unwrap(), vec![b.clone()]);
        let mut withdrawn = withdraw_driver(&pool, UserId(9)).await.unwrap();
        withdrawn.sort_by(|x, y| x.as_str().cmp(y.as_str()));
        assert_eq!(withdrawn, vec![a.clone(), b.clone()]);
        assert!(for_request(&pool, &a).await.unwrap().is_empty());
    }
}
