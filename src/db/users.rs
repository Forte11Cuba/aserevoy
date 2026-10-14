//! Admins appointed by superadmins, and the language users chose.

use sqlx::SqlitePool;

use crate::domain::ids::UserId;
use crate::i18n::Lang;

pub async fn admins(pool: &SqlitePool) -> sqlx::Result<Vec<UserId>> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT telegram_id FROM admins").fetch_all(pool).await?;
    Ok(ids.into_iter().map(UserId).collect())
}

pub async fn is_admin(pool: &SqlitePool, user: UserId) -> sqlx::Result<bool> {
    let found: Option<i64> = sqlx::query_scalar("SELECT 1 FROM admins WHERE telegram_id = ?")
        .bind(user.0)
        .fetch_optional(pool)
        .await?;
    Ok(found.is_some())
}

/// Only set when the user changed it; otherwise Telegram's language is used.
pub async fn lang(pool: &SqlitePool, user: UserId) -> sqlx::Result<Option<Lang>> {
    let code: Option<String> = sqlx::query_scalar("SELECT lang FROM user_prefs WHERE telegram_id = ?")
        .bind(user.0)
        .fetch_optional(pool)
        .await?;
    Ok(code.as_deref().and_then(Lang::parse))
}

pub async fn set_lang(pool: &SqlitePool, user: UserId, lang: Lang) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO user_prefs (telegram_id, lang) VALUES (?, ?)
         ON CONFLICT (telegram_id) DO UPDATE SET lang = excluded.lang",
    )
    .bind(user.0)
    .bind(lang.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn language_preference_is_stored_and_changed() {
        let (pool, _dir) = test_pool().await;
        assert_eq!(lang(&pool, UserId(5)).await.unwrap(), None);
        set_lang(&pool, UserId(5), Lang::En).await.unwrap();
        set_lang(&pool, UserId(5), Lang::Es).await.unwrap();
        assert_eq!(lang(&pool, UserId(5)).await.unwrap(), Some(Lang::Es));
    }

    #[tokio::test]
    async fn admins_come_from_the_table() {
        let (pool, _dir) = test_pool().await;
        sqlx::query("INSERT INTO admins (telegram_id, appointed_by, created_at) VALUES (9, 1, 0)")
            .execute(&pool)
            .await
            .unwrap();
        assert!(is_admin(&pool, UserId(9)).await.unwrap());
        assert!(!is_admin(&pool, UserId(8)).await.unwrap());
        assert_eq!(admins(&pool).await.unwrap(), vec![UserId(9)]);
    }
}
