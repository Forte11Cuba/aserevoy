use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use super::counteroffers;
use crate::domain::fare::Cup;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{PaymentMethod, Point, Request, State, UnixTime};

#[derive(Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    /// The client already has a request in the channel.
    ClientHasOpenRequest,
    /// The generated id is in use; generate another and retry.
    IdTaken,
}

pub async fn insert(pool: &SqlitePool, r: &Request) -> sqlx::Result<InsertOutcome> {
    let result = sqlx::query(
        "INSERT INTO requests (id, client_id, client_phone, pickup_lat, pickup_lon,
             dropoff_lat, dropoff_lon, reference, km, price, payment, state, driver_id,
             channel_msg_id, created_at, assigned_at, check_at, answer_by, confirm_by, on_my_way_at,
             arrived_at, expires_at, pickup_approx, dropoff_approx, dropoff_note, suggested, minutes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(r.check_at)
    .bind(r.answer_by)
    .bind(r.confirm_by)
    .bind(r.on_my_way_at)
    .bind(r.arrived_at)
    .bind(r.expires_at)
    .bind(r.pickup_approx)
    .bind(r.dropoff_approx)
    .bind(&r.dropoff_note)
    .bind(r.suggested)
    .bind(r.minutes)
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

/// The request with its active counteroffers.
pub async fn get(pool: &SqlitePool, id: &RequestId) -> sqlx::Result<Option<Request>> {
    let row = sqlx::query("SELECT * FROM requests WHERE id = ?")
        .bind(id.as_str())
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else { return Ok(None) };
    let mut request = from_row(&row)?;
    request.counteroffers = counteroffers::for_request(pool, id).await?;
    Ok(Some(request))
}

/// The driver's hold on a free driver: reserves the request for them while the
/// client confirms, only if it is still open and not expired and the driver
/// holds nothing else. Exactly one concurrent caller wins.
pub async fn try_reserve(
    pool: &SqlitePool,
    id: &RequestId,
    driver: UserId,
    confirm_by: UnixTime,
    now: UnixTime,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE requests SET state = 'confirming', driver_id = ?, confirm_by = ?, answer_by = NULL
         WHERE id = ? AND state = 'open' AND expires_at > ? AND client_id != ?
           AND NOT EXISTS (SELECT 1 FROM requests WHERE driver_id = ? AND state IN ('confirming', 'assigned'))",
    )
    .bind(driver.0)
    .bind(confirm_by)
    .bind(id.as_str())
    .bind(now)
    .bind(driver.0)
    .bind(driver.0)
    .execute(pool)
    .await;
    affected_one(result)
}

/// `confirming` → `assigned`, for the driver the client saw and within the deadline.
pub async fn confirm(pool: &SqlitePool, id: &RequestId, driver: UserId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE requests SET state = 'assigned', assigned_at = ?, confirm_by = NULL
         WHERE id = ? AND state = 'confirming' AND driver_id = ? AND confirm_by > ?",
    )
    .bind(now)
    .bind(id.as_str())
    .bind(driver.0)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// `confirming` → `open`: the reserved driver withdraws.
pub async fn release(pool: &SqlitePool, id: &RequestId, driver: UserId) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE requests SET state = 'open', driver_id = NULL, confirm_by = NULL
         WHERE id = ? AND state = 'confirming' AND driver_id = ?",
    )
    .bind(id.as_str())
    .bind(driver.0)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The client takes a counteroffer: assigns at its amount if the offer is
/// still valid and the driver holds nothing else. Exactly one caller wins.
pub async fn assign_counteroffer(
    pool: &SqlitePool,
    id: &RequestId,
    driver: UserId,
    amount: Cup,
    now: UnixTime,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE requests SET state = 'assigned', driver_id = ?, price = ?, assigned_at = ?, answer_by = NULL
         WHERE id = ? AND state = 'open' AND expires_at > ?
           AND EXISTS (SELECT 1 FROM counteroffers
                       WHERE request_id = ? AND driver_id = ? AND amount = ? AND expires_at > ?)
           AND NOT EXISTS (SELECT 1 FROM requests WHERE driver_id = ? AND state IN ('confirming', 'assigned'))",
    )
    .bind(driver.0)
    .bind(amount)
    .bind(now)
    .bind(id.as_str())
    .bind(now)
    .bind(id.as_str())
    .bind(driver.0)
    .bind(amount)
    .bind(now)
    .bind(driver.0)
    .execute(pool)
    .await;
    affected_one(result)
}

