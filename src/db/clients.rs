//! A client's optional details, for the driver of their trip.

use sqlx::SqlitePool;

use crate::crypto::PhoneCipher;
use crate::domain::ids::UserId;
use crate::domain::request::UnixTime;

/// What a client chose to share; every part is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientProfile {
    pub name: Option<String>,
    pub phone: Option<String>,
    pub photo_file_id: Option<String>,
}

impl ClientProfile {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.phone.is_none() && self.photo_file_id.is_none()
    }
}

/// `name_enc, phone_enc, photo_file_id`.
type Row = (Option<Vec<u8>>, Option<Vec<u8>>, Option<String>);

pub async fn get(pool: &SqlitePool, cipher: &PhoneCipher, id: UserId) -> anyhow::Result<Option<ClientProfile>> {
    let row: Option<Row> =
        sqlx::query_as("SELECT name_enc, phone_enc, photo_file_id FROM client_profiles WHERE telegram_id = ?")
            .bind(id.0)
            .fetch_optional(pool)
            .await?;
    let Some((name, phone, photo_file_id)) = row else {
        return Ok(None);
    };
    let decrypt = |blob: Option<Vec<u8>>| blob.map(|b| cipher.decrypt(&b)).transpose();
    Ok(Some(ClientProfile { name: decrypt(name)?, phone: decrypt(phone)?, photo_file_id }))
}

/// Saves the whole profile (the caller changed one part of what `get` gave).
pub async fn save(
    pool: &SqlitePool,
    cipher: &PhoneCipher,
    id: UserId,
    p: &ClientProfile,
    now: UnixTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO client_profiles (telegram_id, name_enc, phone_enc, photo_file_id, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (telegram_id) DO UPDATE SET name_enc = excluded.name_enc,
             phone_enc = excluded.phone_enc, photo_file_id = excluded.photo_file_id,
             updated_at = excluded.updated_at",
    )
    .bind(id.0)
    .bind(p.name.as_deref().map(|n| cipher.encrypt(n)))
    .bind(p.phone.as_deref().map(|n| cipher.encrypt(n)))
    .bind(&p.photo_file_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// "🗑 Delete my details".
pub async fn delete(pool: &SqlitePool, id: UserId) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM client_profiles WHERE telegram_id = ?").bind(id.0).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn details_are_saved_encrypted_and_deleted() {
        let (pool, _dir) = test_pool().await;
        let c = PhoneCipher::from_hex(&"ab".repeat(32)).unwrap();
        let id = UserId(9);
        assert_eq!(get(&pool, &c, id).await.unwrap(), None);
        let p = ClientProfile { name: Some("Ana".into()), phone: Some("+5351234567".into()), photo_file_id: None };
        save(&pool, &c, id, &p, 10).await.unwrap();
        assert_eq!(get(&pool, &c, id).await.unwrap(), Some(p));
        let stored: Option<Vec<u8>> = sqlx::query_scalar("SELECT phone_enc FROM client_profiles").fetch_one(&pool).await.unwrap();
        assert!(!String::from_utf8_lossy(&stored.unwrap()).contains("5351234567"));
        delete(&pool, id).await.unwrap();
        assert_eq!(get(&pool, &c, id).await.unwrap(), None);
    }
}
