use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::crypto::PhoneCipher;
use crate::domain::driver::{Driver, DriverStatus, PaymentMethods, Profile};
use crate::domain::ids::UserId;
use crate::domain::request::UnixTime;

/// Creates the profile as `pending`, or updates the editable fields of an
/// existing one without touching its status (edits need no new approval).
pub async fn save_profile(
    pool: &SqlitePool,
    cipher: &PhoneCipher,
    id: UserId,
    p: &Profile,
    added_by: Option<UserId>,
    now: UnixTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO drivers (telegram_id, alias, phone_enc, plate, bike_model, bike_color,
             photo_file_id, bike_photo_file_id, payment_methods, status, created_at, added_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)
         ON CONFLICT (telegram_id) DO UPDATE SET
             alias = excluded.alias, phone_enc = excluded.phone_enc, plate = excluded.plate,
             bike_model = excluded.bike_model, bike_color = excluded.bike_color,
             photo_file_id = excluded.photo_file_id,
             bike_photo_file_id = excluded.bike_photo_file_id,
             payment_methods = excluded.payment_methods",
    )
    .bind(id.0)
    .bind(&p.alias)
    .bind(cipher.encrypt(&p.phone))
    .bind(&p.plate)
    .bind(&p.bike_model)
    .bind(&p.bike_color)
    .bind(&p.photo_file_id)
    .bind(&p.bike_photo_file_id)
    .bind(p.payment_methods.as_str())
    .bind(now)
    .bind(added_by.map(|u| u.0))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, cipher: &PhoneCipher, id: UserId) -> anyhow::Result<Option<Driver>> {
    let row = sqlx::query("SELECT * FROM drivers WHERE telegram_id = ?")
        .bind(id.0)
        .fetch_optional(pool)
        .await?;
    row.map(|row| from_row(&row, cipher)).transpose()
}

pub async fn status(pool: &SqlitePool, id: UserId) -> sqlx::Result<Option<DriverStatus>> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM drivers WHERE telegram_id = ?")
        .bind(id.0)
        .fetch_optional(pool)
        .await?;
    Ok(status.as_deref().and_then(DriverStatus::parse))
}

/// `pending` → `active`. Returns false if someone else already decided.
pub async fn approve(pool: &SqlitePool, id: UserId, by: UserId) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE drivers SET status = 'active', approved_by = ? WHERE telegram_id = ? AND status = 'pending'",
    )
    .bind(by.0)
    .bind(id.0)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Deletes a pending application. Returns false if it was already decided.
pub async fn reject(pool: &SqlitePool, id: UserId) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM drivers WHERE telegram_id = ? AND status = 'pending'")
        .bind(id.0)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// `active` ↔ `blocked`, when a driver is blocked or unblocked.
pub async fn set_status(pool: &SqlitePool, id: UserId, status: DriverStatus) -> sqlx::Result<bool> {
    let status = match status {
        DriverStatus::Pending => "pending",
        DriverStatus::Active => "active",
        DriverStatus::Blocked => "blocked",
    };
    let done = sqlx::query("UPDATE drivers SET status = ? WHERE telegram_id = ? AND status != 'pending'")
        .bind(status)
        .bind(id.0)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Removes a driver for good (their reputation as a driver stays aggregated).
pub async fn remove(pool: &SqlitePool, id: UserId) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM drivers WHERE telegram_id = ?").bind(id.0).execute(pool).await?;
    Ok(done.rows_affected() == 1)
}

/// A line of the drivers list in the admin panel (no phone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: UserId,
    pub alias: String,
    pub status: DriverStatus,
}

/// Drivers by alias, optionally of one status, one page at a time; and how
/// many there are in total.
pub async fn list(
    pool: &SqlitePool,
    status: Option<DriverStatus>,
    offset: i64,
    limit: i64,
) -> sqlx::Result<(Vec<Listed>, i64)> {
    let status = status.map(|s| match s {
        DriverStatus::Pending => "pending",
        DriverStatus::Active => "active",
        DriverStatus::Blocked => "blocked",
    });
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT telegram_id, alias, status FROM drivers WHERE (?1 IS NULL OR status = ?1)
         ORDER BY alias COLLATE NOCASE, telegram_id LIMIT ?2 OFFSET ?3",
    )
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drivers WHERE (?1 IS NULL OR status = ?1)")
        .bind(status)
        .fetch_one(pool)
        .await?;
    let listed = rows
        .into_iter()
        .filter_map(|(id, alias, status)| {
            DriverStatus::parse(&status).map(|status| Listed { id: UserId(id), alias, status })
        })
        .collect();
    Ok((listed, total))
}

