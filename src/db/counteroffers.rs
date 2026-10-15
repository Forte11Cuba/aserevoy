use sqlx::SqlitePool;

use crate::domain::fare::Cup;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{Counteroffer, UnixTime};

/// A counteroffer being removed outside a domain operation (expired, or its
/// driver got busy), with what is needed to delete its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub request: RequestId,
    pub client: UserId,
    pub client_msg_id: Option<i32>,
}

/// Saves a driver's counteroffer, replacing their previous one (and forgetting
/// its message), only while the request is open. Returns false if it is not.
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
             amount = excluded.amount, expires_at = excluded.expires_at, client_msg_id = NULL",
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

/// Records the message shown to the client, if the counteroffer is still the
/// same one (not replaced or removed meanwhile). Returns false otherwise, and
/// then the message is stale.
pub async fn set_msg(
    pool: &SqlitePool,
    request: &RequestId,
    driver: UserId,
    expires_at: UnixTime,
    msg: i32,
) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE counteroffers SET client_msg_id = ?
         WHERE request_id = ? AND driver_id = ? AND expires_at = ? AND client_msg_id IS NULL",
    )
    .bind(msg)
    .bind(request.as_str())
    .bind(driver.0)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn for_request(pool: &SqlitePool, request: &RequestId) -> sqlx::Result<Vec<Counteroffer>> {
    let rows: Vec<(i64, i64, i64, Option<i32>)> = sqlx::query_as(
        "SELECT driver_id, amount, expires_at, client_msg_id FROM counteroffers
         WHERE request_id = ? ORDER BY amount, driver_id",
    )
    .bind(request.as_str())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(driver, amount, expires_at, client_msg_id)| Counteroffer {
            driver: UserId(driver),
            amount,
            expires_at,
            client_msg_id,
        })
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

/// Drops all of the driver's counteroffers.
pub async fn withdraw_driver(pool: &SqlitePool, driver: UserId) -> sqlx::Result<Vec<Removed>> {
    let rows = sqlx::query_as(
        "SELECT c.request_id, c.driver_id, c.expires_at, r.client_id, c.client_msg_id
         FROM counteroffers c JOIN requests r ON r.id = c.request_id WHERE c.driver_id = ?",
    )
    .bind(driver.0)
    .fetch_all(pool)
    .await?;
    take(pool, rows).await
}

/// Drops the counteroffers that expired.
pub async fn take_expired(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<Vec<Removed>> {
    let rows = sqlx::query_as(
        "SELECT c.request_id, c.driver_id, c.expires_at, r.client_id, c.client_msg_id
         FROM counteroffers c JOIN requests r ON r.id = c.request_id WHERE c.expires_at <= ?",
    )
    .bind(now)
    .fetch_all(pool)
    .await?;
    take(pool, rows).await
}

/// `(request_id, driver_id, expires_at, client_id, client_msg_id)`
type Row = (String, i64, i64, i64, Option<i32>);

/// Deletes the selected counteroffers and returns them. Each is deleted by
/// its exact row, so one replaced in the meantime is neither lost nor reported.
async fn take(pool: &SqlitePool, rows: Vec<Row>) -> sqlx::Result<Vec<Removed>> {
    let mut removed = Vec::new();
    for (request, driver, expires_at, client, client_msg_id) in rows {
        let done = sqlx::query("DELETE FROM counteroffers WHERE request_id = ? AND driver_id = ? AND expires_at = ?")
            .bind(&request)
            .bind(driver)
            .bind(expires_at)
            .execute(pool)
            .await?;
        if let (1, Some(request)) = (done.rows_affected(), RequestId::parse(&request)) {
            removed.push(Removed { request, client: UserId(client), client_msg_id });
        }
    }
    Ok(removed)
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
            pickup_approx: false,
            dropoff_approx: false,
            dropoff_note: None,
            reference: None,
            km: 2.0,
            minutes: 6.0,
            price: 400,
            payment: PaymentMethod::Cash,
        };
        let fares = Fares { base: 100, per_km: 150, min_percent: 80 };
        let (r, _) = Request::open(RequestId::parse(id).unwrap(), new, &fares, 0).unwrap();
        requests::insert(pool, &r).await.unwrap();
        r.id
    }

    #[tokio::test]
    async fn a_new_counteroffer_replaces_the_previous_one_and_its_message() {
        let (pool, _dir) = test_pool().await;
        let id = open_request(&pool, "AB7K", 1).await;
        assert!(upsert(&pool, &id, UserId(9), 450, 300).await.unwrap());
        assert!(set_msg(&pool, &id, UserId(9), 300, 77).await.unwrap());
        assert!(upsert(&pool, &id, UserId(9), 500, 400).await.unwrap());
        // A late message id for the replaced offer is refused.
        assert!(!set_msg(&pool, &id, UserId(9), 300, 78).await.unwrap());
        let offers = for_request(&pool, &id).await.unwrap();
        let expected = Counteroffer { driver: UserId(9), amount: 500, expires_at: 400, client_msg_id: None };
        assert_eq!(offers, vec![expected]);
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
    async fn withdrawing_and_expiring_return_the_messages_to_delete() {
        let (pool, _dir) = test_pool().await;
        let (a, b) = (open_request(&pool, "AB7K", 1).await, open_request(&pool, "CD8M", 2).await);
        upsert(&pool, &a, UserId(9), 450, 300).await.unwrap();
        upsert(&pool, &b, UserId(9), 450, 300).await.unwrap();
        upsert(&pool, &b, UserId(8), 450, 100).await.unwrap();
        set_msg(&pool, &b, UserId(8), 100, 55).await.unwrap();

        let expired = take_expired(&pool, 100).await.unwrap();
        assert_eq!(expired, vec![Removed { request: b.clone(), client: UserId(2), client_msg_id: Some(55) }]);

        let mut withdrawn: Vec<_> = withdraw_driver(&pool, UserId(9)).await.unwrap().into_iter().map(|r| r.request).collect();
        withdrawn.sort_by(|x, y| x.as_str().cmp(y.as_str()));
        assert_eq!(withdrawn, vec![a.clone(), b.clone()]);
        assert!(for_request(&pool, &a).await.unwrap().is_empty());
    }
}
