use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{PaymentMethod, Point, Request, State, UnixTime};

#[derive(Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    /// The client already has an open request.
    ClientHasOpenRequest,
    /// The generated id is in use; generate another and retry.
    IdTaken,
}

pub async fn insert(pool: &SqlitePool, r: &Request) -> sqlx::Result<InsertOutcome> {
    let result = sqlx::query(
        "INSERT INTO requests (id, client_id, client_phone, pickup_lat, pickup_lon,
             dropoff_lat, dropoff_lon, reference, km, price, payment, state, driver_id,
             channel_msg_id, created_at, assigned_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(r.id.as_str())
    .bind(r.client.0)
    .bind(&r.client_phone)
    .bind(r.pickup.lat)
    .bind(r.pickup.lon)
    .bind(r.dropoff.lat)
    .bind(r.dropoff.lon)
    .bind(&r.reference)
    .bind(r.km)
    .bind(r.price)
    .bind(r.payment.as_str())
    .bind(r.state.as_str())
    .bind(r.driver.map(|d| d.0))
    .bind(r.channel_msg_id)
    .bind(r.created_at)
    .bind(r.assigned_at)
    .bind(r.expires_at)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(InsertOutcome::Inserted),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            if e.message().contains("requests.id") {
                Ok(InsertOutcome::IdTaken)
            } else {
                Ok(InsertOutcome::ClientHasOpenRequest)
            }
        }
        Err(e) => Err(e),
    }
}

pub async fn get(pool: &SqlitePool, id: &RequestId) -> sqlx::Result<Option<Request>> {
    sqlx::query("SELECT * FROM requests WHERE id = ?")
        .bind(id.as_str())
        .fetch_optional(pool)
        .await?
        .map(|row| from_row(&row))
        .transpose()
}

/// Assigns the request to `driver` only if it is still open, not expired, and
/// the driver has no other assigned trip. Exactly one concurrent caller wins.
pub async fn try_assign(
    pool: &SqlitePool,
    id: &RequestId,
    driver: UserId,
    now: UnixTime,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE requests SET state = 'assigned', driver_id = ?, assigned_at = ?
         WHERE id = ? AND state = 'open' AND expires_at > ? AND client_id != ?
           AND NOT EXISTS (SELECT 1 FROM requests WHERE driver_id = ? AND state = 'assigned')",
    )
    .bind(driver.0)
    .bind(now)
    .bind(id.as_str())
    .bind(now)
    .bind(driver.0)
    .bind(driver.0)
    .execute(pool)
    .await;

    match result {
        Ok(done) => Ok(done.rows_affected() == 1),
        // The partial unique index caught a race with another request for the same driver.
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e),
    }
}

