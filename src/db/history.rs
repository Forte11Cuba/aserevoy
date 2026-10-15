//! Trip history (45 days per trip) and drivers' monthly totals (for good).

use sqlx::SqlitePool;

use crate::crypto::PhoneCipher;
use crate::domain::ids::UserId;
use crate::domain::reputation::Role;
use crate::domain::fare::Cup;
use crate::domain::request::{PaymentMethod, UnixTime};

/// How long a trip stays in the history.
pub const HISTORY_SECS: i64 = 45 * 24 * 60 * 60;

/// One side of a completed trip, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub number: i64,
    pub request_id: String,
    pub completed_at: UnixTime,
    pub origin: String,
    pub destination: String,
    pub price: Cup,
    pub payment: PaymentMethod,
    /// The driver's alias for a client; the client's name for a driver.
    pub counterpart: String,
    /// The stars this user gave, once they rated.
    pub stars: Option<i64>,
}

pub async fn record(
    pool: &SqlitePool,
    cipher: &PhoneCipher,
    user: UserId,
    role: Role,
    e: &Entry,
    rating_code: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO trip_history (telegram_id, role, number, request_id, completed_at, origin_enc,
             destination_enc, price, payment, counterpart_enc, rating_code)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user.0)
    .bind(role.as_str())
    .bind(e.number)
    .bind(&e.request_id)
    .bind(e.completed_at)
    .bind(cipher.encrypt(&e.origin))
    .bind(cipher.encrypt(&e.destination))
    .bind(e.price)
    .bind(e.payment.as_str())
    .bind(cipher.encrypt(&e.counterpart))
    .bind(rating_code)
    .execute(pool)
    .await?;
    Ok(())
}

/// The stars this user gave for the trip of that rating.
pub async fn set_stars(pool: &SqlitePool, user: UserId, rating_code: &str, stars: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE trip_history SET stars = ? WHERE telegram_id = ? AND rating_code = ?")
        .bind(stars)
        .bind(user.0)
        .bind(rating_code)
        .execute(pool)
        .await?;
    Ok(())
}

/// `number, request_id, completed_at, origin_enc, destination_enc, price, payment, counterpart_enc, stars`
type Row = (i64, String, i64, Vec<u8>, Vec<u8>, i64, String, Vec<u8>, Option<i64>);

/// A page of the user's trips in a role, the most recent first, and how many
/// there are.
pub async fn page(
    pool: &SqlitePool,
    cipher: &PhoneCipher,
    user: UserId,
    role: Role,
    offset: i64,
    limit: i64,
) -> anyhow::Result<(Vec<Entry>, i64)> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT number, request_id, completed_at, origin_enc, destination_enc, price, payment, counterpart_enc, stars
         FROM trip_history WHERE telegram_id = ? AND role = ?
         ORDER BY completed_at DESC, id DESC LIMIT ? OFFSET ?",
    )
    .bind(user.0)
    .bind(role.as_str())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trip_history WHERE telegram_id = ? AND role = ?")
        .bind(user.0)
        .bind(role.as_str())
        .fetch_one(pool)
        .await?;
    let mut entries = Vec::with_capacity(rows.len());
    for (number, request_id, completed_at, origin, destination, price, payment, counterpart, stars) in rows {
        entries.push(Entry {
            number,
            request_id,
            completed_at,
            origin: cipher.decrypt(&origin)?,
            destination: cipher.decrypt(&destination)?,
            price,
            payment: PaymentMethod::parse(&payment).unwrap_or(PaymentMethod::Cash),
            counterpart: cipher.decrypt(&counterpart)?,
            stars,
        });
    }
    Ok((entries, total))
}

/// "🗑 Delete my history", in one role.
pub async fn delete_for(pool: &SqlitePool, user: UserId, role: Role) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM trip_history WHERE telegram_id = ? AND role = ?")
        .bind(user.0)
        .bind(role.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

/// Sweeper: trips older than 45 days leave the history.
pub async fn delete_old(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM trip_history WHERE completed_at <= ?").bind(now - HISTORY_SECS).execute(pool).await?;
    Ok(())
}

/// A driver's completed trip counts in its month (`YYYY-MM`).
pub async fn add_driver_month(pool: &SqlitePool, driver: UserId, month: &str, earned: Cup) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO driver_monthly (telegram_id, month, trips, earned) VALUES (?, ?, 1, ?)
         ON CONFLICT (telegram_id, month) DO UPDATE SET trips = trips + 1, earned = earned + excluded.earned",
    )
    .bind(driver.0)
    .bind(month)
    .bind(earned)
    .execute(pool)
    .await?;
    Ok(())
}

/// A driver's months, the most recent first: `(month, trips, earned)`.
pub async fn driver_months(pool: &SqlitePool, driver: UserId) -> sqlx::Result<Vec<(String, i64, Cup)>> {
    sqlx::query_as("SELECT month, trips, earned FROM driver_monthly WHERE telegram_id = ? ORDER BY month DESC")
        .bind(driver.0)
        .fetch_all(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    fn entry(number: i64, at: UnixTime) -> Entry {
        Entry {
            number,
            request_id: format!("R{number}"),
            completed_at: at,
            origin: "Plaza (23 y L)".into(),
            destination: "Habana del Este (Alamar)".into(),
            price: 3300,
            payment: PaymentMethod::Cash,
            counterpart: "Pepe".into(),
            stars: None,
        }
    }

    #[tokio::test]
    async fn history_is_newest_first_encrypted_and_expires() {
        let (pool, _dir) = test_pool().await;
        let c = PhoneCipher::from_hex(&"ab".repeat(32)).unwrap();
        let user = UserId(5);
        record(&pool, &c, user, Role::Client, &entry(1, 100), "a").await.unwrap();
        record(&pool, &c, user, Role::Client, &entry(2, 200), "b").await.unwrap();
        record(&pool, &c, user, Role::Driver, &entry(1, 150), "c").await.unwrap();
        set_stars(&pool, user, "b", 5).await.unwrap();

        let (entries, total) = page(&pool, &c, user, Role::Client, 0, 10).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(entries.iter().map(|e| e.number).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!((entries[0].stars, entries[1].stars), (Some(5), None));
        assert_eq!(entries[0].origin, "Plaza (23 y L)");
        let stored: Vec<u8> = sqlx::query_scalar("SELECT origin_enc FROM trip_history LIMIT 1").fetch_one(&pool).await.unwrap();
        assert!(!String::from_utf8_lossy(&stored).contains("Plaza"));

        delete_old(&pool, 160 + HISTORY_SECS).await.unwrap();
        assert_eq!(page(&pool, &c, user, Role::Client, 0, 10).await.unwrap().1, 1);
        assert_eq!(page(&pool, &c, user, Role::Driver, 0, 10).await.unwrap().1, 0);
    }

    #[tokio::test]
    async fn driver_months_add_up() {
        let (pool, _dir) = test_pool().await;
        let d = UserId(7);
        add_driver_month(&pool, d, "2026-09", 1000).await.unwrap();
        add_driver_month(&pool, d, "2026-10", 500).await.unwrap();
        add_driver_month(&pool, d, "2026-10", 700).await.unwrap();
        assert_eq!(
            driver_months(&pool, d).await.unwrap(),
            vec![("2026-10".to_owned(), 2, 1200), ("2026-09".to_owned(), 1, 1000)]
        );
    }
}
