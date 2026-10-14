//! Aggregate reputation, blind pending ratings and recent pairs.

use sqlx::SqlitePool;

use crate::domain::ids::UserId;
use crate::domain::reputation::{PAIR_WINDOW_SECS, Rating, Reputation, Role, Tag};
use crate::domain::request::UnixTime;

/// Records the first time someone used the bot in a role (for "days in the bot").
pub async fn touch(pool: &SqlitePool, user: UserId, role: Role, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO reputation (telegram_id, role, first_use) VALUES (?, ?, ?)")
        .bind(user.0)
        .bind(role.as_str())
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, user: UserId, role: Role) -> sqlx::Result<Option<Reputation>> {
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT first_use, trips, stars_sum, stars_count FROM reputation WHERE telegram_id = ? AND role = ?",
    )
    .bind(user.0)
    .bind(role.as_str())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(first_use, trips, stars_sum, stars_count)| Reputation { first_use, trips, stars_sum, stars_count }))
}

pub async fn add_trip(pool: &SqlitePool, user: UserId, role: Role, now: UnixTime) -> sqlx::Result<()> {
    touch(pool, user, role, now).await?;
    sqlx::query("UPDATE reputation SET trips = trips + 1 WHERE telegram_id = ? AND role = ?")
        .bind(user.0)
        .bind(role.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_rating(pool: &SqlitePool, user: UserId, role: Role, rating: Rating, now: UnixTime) -> sqlx::Result<()> {
    touch(pool, user, role, now).await?;
    let tag = |t: Tag| i64::from(rating.has(t));
    sqlx::query(
        "UPDATE reputation SET stars_sum = stars_sum + ?, stars_count = stars_count + 1,
             tag_punctual = tag_punctual + ?, tag_kind = tag_kind + ?,
             tag_overcharged = tag_overcharged + ?, tag_no_show = tag_no_show + ?
         WHERE telegram_id = ? AND role = ?",
    )
    .bind(rating.stars)
    .bind(tag(Tag::Punctual))
    .bind(tag(Tag::Kind))
    .bind(tag(Tag::Overcharged))
    .bind(tag(Tag::NoShow))
    .bind(user.0)
    .bind(role.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

/// How many ratings carried each tag, in `Tag::ALL` order.
pub async fn tags(pool: &SqlitePool, user: UserId, role: Role) -> sqlx::Result<Vec<(Tag, i64)>> {
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT tag_punctual, tag_kind, tag_overcharged, tag_no_show FROM reputation WHERE telegram_id = ? AND role = ?",
    )
    .bind(user.0)
    .bind(role.as_str())
    .fetch_optional(pool)
    .await?;
    let (p, k, o, n) = row.unwrap_or_default();
    Ok(Tag::ALL.into_iter().zip([p, k, o, n]).collect())
}

/// Whether this pair's trip counts: true the first time in 7 days (and the
/// window starts), false while a previous one is recent.
pub async fn pair_counts(pool: &SqlitePool, pair_hmac: &str, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "INSERT INTO recent_pairs (pair_hmac, expires_at) VALUES (?, ?)
         ON CONFLICT (pair_hmac) DO UPDATE SET expires_at = excluded.expires_at
         WHERE recent_pairs.expires_at <= ?",
    )
    .bind(pair_hmac)
    .bind(now + PAIR_WINDOW_SECS)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn delete_expired_pairs(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM recent_pairs WHERE expires_at <= ?").bind(now).execute(pool).await?;
    Ok(())
}

/// A finished trip's blind ratings, until both sides rated or 24 h passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub code: String,
    pub client: UserId,
    pub driver: UserId,
    pub counts: bool,
    /// The client's rating of the driver.
    pub by_client: Side,
    /// The driver's rating of the client.
    pub by_driver: Side,
    pub expires_at: UnixTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Side {
    pub stars: Option<i64>,
    pub tags: i64,
    pub done: bool,
}

impl Side {
    pub fn rating(&self) -> Option<Rating> {
        self.stars.map(|stars| Rating { stars, tags: self.tags })
    }
}

impl Pending {
    pub fn side(&self, rater: Role) -> Side {
        match rater {
            Role::Client => self.by_client,
            Role::Driver => self.by_driver,
        }
    }

    pub fn rater_role(&self, user: UserId) -> Option<Role> {
        if user == self.client {
            Some(Role::Client)
        } else if user == self.driver {
            Some(Role::Driver)
        } else {
            None
        }
    }
}

pub async fn create_pending(
    pool: &SqlitePool,
    code: &str,
    client: UserId,
    driver: UserId,
    counts: bool,
    expires_at: UnixTime,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO pending_ratings (code, client_id, driver_id, counts, expires_at) VALUES (?, ?, ?, ?, ?)")
        .bind(code)
        .bind(client.0)
        .bind(driver.0)
        .bind(counts)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(())
}

/// `code, client_id, driver_id, counts, client_stars, client_tags, client_done,
/// driver_stars, driver_tags, driver_done, expires_at`
type PendingRow = (String, i64, i64, bool, Option<i64>, i64, bool, Option<i64>, i64, bool, i64);

fn pending_from(row: PendingRow) -> Pending {
    let (code, client, driver, counts, cs, ct, cd, ds, dt, dd, expires_at) = row;
    Pending {
        code,
        client: UserId(client),
        driver: UserId(driver),
        counts,
        by_client: Side { stars: cs, tags: ct, done: cd },
        by_driver: Side { stars: ds, tags: dt, done: dd },
        expires_at,
    }
}

pub async fn get_pending(pool: &SqlitePool, code: &str) -> sqlx::Result<Option<Pending>> {
    let row: Option<PendingRow> = sqlx::query_as(
        "SELECT code, client_id, driver_id, counts, client_stars, client_tags, client_done,
             driver_stars, driver_tags, driver_done, expires_at
         FROM pending_ratings WHERE code = ?",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(pending_from))
}

/// Sets the stars of one side, while it is not finished.
pub async fn set_stars(pool: &SqlitePool, code: &str, rater: Role, stars: i64) -> sqlx::Result<bool> {
    let sql = match rater {
        Role::Client => "UPDATE pending_ratings SET client_stars = ? WHERE code = ? AND client_done = 0",
        Role::Driver => "UPDATE pending_ratings SET driver_stars = ? WHERE code = ? AND driver_done = 0",
    };
    let done = sqlx::query(sql).bind(stars).bind(code).execute(pool).await?;
    Ok(done.rows_affected() == 1)
}

/// Turns a tag on or off for one side, while it is not finished. SQLite has
/// no XOR: `(a | b) - (a & b)` is the same.
pub async fn toggle_tag(pool: &SqlitePool, code: &str, rater: Role, tag: Tag) -> sqlx::Result<bool> {
    let sql = match rater {
        Role::Client => {
            "UPDATE pending_ratings SET client_tags = (client_tags | ?1) - (client_tags & ?1)
             WHERE code = ?2 AND client_done = 0"
        }
        Role::Driver => {
            "UPDATE pending_ratings SET driver_tags = (driver_tags | ?1) - (driver_tags & ?1)
             WHERE code = ?2 AND driver_done = 0"
        }
    };
    let done = sqlx::query(sql).bind(tag.bit()).bind(code).execute(pool).await?;
    Ok(done.rows_affected() == 1)
}

/// Closes one side's rating once it has stars.
pub async fn finish_side(pool: &SqlitePool, code: &str, rater: Role) -> sqlx::Result<bool> {
    let sql = match rater {
        Role::Client => {
            "UPDATE pending_ratings SET client_done = 1 WHERE code = ? AND client_done = 0 AND client_stars IS NOT NULL"
        }
        Role::Driver => {
            "UPDATE pending_ratings SET driver_done = 1 WHERE code = ? AND driver_done = 0 AND driver_stars IS NOT NULL"
        }
    };
    let done = sqlx::query(sql).bind(code).execute(pool).await?;
    Ok(done.rows_affected() == 1)
}

/// Codes ready to be applied: both sides done, or the 24 hours passed.
pub async fn due_pending(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT code FROM pending_ratings WHERE (client_done = 1 AND driver_done = 1) OR expires_at <= ?",
    )
    .bind(now)
    .fetch_all(pool)
    .await
}

/// Deletes the pending ratings and returns them; only one caller gets them.
pub async fn take_pending(pool: &SqlitePool, code: &str) -> sqlx::Result<Option<Pending>> {
    let row: Option<PendingRow> = sqlx::query_as(
        "DELETE FROM pending_ratings WHERE code = ?
         RETURNING code, client_id, driver_id, counts, client_stars, client_tags, client_done,
             driver_stars, driver_tags, driver_done, expires_at",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(pending_from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn trips_and_ratings_add_up() {
        let (pool, _dir) = test_pool().await;
        let u = UserId(5);
        touch(&pool, u, Role::Client, 100).await.unwrap();
        touch(&pool, u, Role::Client, 999).await.unwrap(); // first use stays
        add_trip(&pool, u, Role::Client, 200).await.unwrap();
        add_rating(&pool, u, Role::Client, Rating { stars: 4, tags: Tag::Kind.bit() }, 200).await.unwrap();
        add_rating(&pool, u, Role::Client, Rating { stars: 5, tags: 0 }, 200).await.unwrap();
        let r = get(&pool, u, Role::Client).await.unwrap().unwrap();
        assert_eq!(r, Reputation { first_use: 100, trips: 1, stars_sum: 9, stars_count: 2 });
        assert_eq!(get(&pool, u, Role::Driver).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_pair_counts_once_a_week() {
        let (pool, _dir) = test_pool().await;
        assert!(pair_counts(&pool, "abc", 0).await.unwrap());
        assert!(!pair_counts(&pool, "abc", PAIR_WINDOW_SECS - 1).await.unwrap());
        assert!(pair_counts(&pool, "abc", PAIR_WINDOW_SECS).await.unwrap());
    }

    #[tokio::test]
    async fn pending_ratings_are_blind_until_both_finish() {
        let (pool, _dir) = test_pool().await;
        create_pending(&pool, "R1", UserId(1), UserId(2), true, 1000).await.unwrap();
        assert!(!finish_side(&pool, "R1", Role::Client).await.unwrap()); // no stars yet
        assert!(set_stars(&pool, "R1", Role::Client, 5).await.unwrap());
        assert!(toggle_tag(&pool, "R1", Role::Client, Tag::Punctual).await.unwrap());
        assert!(toggle_tag(&pool, "R1", Role::Client, Tag::Kind).await.unwrap());
        assert!(toggle_tag(&pool, "R1", Role::Client, Tag::Kind).await.unwrap()); // off again
        assert!(finish_side(&pool, "R1", Role::Client).await.unwrap());
        assert!(!set_stars(&pool, "R1", Role::Client, 1).await.unwrap()); // finished: frozen
        assert!(due_pending(&pool, 10).await.unwrap().is_empty());

        set_stars(&pool, "R1", Role::Driver, 3).await.unwrap();
        finish_side(&pool, "R1", Role::Driver).await.unwrap();
        assert_eq!(due_pending(&pool, 10).await.unwrap(), vec!["R1".to_owned()]);

        let p = take_pending(&pool, "R1").await.unwrap().unwrap();
        assert_eq!(p.by_client.rating(), Some(Rating { stars: 5, tags: Tag::Punctual.bit() }));
        assert_eq!(p.by_driver.rating(), Some(Rating { stars: 3, tags: 0 }));
        assert_eq!(take_pending(&pool, "R1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn unfinished_ratings_are_due_after_24_hours() {
        let (pool, _dir) = test_pool().await;
        create_pending(&pool, "R1", UserId(1), UserId(2), true, 1000).await.unwrap();
        assert!(due_pending(&pool, 999).await.unwrap().is_empty());
        assert_eq!(due_pending(&pool, 1000).await.unwrap().len(), 1);
    }
}
