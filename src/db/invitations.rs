use sqlx::SqlitePool;

use crate::domain::ids::{InviteCode, UserId};
use crate::domain::request::UnixTime;

/// Invitations expire 48 h after they are created.
pub const INVITE_TTL_SECS: i64 = 48 * 60 * 60;

pub async fn create(pool: &SqlitePool, code: &InviteCode, by: UserId, now: UnixTime) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO invitations (code, role, created_by, expires_at) VALUES (?, 'driver', ?, ?)")
        .bind(code.as_str())
        .bind(by.0)
        .bind(now + INVITE_TTL_SECS)
        .execute(pool)
        .await?;
    Ok(())
}

/// Who created the invitation, if it is still unused and not expired.
/// Does not consume it: the driver may abandon the form and open the link again.
pub async fn check(pool: &SqlitePool, code: &InviteCode, now: UnixTime) -> sqlx::Result<Option<UserId>> {
    let by: Option<i64> = sqlx::query_scalar(
        "SELECT created_by FROM invitations WHERE code = ? AND used_by IS NULL AND expires_at > ?",
    )
    .bind(code.as_str())
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(by.map(UserId))
}

/// Marks the invitation as used. Exactly one caller succeeds.
pub async fn redeem(pool: &SqlitePool, code: &InviteCode, user: UserId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE invitations SET used_by = ? WHERE code = ? AND used_by IS NULL AND expires_at > ?",
    )
    .bind(user.0)
    .bind(code.as_str())
    .bind(now)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Removes expired invitations, used or not.
pub async fn delete_expired(pool: &SqlitePool, now: UnixTime) -> sqlx::Result<u64> {
    let done = sqlx::query("DELETE FROM invitations WHERE expires_at <= ?").bind(now).execute(pool).await?;
    Ok(done.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn invitations_are_single_use_and_expire() {
        let (pool, _dir) = test_pool().await;
        let code = InviteCode::generate(&mut rand::rng());
        create(&pool, &code, UserId(1), 0).await.unwrap();

        assert_eq!(check(&pool, &code, 10).await.unwrap(), Some(UserId(1)));
        assert!(redeem(&pool, &code, UserId(7), 10).await.unwrap());
        assert!(!redeem(&pool, &code, UserId(8), 10).await.unwrap());
        assert_eq!(check(&pool, &code, 10).await.unwrap(), None);

        let late = InviteCode::generate(&mut rand::rng());
        create(&pool, &late, UserId(1), 0).await.unwrap();
        assert_eq!(check(&pool, &late, INVITE_TTL_SECS).await.unwrap(), None);
        assert_eq!(delete_expired(&pool, INVITE_TTL_SECS).await.unwrap(), 2);
    }
}