fn from_row(row: &SqliteRow, cipher: &PhoneCipher) -> anyhow::Result<Driver> {
    let decode_err = |what: &str| anyhow::anyhow!("invalid driver {what}");
    let phone_enc: Vec<u8> = row.try_get("phone_enc")?;
    let methods: String = row.try_get("payment_methods")?;
    let status: String = row.try_get("status")?;
    Ok(Driver {
        id: UserId(row.try_get("telegram_id")?),
        profile: Profile {
            alias: row.try_get("alias")?,
            phone: cipher.decrypt(&phone_enc)?,
            plate: row.try_get("plate")?,
            bike_model: row.try_get("bike_model")?,
            bike_color: row.try_get("bike_color")?,
            photo_file_id: row.try_get("photo_file_id")?,
            bike_photo_file_id: row.try_get("bike_photo_file_id")?,
            payment_methods: PaymentMethods::parse(&methods).ok_or_else(|| decode_err("payment methods"))?,
        },
        status: DriverStatus::parse(&status).ok_or_else(|| decode_err("status"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    fn cipher() -> PhoneCipher {
        PhoneCipher::from_hex(&"ab".repeat(32)).unwrap()
    }

    fn profile(alias: &str) -> Profile {
        Profile {
            alias: alias.into(),
            phone: "+5351234567".into(),
            plate: "P123456".into(),
            bike_model: "Suzuki AX 100".into(),
            bike_color: "Roja".into(),
            photo_file_id: "photo".into(),
            bike_photo_file_id: None,
            payment_methods: PaymentMethods::BOTH,
        }
    }

    #[tokio::test]
    async fn saves_pending_then_edits_keep_the_status() {
        let (pool, _dir) = test_pool().await;
        let (c, id, admin) = (cipher(), UserId(7), UserId(1));
        save_profile(&pool, &c, id, &profile("Pepe"), Some(admin), 10).await.unwrap();
        assert_eq!(status(&pool, id).await.unwrap(), Some(DriverStatus::Pending));

        assert!(approve(&pool, id, admin).await.unwrap());
        assert!(!approve(&pool, id, admin).await.unwrap());
        save_profile(&pool, &c, id, &profile("Pepito"), None, 20).await.unwrap();

        let driver = get(&pool, &c, id).await.unwrap().unwrap();
        assert_eq!(driver.status, DriverStatus::Active);
        assert_eq!(driver.profile, profile("Pepito"));
    }

    #[tokio::test]
    async fn phones_are_not_stored_in_clear() {
        let (pool, _dir) = test_pool().await;
        save_profile(&pool, &cipher(), UserId(7), &profile("Pepe"), None, 10).await.unwrap();
        let blob: Vec<u8> = sqlx::query_scalar("SELECT phone_enc FROM drivers").fetch_one(&pool).await.unwrap();
        assert!(!blob.windows(8).any(|w| w == b"51234567"));
    }

    #[tokio::test]
    async fn drivers_are_listed_by_status_and_page() {
        let (pool, _dir) = test_pool().await;
        let c = cipher();
        for (id, alias) in [(1, "Beto"), (2, "ana"), (3, "Carlos")] {
            save_profile(&pool, &c, UserId(id), &profile(alias), None, 10).await.unwrap();
        }
        approve(&pool, UserId(3), UserId(9)).await.unwrap();

        let (all, total) = list(&pool, None, 0, 10).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(all.iter().map(|d| d.alias.as_str()).collect::<Vec<_>>(), ["ana", "Beto", "Carlos"]);
        let (active, total) = list(&pool, Some(DriverStatus::Active), 0, 10).await.unwrap();
        assert_eq!((active.len(), total, active[0].alias.as_str()), (1, 1, "Carlos"));
        let (page, total) = list(&pool, None, 2, 2).await.unwrap();
        assert_eq!((page.len(), total), (1, 3));
    }

    #[tokio::test]
    async fn only_pending_applications_can_be_rejected() {
        let (pool, _dir) = test_pool().await;
        let c = cipher();
        save_profile(&pool, &c, UserId(7), &profile("Pepe"), None, 10).await.unwrap();
        save_profile(&pool, &c, UserId(8), &profile("Juan"), None, 10).await.unwrap();
        approve(&pool, UserId(8), UserId(1)).await.unwrap();
        assert!(reject(&pool, UserId(7)).await.unwrap());
        assert!(!reject(&pool, UserId(8)).await.unwrap());
        assert_eq!(status(&pool, UserId(7)).await.unwrap(), None);
    }
}