pub async fn set_channel_msg(pool: &SqlitePool, id: &RequestId, msg_id: i32) -> sqlx::Result<()> {
    sqlx::query("UPDATE requests SET channel_msg_id = ? WHERE id = ?")
        .bind(msg_id)
        .bind(id.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

/// Deletes the request only if it is still in `expected` state, so that two
/// concurrent closes (Completed twice, a cancel racing an acceptance, the
/// sweeper racing a user) apply their effects once. Returns whether it won.
pub async fn close(pool: &SqlitePool, id: &RequestId, expected: State) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM requests WHERE id = ? AND state = ?")
        .bind(id.as_str())
        .bind(expected.as_str())
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// The open or assigned request where `user` is the client, or the assigned
/// trip where `user` is the driver.
pub async fn active_for(pool: &SqlitePool, user: UserId) -> sqlx::Result<Option<Request>> {
    sqlx::query(
        "SELECT * FROM requests
         WHERE (client_id = ? AND state IN ('open', 'assigned'))
            OR (driver_id = ? AND state = 'assigned')
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user.0)
    .bind(user.0)
    .fetch_optional(pool)
    .await?
    .map(|row| from_row(&row))
    .transpose()
}

/// Whether the driver already holds an assigned trip.
pub async fn driver_busy(pool: &SqlitePool, driver: UserId) -> sqlx::Result<bool> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM requests WHERE driver_id = ? AND state = 'assigned'")
            .bind(driver.0)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

/// Removes a closed request. Nothing about the trip is kept.
pub async fn forget(pool: &SqlitePool, id: &RequestId) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM requests WHERE id = ?")
        .bind(id.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

/// Requests the sweeper must look at: open ones past their round, and assigned
/// ones old enough to be auto-completed.
pub async fn due(pool: &SqlitePool, now: UnixTime, auto_complete_secs: i64) -> sqlx::Result<Vec<Request>> {
    sqlx::query(
        "SELECT * FROM requests
         WHERE (state = 'open' AND expires_at <= ?)
            OR (state = 'assigned' AND assigned_at <= ?)",
    )
    .bind(now)
    .bind(now - auto_complete_secs)
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}

fn from_row(row: &SqliteRow) -> sqlx::Result<Request> {
    let decode_err = |what: &str| sqlx::Error::Decode(format!("invalid {what}").into());
    let id: String = row.try_get("id")?;
    let state: String = row.try_get("state")?;
    let payment: String = row.try_get("payment")?;
    Ok(Request {
        id: RequestId::parse(&id).ok_or_else(|| decode_err("request id"))?,
        client: UserId(row.try_get("client_id")?),
        client_phone: row.try_get("client_phone")?,
        pickup: Point { lat: row.try_get("pickup_lat")?, lon: row.try_get("pickup_lon")? },
        dropoff: Point { lat: row.try_get("dropoff_lat")?, lon: row.try_get("dropoff_lon")? },
        reference: row.try_get("reference")?,
        km: row.try_get("km")?,
        price: row.try_get("price")?,
        payment: PaymentMethod::parse(&payment).ok_or_else(|| decode_err("payment"))?,
        state: State::parse(&state).ok_or_else(|| decode_err("state"))?,
        driver: row.try_get::<Option<i64>, _>("driver_id")?.map(UserId),
        channel_msg_id: row.try_get("channel_msg_id")?,
        created_at: row.try_get("created_at")?,
        assigned_at: row.try_get("assigned_at")?,
        expires_at: row.try_get("expires_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::domain::fare::Fares;
    use crate::domain::request::{NewRequest, ROUND_SECS};

    const T0: UnixTime = 1_000_000;

    fn request(id: &str, client: i64) -> Request {
        let new = NewRequest {
            client: UserId(client),
            client_phone: Some("+5350000000".into()),
            pickup: Point { lat: 23.13, lon: -82.38 },
            dropoff: Point { lat: 23.12, lon: -82.40 },
            reference: None,
            km: 2.0,
            price: 400,
            payment: PaymentMethod::Transfer,
        };
        let fares = Fares { base: 100, per_km: 150, min_percent: 80 };
        Request::open(RequestId::parse(id).unwrap(), new, &fares, T0).unwrap().0
    }

    #[tokio::test]
    async fn insert_and_get_round_trip() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        assert_eq!(insert(&pool, &r).await.unwrap(), InsertOutcome::Inserted);
        assert_eq!(get(&pool, &r.id).await.unwrap(), Some(r));
    }

    #[tokio::test]
    async fn one_open_request_per_client_and_unique_ids() {
        let (pool, _dir) = test_pool().await;
        insert(&pool, &request("AB7K", 1)).await.unwrap();
        assert_eq!(insert(&pool, &request("CD8M", 1)).await.unwrap(), InsertOutcome::ClientHasOpenRequest);
        assert_eq!(insert(&pool, &request("AB7K", 2)).await.unwrap(), InsertOutcome::IdTaken);
    }

    #[tokio::test]
    async fn exactly_one_of_many_concurrent_drivers_wins() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();

        let attempts: Vec<_> = (100..120)
            .map(|driver| {
                let (pool, id) = (pool.clone(), r.id.clone());
                tokio::spawn(async move { try_assign(&pool, &id, UserId(driver), T0 + 1).await.unwrap() })
            })
            .collect();
        let mut winners = 0;
        for attempt in attempts {
            winners += attempt.await.unwrap() as usize;
        }
        assert_eq!(winners, 1);
        assert_eq!(get(&pool, &r.id).await.unwrap().unwrap().state, State::Assigned);
    }

    #[tokio::test]
    async fn a_busy_driver_cannot_take_another_trip() {
        let (pool, _dir) = test_pool().await;
        let (a, b) = (request("AB7K", 1), request("CD8M", 2));
        insert(&pool, &a).await.unwrap();
        insert(&pool, &b).await.unwrap();
        assert!(try_assign(&pool, &a.id, UserId(9), T0 + 1).await.unwrap());
        assert!(!try_assign(&pool, &b.id, UserId(9), T0 + 1).await.unwrap());
    }

    #[tokio::test]
    async fn cannot_assign_expired_or_own_requests() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert!(!try_assign(&pool, &r.id, UserId(1), T0 + 1).await.unwrap());
        assert!(!try_assign(&pool, &r.id, UserId(9), T0 + ROUND_SECS).await.unwrap());
    }

    #[tokio::test]
    async fn close_wins_once_and_only_from_the_expected_state() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        try_assign(&pool, &r.id, UserId(9), T0 + 1).await.unwrap();
        assert!(!close(&pool, &r.id, State::Open).await.unwrap());
        assert!(close(&pool, &r.id, State::Assigned).await.unwrap());
        assert!(!close(&pool, &r.id, State::Assigned).await.unwrap());
    }

    #[tokio::test]
    async fn active_for_finds_clients_and_drivers() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert_eq!(active_for(&pool, UserId(1)).await.unwrap().unwrap().id, r.id);
        assert!(active_for(&pool, UserId(9)).await.unwrap().is_none());
        assert!(!driver_busy(&pool, UserId(9)).await.unwrap());
        try_assign(&pool, &r.id, UserId(9), T0 + 1).await.unwrap();
        assert_eq!(active_for(&pool, UserId(9)).await.unwrap().unwrap().id, r.id);
        assert!(driver_busy(&pool, UserId(9)).await.unwrap());
    }

    #[tokio::test]
    async fn due_finds_expired_and_stale_requests_and_forget_deletes() {
        let (pool, _dir) = test_pool().await;
        let (a, b) = (request("AB7K", 1), request("CD8M", 2));
        insert(&pool, &a).await.unwrap();
        insert(&pool, &b).await.unwrap();
        try_assign(&pool, &b.id, UserId(9), T0 + 1).await.unwrap();

        assert!(due(&pool, T0 + 10, 7200).await.unwrap().is_empty());
        let ids: Vec<_> = due(&pool, T0 + 7201, 7200).await.unwrap().into_iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), 2);

        forget(&pool, &a.id).await.unwrap();
        assert_eq!(get(&pool, &a.id).await.unwrap(), None);
    }
}
