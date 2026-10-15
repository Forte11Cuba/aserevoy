//! Cancellations that count against clients, and blocks.

use sqlx::SqlitePool;

use crate::domain::ids::UserId;
use crate::domain::reputation::{CANCELLATION_TTL_SECS, CANCELLATION_WINDOW_SECS, Role};
use crate::domain::request::UnixTime;

/// Records a cancellation against the client and returns how many they have
/// in the last 7 days, this one included.
pub async fn count_cancellation(pool: &SqlitePool, client: UserId, now: UnixTime) -> sqlx::Result<i64> {
    sqlx::query("INSERT INTO cancellations (telegram_id, role, created_at) VALUES (?, 'client', ?)")
        .bind(client.0)
        .bind(now)
        .execute(pool)
        .await?;
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM cancellations WHERE telegram_id = ? AND role = 'client' AND created_at > ?",
    )
    .bind(client.0)
    .bind(now - CANCELLATION_WINDOW_SECS)
    .fetch_one(pool)
    .await
}

/// The client's cancellations that still count (the last 30 days), for the
/// offers channel.
pub async fn recent_cancellations(pool: &SqlitePool, client: UserId, now: UnixTime) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM cancellations WHERE telegram_id = ? AND role = 'client' AND created_at > ?",
    )
    .bind(client.0)
    .bind(now - CANCELLATION_TTL_SECS)
    .fetch_one(pool)
    .await
}

pub async fn delete_old_cancellations(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM cancellations WHERE created_at <= ?")
        .bind(now - CANCELLATION_TTL_SECS)
        .execute(pool)
        .await?;
    Ok(())
}

/// Why someone was blocked, as stored (short codes, no free text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// 3 counted cancellations in 7 days.
    Cancellations,
    /// An admin decided it.
    Admin,
}

impl BlockReason {
    fn as_str(self) -> &'static str {
        match self {
            BlockReason::Cancellations => "cancellations",
            BlockReason::Admin => "admin",
        }
    }
}

/// Blocks `user` in `role` until `until` (`None`: permanently), replacing a
/// previous block.
pub async fn block(
    pool: &SqlitePool,
    user: UserId,
    role: Role,
    until: Option<UnixTime>,
    reason: BlockReason,
    by: UserId,
    now: UnixTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO blocked (telegram_id, role, reason, until, created_at, blocked_by) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (telegram_id, role) DO UPDATE SET
             reason = excluded.reason, until = excluded.until,
             created_at = excluded.created_at, blocked_by = excluded.blocked_by",
    )
    .bind(user.0)
    .bind(role.as_str())
    .bind(reason.as_str())
    .bind(until)
    .bind(now)
    .bind(by.0)
    .execute(pool)
    .await?;
    Ok(())
}

/// Lifts every block of the user. Returns whether there was any.
pub async fn unblock(pool: &SqlitePool, user: UserId) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM blocked WHERE telegram_id = ?").bind(user.0).execute(pool).await?;
    Ok(done.rows_affected() > 0)
}

/// Until when the user is blocked in the role: `Some(None)` permanently,
/// `Some(Some(t))` until `t`, `None` if not blocked.
pub async fn blocked_until(
    pool: &SqlitePool,
    user: UserId,
    role: Role,
    now: UnixTime,
) -> sqlx::Result<Option<Option<UnixTime>>> {
    let row: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT until FROM blocked WHERE telegram_id = ? AND role = ? AND (until IS NULL OR until > ?)",
    )
    .bind(user.0)
    .bind(role.as_str())
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(until,)| until))
}

pub async fn delete_expired_blocks(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM blocked WHERE until IS NOT NULL AND until <= ?").bind(now).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    const DAY: i64 = 86_400;

    #[tokio::test]
    async fn cancellations_count_within_7_days_and_expire_after_30() {
        let (pool, _dir) = test_pool().await;
        let u = UserId(5);
        assert_eq!(count_cancellation(&pool, u, 0).await.unwrap(), 1);
        assert_eq!(count_cancellation(&pool, u, DAY).await.unwrap(), 2);
        // On day 7 the first one (day 0) is out of the 7-day window.
        assert_eq!(count_cancellation(&pool, u, 7 * DAY).await.unwrap(), 2);
        // On day 31 only the one from day 7 is left.
        delete_old_cancellations(&pool, 31 * DAY).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cancellations").fetch_one(&pool).await.unwrap();
        assert_eq!(left, 1);
    }

    #[tokio::test]
    async fn temporary_and_permanent_blocks() {
        let (pool, _dir) = test_pool().await;
        let (u, admin) = (UserId(5), UserId(1));
        block(&pool, u, Role::Client, Some(DAY), BlockReason::Cancellations, admin, 0).await.unwrap();
        assert_eq!(blocked_until(&pool, u, Role::Client, 10).await.unwrap(), Some(Some(DAY)));
        assert_eq!(blocked_until(&pool, u, Role::Driver, 10).await.unwrap(), None);
        assert_eq!(blocked_until(&pool, u, Role::Client, DAY).await.unwrap(), None);

        block(&pool, u, Role::Driver, None, BlockReason::Admin, admin, 0).await.unwrap();
        assert_eq!(blocked_until(&pool, u, Role::Driver, 10 * DAY).await.unwrap(), Some(None));
        delete_expired_blocks(&pool, 10 * DAY).await.unwrap();
        assert!(unblock(&pool, u).await.unwrap());
        assert!(!unblock(&pool, u).await.unwrap());
    }
}
