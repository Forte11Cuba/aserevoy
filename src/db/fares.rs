use sqlx::SqlitePool;

use crate::domain::fare::{FareKey, FareTable, Fares};
use crate::domain::vehicle::VehicleKind;

const BASE: &str = "base";
const PER_KM: &str = "per_km";
const MIN_PERCENT: &str = "min_percent";

/// The moto keeps the keys from before there were other kinds.
fn key_of(kind: VehicleKind, key: FareKey) -> &'static str {
    use FareKey::*;
    use VehicleKind::*;
    match (kind, key) {
        (Moto, Base) => BASE,
        (Moto, PerKm) => PER_KM,
        (Moto, MinPercent) => MIN_PERCENT,
        (Car, Base) => "car.base",
        (Car, PerKm) => "car.per_km",
        (Car, MinPercent) => "car.min_percent",
        (Tricycle, Base) => "tricycle.base",
        (Tricycle, PerKm) => "tricycle.per_km",
        (Tricycle, MinPercent) => "tricycle.min_percent",
    }
}

/// Copies the `.env` defaults into the table the first time; later changes are
/// made from the bot and survive restarts.
pub async fn seed(pool: &SqlitePool, defaults: &Fares) -> sqlx::Result<()> {
    let rows = [
        (BASE, defaults.base),
        (PER_KM, defaults.per_km),
        (MIN_PERCENT, i64::from(defaults.min_percent)),
    ];
    for (key, value) in rows {
        sqlx::query("INSERT OR IGNORE INTO fares (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
    }
    // Car and tricycle start as copies of the moto, as it is now.
    for kind in [VehicleKind::Car, VehicleKind::Tricycle] {
        for key in [FareKey::Base, FareKey::PerKm, FareKey::MinPercent] {
            sqlx::query("INSERT OR IGNORE INTO fares (key, value) SELECT ?, value FROM fares WHERE key = ?")
                .bind(key_of(kind, key))
                .bind(key_of(VehicleKind::Moto, key))
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// A superadmin's change to one kind's fare; it survives restarts.
pub async fn set(pool: &SqlitePool, kind: VehicleKind, key: FareKey, value: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE fares SET value = ? WHERE key = ?").bind(value).bind(key_of(kind, key)).execute(pool).await?;
    Ok(())
}

pub async fn load_table(pool: &SqlitePool) -> sqlx::Result<FareTable> {
    Ok(FareTable {
        moto: load_kind(pool, VehicleKind::Moto).await?,
        car: load_kind(pool, VehicleKind::Car).await?,
        tricycle: load_kind(pool, VehicleKind::Tricycle).await?,
    })
}

async fn load_kind(pool: &SqlitePool, kind: VehicleKind) -> sqlx::Result<Fares> {
    let get = |key: FareKey| async move {
        sqlx::query_scalar::<_, i64>("SELECT value FROM fares WHERE key = ?")
            .bind(key_of(kind, key))
            .fetch_one(pool)
            .await
    };
    Ok(Fares {
        base: get(FareKey::Base).await?,
        per_km: get(FareKey::PerKm).await?,
        min_percent: get(FareKey::MinPercent).await?.clamp(0, 100) as u8,
    })
}

/// The moto's fares.
pub async fn load(pool: &SqlitePool) -> sqlx::Result<Fares> {
    load_kind(pool, VehicleKind::Moto).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_does_not_overwrite_existing_values() {
        let (pool, _dir) = crate::db::test_pool().await;
        let first = Fares { base: 100, per_km: 150, min_percent: 80 };
        seed(&pool, &first).await.unwrap();
        seed(&pool, &Fares { base: 1, per_km: 1, min_percent: 1 }).await.unwrap();
        assert_eq!(load(&pool).await.unwrap(), first);
    }

    #[tokio::test]
    async fn car_and_tricycle_start_as_the_moto_and_change_apart() {
        let (pool, _dir) = crate::db::test_pool().await;
        let moto = Fares { base: 100, per_km: 150, min_percent: 80 };
        seed(&pool, &moto).await.unwrap();
        assert_eq!(load_table(&pool).await.unwrap(), FareTable::like_moto(moto.clone()));

        set(&pool, VehicleKind::Car, FareKey::PerKm, 250).await.unwrap();
        // A later start does not copy the moto over them again.
        seed(&pool, &moto).await.unwrap();
        let table = load_table(&pool).await.unwrap();
        assert_eq!(table.car, Fares { base: 100, per_km: 250, min_percent: 80 });
        assert_eq!((table.moto, table.tricycle), (moto.clone(), moto));
    }

    #[tokio::test]
    async fn set_changes_one_fare() {
        let (pool, _dir) = crate::db::test_pool().await;
        seed(&pool, &Fares { base: 100, per_km: 150, min_percent: 80 }).await.unwrap();
        set(&pool, VehicleKind::Moto, FareKey::PerKm, 180).await.unwrap();
        assert_eq!(load(&pool).await.unwrap(), Fares { base: 100, per_km: 180, min_percent: 80 });
    }
}
