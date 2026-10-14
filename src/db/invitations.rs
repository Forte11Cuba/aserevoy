use sqlx::SqlitePool;

use crate::domain::ids::{InviteCode, UserId};
use crate::domain::request::UnixTime;

/// Invitations expire 48 h after they are created.
pub const INVITE_TTL_SECS: i64 = 48 * 60 * 60;

/// What the invitation makes of whoever uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteRole {
    Driver,
    Admin,
}

impl InviteRole {
    fn as_str(self) -> &'static str {
        match self {
            InviteRole::Driver => "driver",
            InviteRole::Admin => "admin",
        }
    }
}

pub async fn create(
    pool: &SqlitePool,
    code: &InviteCode,
    role: InviteRole,
    by: UserId,
    now: UnixTime,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO invitations (code, role, created_by, expires_at) VALUES (?, ?, ?, ?)")
        .bind(code.as_str())
        .bind(role.as_str())
        .bind(by.0)
        .bind(now + INVITE_TTL_SECS)
        .execute(pool)
        .await?;
    Ok(())
}

/// An invitation for a contact the admin picked: only that person can use it.
pub async fn create_for(
    pool: &SqlitePool,
    code: &InviteCode,
    for_user: UserId,
    by: UserId,
    now: UnixTime,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO invitations (code, role, created_by, expires_at, for_user) VALUES (?, 'driver', ?, ?, ?)")
        .bind(code.as_str())
        .bind(by.0)
        .bind(now + INVITE_TTL_SECS)
        .bind(for_user.0)
        .execute(pool)
        .await?;
    Ok(())
}

/// A pending driver invitation for this user, if an admin picked them.
pub async fn pending_for(pool: &SqlitePool, user: UserId, now: UnixTime) -> sqlx::Result<Option<InviteCode>> {
    let code: Option<String> = sqlx::query_scalar(
        "SELECT code FROM invitations WHERE for_user = ? AND role = 'driver' AND used_by IS NULL AND expires_at > ?
         ORDER BY expires_at DESC LIMIT 1",
    )
    .bind(user.0)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(code.as_deref().and_then(InviteCode::parse))
}

/// Who created the invitation, if it is for `role`, still unused, not
/// expired and (for a contact invitation) for `user`. Does not consume it: the
/// driver may abandon the form and open the link again.
pub async fn check(
    pool: &SqlitePool,
    code: &InviteCode,
    role: InviteRole,
    user: UserId,
    now: UnixTime,
) -> sqlx::Result<Option<UserId>> {
    let by: Option<i64> = sqlx::query_scalar(
        "SELECT created_by FROM invitations WHERE code = ? AND role = ? AND used_by IS NULL AND expires_at > ?
         AND (for_user IS NULL OR for_user = ?)",
    )
    .bind(code.as_str())
    .bind(role.as_str())
    .bind(now)
    .bind(user.0)
    .fetch_optional(pool)
    .await?;
    Ok(by.map(UserId))
}

/// Marks the invitation as used. Exactly one caller succeeds.
pub async fn redeem(pool: &SqlitePool, code: &InviteCode, role: InviteRole, user: UserId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE invitations SET used_by = ? WHERE code = ? AND role = ? AND used_by IS NULL AND expires_at > ?
         AND (for_user IS NULL OR for_user = ?)",
    )
    .bind(user.0)
    .bind(code.as_str())
    .bind(role.as_str())
    .bind(now)
    .bind(user.0)
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
        create(&pool, &code, InviteRole::Driver, UserId(1), 0).await.unwrap();

        assert_eq!(check(&pool, &code, InviteRole::Driver, UserId(7), 10).await.unwrap(), Some(UserId(1)));
        assert!(redeem(&pool, &code, InviteRole::Driver, UserId(7), 10).await.unwrap());
        assert!(!redeem(&pool, &code, InviteRole::Driver, UserId(8), 10).await.unwrap());
        assert_eq!(check(&pool, &code, InviteRole::Driver, UserId(7), 10).await.unwrap(), None);

        let late = InviteCode::generate(&mut rand::rng());
        create(&pool, &late, InviteRole::Driver, UserId(1), 0).await.unwrap();
        assert_eq!(check(&pool, &late, InviteRole::Driver, UserId(7), INVITE_TTL_SECS).await.unwrap(), None);
        assert_eq!(delete_expired(&pool, INVITE_TTL_SECS).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn contact_invitations_only_work_for_their_person() {
        let (pool, _dir) = test_pool().await;
        let code = InviteCode::generate(&mut rand::rng());
        create_for(&pool, &code, UserId(7), UserId(1), 0).await.unwrap();
        assert_eq!(pending_for(&pool, UserId(7), 10).await.unwrap(), Some(code.clone()));
        assert_eq!(pending_for(&pool, UserId(8), 10).await.unwrap(), None);
        assert_eq!(check(&pool, &code, InviteRole::Driver, UserId(8), 10).await.unwrap(), None);
        assert!(!redeem(&pool, &code, InviteRole::Driver, UserId(8), 10).await.unwrap());
        assert!(redeem(&pool, &code, InviteRole::Driver, UserId(7), 10).await.unwrap());
        assert_eq!(pending_for(&pool, UserId(7), 10).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_driver_invitation_does_not_make_an_admin() {
        let (pool, _dir) = test_pool().await;
        let code = InviteCode::generate(&mut rand::rng());
        create(&pool, &code, InviteRole::Driver, UserId(1), 0).await.unwrap();
        assert_eq!(check(&pool, &code, InviteRole::Admin, UserId(7), 10).await.unwrap(), None);
        assert!(!redeem(&pool, &code, InviteRole::Admin, UserId(7), 10).await.unwrap());
    }
}