/// Raises the price of an open request; also answers "still waiting?".
pub async fn raise_price(pool: &SqlitePool, id: &RequestId, price: Cup) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE requests SET price = ?, answer_by = NULL WHERE id = ? AND state = 'open' AND price < ?",
    )
    .bind(price)
    .bind(id.as_str())
    .bind(price)
    .execute(pool)
    .await?;
    if done.rows_affected() == 1 {
        counteroffers::delete_at_or_below(pool, id, price).await?;
    }
    Ok(done.rows_affected() == 1)
}

/// Saves that "still waiting?" was asked, unless something changed meanwhile.
pub async fn ask_still_waiting(pool: &SqlitePool, id: &RequestId, answer_by: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE requests SET check_at = NULL, answer_by = ?
         WHERE id = ? AND state = 'open' AND check_at IS NOT NULL AND answer_by IS NULL",
    )
    .bind(answer_by)
    .bind(id.as_str())
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The client answered "yes, still waiting".
pub async fn keep_waiting(pool: &SqlitePool, id: &RequestId) -> sqlx::Result<bool> {
    let done = sqlx::query("UPDATE requests SET answer_by = NULL WHERE id = ? AND state = 'open'")
        .bind(id.as_str())
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

fn affected_one(result: sqlx::Result<sqlx::sqlite::SqliteQueryResult>) -> sqlx::Result<bool> {
    match result {
        Ok(done) => Ok(done.rows_affected() == 1),
        // A partial unique index caught a race (the same driver holding two requests).
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

/// The client's card to confirm the driver; `None` once it is gone.
pub async fn set_confirm_msg(pool: &SqlitePool, id: &RequestId, msg_id: Option<i32>) -> sqlx::Result<()> {
    sqlx::query("UPDATE requests SET confirm_msg_id = ? WHERE id = ?").bind(msg_id).bind(id.as_str()).execute(pool).await?;
    Ok(())
}

/// The post left the channel (the trip was taken): nothing to delete at the end.
pub async fn clear_channel_msg(pool: &SqlitePool, id: &RequestId) -> sqlx::Result<()> {
    sqlx::query("UPDATE requests SET channel_msg_id = NULL WHERE id = ?").bind(id.as_str()).execute(pool).await?;
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

/// 🚶 On my way, once, by the client of an assigned trip.
pub async fn mark_on_my_way(pool: &SqlitePool, id: &RequestId, client: UserId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE requests SET on_my_way_at = ?
         WHERE id = ? AND state = 'assigned' AND client_id = ? AND on_my_way_at IS NULL",
    )
    .bind(now)
    .bind(id.as_str())
    .bind(client.0)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// 📍 Arrived, once, by the driver of an assigned trip.
pub async fn mark_arrived(pool: &SqlitePool, id: &RequestId, driver: UserId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE requests SET arrived_at = ?
         WHERE id = ? AND state = 'assigned' AND driver_id = ? AND arrived_at IS NULL",
    )
    .bind(now)
    .bind(id.as_str())
    .bind(driver.0)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The client's request in the channel or on a trip, or the request the
/// driver holds or drives.
pub async fn active_for(pool: &SqlitePool, user: UserId) -> sqlx::Result<Option<Request>> {
    let id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM requests
         WHERE (client_id = ? AND state IN ('open', 'confirming', 'assigned'))
            OR (driver_id = ? AND state IN ('confirming', 'assigned'))
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user.0)
    .bind(user.0)
    .fetch_optional(pool)
    .await?;
    match id.as_deref().and_then(RequestId::parse) {
        Some(id) => get(pool, &id).await,
        None => Ok(None),
    }
}

/// Whether the driver holds a request (waiting for confirmation or on a trip).
pub async fn driver_busy(pool: &SqlitePool, driver: UserId) -> sqlx::Result<bool> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM requests WHERE driver_id = ? AND state IN ('confirming', 'assigned')",
    )
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

/// Requests with a deadline due: open ones past their 60 minutes, their
/// "still waiting?" time or its answer deadline; unconfirmed drivers; and
/// assigned trips old enough to be auto-completed.
pub async fn due(pool: &SqlitePool, now: UnixTime, auto_complete_secs: i64) -> sqlx::Result<Vec<Request>> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM requests
         WHERE (state = 'open' AND (expires_at <= ? OR answer_by <= ? OR (check_at <= ? AND answer_by IS NULL)))
            OR (state = 'confirming' AND confirm_by <= ?)
            OR (state = 'assigned' AND assigned_at <= ?)",
    )
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(now - auto_complete_secs)
    .fetch_all(pool)
    .await?;
    let mut requests = Vec::new();
    for id in ids.iter().filter_map(|id| RequestId::parse(id)) {
        if let Some(request) = get(pool, &id).await? {
            requests.push(request);
        }
    }
    Ok(requests)
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
        pickup_approx: row.try_get("pickup_approx")?,
        dropoff_approx: row.try_get("dropoff_approx")?,
        reference: row.try_get("reference")?,
        dropoff_note: row.try_get("dropoff_note")?,
        suggested: row.try_get("suggested")?,
        km: row.try_get("km")?,
        minutes: row.try_get("minutes")?,
        price: row.try_get("price")?,
        payment: PaymentMethod::parse(&payment).ok_or_else(|| decode_err("payment"))?,
        state: State::parse(&state).ok_or_else(|| decode_err("state"))?,
        driver: row.try_get::<Option<i64>, _>("driver_id")?.map(UserId),
        channel_msg_id: row.try_get("channel_msg_id")?,
        confirm_msg_id: row.try_get("confirm_msg_id")?,
        created_at: row.try_get("created_at")?,
        assigned_at: row.try_get("assigned_at")?,
        check_at: row.try_get("check_at")?,
        answer_by: row.try_get("answer_by")?,
        confirm_by: row.try_get("confirm_by")?,
        on_my_way_at: row.try_get("on_my_way_at")?,
        arrived_at: row.try_get("arrived_at")?,
        expires_at: row.try_get("expires_at")?,
        counteroffers: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::domain::fare::Fares;
    use crate::domain::request::{CONFIRM_SECS, NewRequest, REQUEST_TTL_SECS, STILL_WAITING_AFTER_SECS};

    const T0: UnixTime = 1_000_000;

    fn request(id: &str, client: i64) -> Request {
        let new = NewRequest {
            client: UserId(client),
            client_phone: Some("+5350000000".into()),
            pickup: Point { lat: 23.13, lon: -82.38 },
            dropoff: Point { lat: 23.12, lon: -82.40 },
            pickup_approx: false,
            dropoff_approx: false,
            dropoff_note: None,
            reference: None,
            km: 2.0,
            minutes: 6.0,
            price: 400,
            payment: PaymentMethod::Transfer,
        };
        let fares = Fares { base: 100, per_km: 150, min_percent: 80 };
        Request::open(RequestId::parse(id).unwrap(), new, &fares, T0).unwrap().0
    }

    async fn reserve(pool: &SqlitePool, r: &Request, driver: i64) -> bool {
        try_reserve(pool, &r.id, UserId(driver), T0 + 1 + CONFIRM_SECS, T0 + 1).await.unwrap()
    }

    #[tokio::test]
    async fn insert_and_get_round_trip() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        assert_eq!(insert(&pool, &r).await.unwrap(), InsertOutcome::Inserted);
        assert_eq!(get(&pool, &r.id).await.unwrap(), Some(r));
    }

    #[tokio::test]
    async fn one_request_per_client_and_unique_ids() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert_eq!(insert(&pool, &request("CD8M", 1)).await.unwrap(), InsertOutcome::ClientHasOpenRequest);
        reserve(&pool, &r, 9).await;
        // Still one while a driver waits for confirmation.
        assert_eq!(insert(&pool, &request("CD8M", 1)).await.unwrap(), InsertOutcome::ClientHasOpenRequest);
        assert_eq!(insert(&pool, &request("AB7K", 2)).await.unwrap(), InsertOutcome::IdTaken);
    }

    #[tokio::test]
    async fn exactly_one_of_many_concurrent_drivers_reserves() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();

        let attempts: Vec<_> = (100..120)
            .map(|driver| {
                let (pool, r) = (pool.clone(), r.clone());
                tokio::spawn(async move { reserve(&pool, &r, driver).await })
            })
            .collect();
        let mut winners = 0;
        for attempt in attempts {
            winners += attempt.await.unwrap() as usize;
        }
        assert_eq!(winners, 1);
        assert_eq!(get(&pool, &r.id).await.unwrap().unwrap().state, State::Confirming);
    }

    #[tokio::test]
    async fn confirm_assigns_only_the_reserved_driver() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        reserve(&pool, &r, 9).await;
        assert!(!confirm(&pool, &r.id, UserId(8), T0 + 2).await.unwrap());
        assert!(!confirm(&pool, &r.id, UserId(9), T0 + 1 + CONFIRM_SECS).await.unwrap());
        assert!(confirm(&pool, &r.id, UserId(9), T0 + 2).await.unwrap());
        let stored = get(&pool, &r.id).await.unwrap().unwrap();
        assert_eq!((stored.state, stored.assigned_at, stored.confirm_by), (State::Assigned, Some(T0 + 2), None));
    }

    #[tokio::test]
    async fn on_my_way_and_arrived_are_saved_once_by_the_right_side() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert!(!mark_arrived(&pool, &r.id, UserId(9), T0 + 3).await.unwrap()); // not assigned yet
        reserve(&pool, &r, 9).await;
        confirm(&pool, &r.id, UserId(9), T0 + 2).await.unwrap();
        assert!(!mark_on_my_way(&pool, &r.id, UserId(9), T0 + 3).await.unwrap());
        assert!(mark_on_my_way(&pool, &r.id, UserId(1), T0 + 3).await.unwrap());
        assert!(!mark_on_my_way(&pool, &r.id, UserId(1), T0 + 4).await.unwrap());
        assert!(mark_arrived(&pool, &r.id, UserId(9), T0 + 5).await.unwrap());
        let stored = get(&pool, &r.id).await.unwrap().unwrap();
        assert_eq!((stored.on_my_way_at, stored.arrived_at), (Some(T0 + 3), Some(T0 + 5)));
    }

    #[tokio::test]
    async fn the_reserved_driver_can_release_the_request() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        reserve(&pool, &r, 9).await;
        assert!(!release(&pool, &r.id, UserId(8)).await.unwrap());
        assert!(release(&pool, &r.id, UserId(9)).await.unwrap());
        let stored = get(&pool, &r.id).await.unwrap().unwrap();
        assert_eq!((stored.state, stored.driver), (State::Open, None));
        assert!(!driver_busy(&pool, UserId(9)).await.unwrap());
        assert!(reserve(&pool, &r, 8).await);
    }

    #[tokio::test]
    async fn a_busy_driver_cannot_hold_another_request() {
        let (pool, _dir) = test_pool().await;
        let (a, b) = (request("AB7K", 1), request("CD8M", 2));
        insert(&pool, &a).await.unwrap();
        insert(&pool, &b).await.unwrap();
        assert!(reserve(&pool, &a, 9).await);
        assert!(driver_busy(&pool, UserId(9)).await.unwrap());
        assert!(!reserve(&pool, &b, 9).await);
    }

    #[tokio::test]
    async fn cannot_reserve_expired_or_own_requests() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert!(!reserve(&pool, &r, 1).await);
        let late = T0 + REQUEST_TTL_SECS;
        assert!(!try_reserve(&pool, &r.id, UserId(9), late + CONFIRM_SECS, late).await.unwrap());
    }

    #[tokio::test]
    async fn counteroffers_are_assigned_at_their_amount_while_valid() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        counteroffers::upsert(&pool, &r.id, UserId(9), 500, T0 + 300).await.unwrap();
        // Wrong amount or expired: no.
        assert!(!assign_counteroffer(&pool, &r.id, UserId(9), 450, T0 + 1).await.unwrap());
        assert!(!assign_counteroffer(&pool, &r.id, UserId(9), 500, T0 + 300).await.unwrap());
        assert!(assign_counteroffer(&pool, &r.id, UserId(9), 500, T0 + 1).await.unwrap());
        let stored = get(&pool, &r.id).await.unwrap().unwrap();
        assert_eq!((stored.state, stored.price, stored.driver), (State::Assigned, 500, Some(UserId(9))));
    }

    #[tokio::test]
    async fn raising_the_price_drops_cheaper_counteroffers() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        counteroffers::upsert(&pool, &r.id, UserId(8), 450, T0 + 300).await.unwrap();
        counteroffers::upsert(&pool, &r.id, UserId(9), 550, T0 + 300).await.unwrap();
        assert!(!raise_price(&pool, &r.id, 400).await.unwrap());
        assert!(raise_price(&pool, &r.id, 500).await.unwrap());
        let stored = get(&pool, &r.id).await.unwrap().unwrap();
        assert_eq!(stored.price, 500);
        assert_eq!(stored.counteroffers.len(), 1);
        assert_eq!(stored.counteroffers[0].driver, UserId(9));
    }

    #[tokio::test]
    async fn still_waiting_is_saved_once_and_answered() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert!(ask_still_waiting(&pool, &r.id, T0 + 2400).await.unwrap());
        assert!(!ask_still_waiting(&pool, &r.id, T0 + 2400).await.unwrap());
        assert!(keep_waiting(&pool, &r.id).await.unwrap());
        let stored = get(&pool, &r.id).await.unwrap().unwrap();
        assert_eq!((stored.check_at, stored.answer_by), (None, None));
    }

    #[tokio::test]
    async fn close_wins_once_and_only_from_the_expected_state() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        reserve(&pool, &r, 9).await;
        assert!(!close(&pool, &r.id, State::Open).await.unwrap());
        assert!(close(&pool, &r.id, State::Confirming).await.unwrap());
        assert!(!close(&pool, &r.id, State::Confirming).await.unwrap());
    }

    #[tokio::test]
    async fn closing_deletes_the_counteroffers() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        counteroffers::upsert(&pool, &r.id, UserId(9), 500, T0 + 300).await.unwrap();
        assert!(close(&pool, &r.id, State::Open).await.unwrap());
        assert!(counteroffers::for_request(&pool, &r.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn active_for_finds_clients_and_drivers() {
        let (pool, _dir) = test_pool().await;
        let r = request("AB7K", 1);
        insert(&pool, &r).await.unwrap();
        assert_eq!(active_for(&pool, UserId(1)).await.unwrap().unwrap().id, r.id);
        assert!(active_for(&pool, UserId(9)).await.unwrap().is_none());
        reserve(&pool, &r, 9).await;
        assert_eq!(active_for(&pool, UserId(9)).await.unwrap().unwrap().id, r.id);
    }

    #[tokio::test]
    async fn due_finds_every_kind_of_deadline() {
        let (pool, _dir) = test_pool().await;
        let (a, b, c) = (request("AB7K", 1), request("CD8M", 2), request("EF9N", 3));
        for r in [&a, &b, &c] {
            insert(&pool, r).await.unwrap();
        }
        reserve(&pool, &b, 9).await;
        reserve(&pool, &c, 10).await;
        confirm(&pool, &c.id, UserId(10), T0 + 2).await.unwrap();

        let ids = |now| {
            let pool = pool.clone();
            async move {
                let mut ids: Vec<_> =
                    due(&pool, now, 7200).await.unwrap().into_iter().map(|r| r.id.as_str().to_owned()).collect();
                ids.sort();
                ids
            }
        };
        assert!(ids(T0 + 10).await.is_empty());
        assert_eq!(ids(T0 + 1 + CONFIRM_SECS).await, vec!["CD8M"]);
        assert_eq!(ids(T0 + STILL_WAITING_AFTER_SECS).await, vec!["AB7K", "CD8M"]);
        assert_eq!(ids(T0 + 7202).await, vec!["AB7K", "CD8M", "EF9N"]);

        forget(&pool, &a.id).await.unwrap();
        assert_eq!(get(&pool, &a.id).await.unwrap(), None);
    }
}
