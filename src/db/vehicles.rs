//! Drivers' vehicles (decisions 120 to 129). Switching or removing the active
//! vehicle is a conditional `UPDATE`/`DELETE`, so it cannot slip in between a
//! driver reserving a request and the client confirming them.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::domain::ids::UserId;
use crate::domain::request::UnixTime;
use crate::domain::vehicle::{NewVehicle, Vehicle, VehicleId, VehicleKind, VehicleStatus, MAX_PER_DRIVER};

/// What can change on a vehicle after it is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Plate,
    Model,
    Color,
    Photo,
}

/// How a change was saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// At once: the model or color, or anything on a vehicle not approved yet.
    Applied,
    /// The plate or photo of an approved vehicle, waiting for an admin.
    Waiting,
}

/// Adds a pending vehicle, unless the driver already has the maximum.
pub async fn add(pool: &SqlitePool, driver: UserId, v: &NewVehicle, now: UnixTime) -> sqlx::Result<Option<VehicleId>> {
    let done = sqlx::query(
        "INSERT INTO vehicles (driver_id, kind, plate, model, color, photo_file_id, status, created_at)
         SELECT ?, ?, ?, ?, ?, ?, 'pending', ?
         WHERE (SELECT COUNT(*) FROM vehicles WHERE driver_id = ?) < ?",
    )
    .bind(driver.0)
    .bind(v.kind.code())
    .bind(&v.plate)
    .bind(&v.model)
    .bind(&v.color)
    .bind(&v.photo_file_id)
    .bind(now)
    .bind(driver.0)
    .bind(MAX_PER_DRIVER as i64)
    .execute(pool)
    .await?;
    Ok((done.rows_affected() == 1).then(|| done.last_insert_rowid()))
}

pub async fn get(pool: &SqlitePool, id: VehicleId) -> sqlx::Result<Option<Vehicle>> {
    let row = sqlx::query("SELECT * FROM vehicles WHERE id = ?").bind(id).fetch_optional(pool).await?;
    row.map(|row| from_row(&row)).transpose()
}

/// The driver's vehicles, oldest first.
pub async fn for_driver(pool: &SqlitePool, driver: UserId) -> sqlx::Result<Vec<Vehicle>> {
    let rows = sqlx::query("SELECT * FROM vehicles WHERE driver_id = ? ORDER BY id")
        .bind(driver.0)
        .fetch_all(pool)
        .await?;
    rows.iter().map(from_row).collect()
}

/// The vehicle the driver works with, approved or not.
pub async fn active(pool: &SqlitePool, driver: UserId) -> sqlx::Result<Option<Vehicle>> {
    let row = sqlx::query(
        "SELECT vehicles.* FROM vehicles JOIN drivers ON drivers.active_vehicle_id = vehicles.id
         WHERE drivers.telegram_id = ?",
    )
    .bind(driver.0)
    .fetch_optional(pool)
    .await?;
    row.map(|row| from_row(&row)).transpose()
}

/// Whether the driver has a counteroffer that has not expired.
pub async fn bidding(pool: &SqlitePool, driver: UserId, now: UnixTime) -> sqlx::Result<bool> {
    let found: Option<i64> = sqlx::query_scalar("SELECT 1 FROM counteroffers WHERE driver_id = ? AND expires_at > ?")
        .bind(driver.0)
        .bind(now)
        .fetch_optional(pool)
        .await?;
    Ok(found.is_some())
}

/// Makes an approved vehicle of the driver's their active one, unless they
/// hold a request or have a live counteroffer. Returns false if not.
pub async fn set_active(pool: &SqlitePool, driver: UserId, id: VehicleId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE drivers SET active_vehicle_id = ?
         WHERE telegram_id = ?
           AND EXISTS (SELECT 1 FROM vehicles WHERE id = ? AND driver_id = ? AND status = 'approved')
           AND NOT EXISTS (SELECT 1 FROM requests WHERE driver_id = ? AND state IN ('confirming', 'assigned'))
           AND NOT EXISTS (SELECT 1 FROM counteroffers WHERE driver_id = ? AND expires_at > ?)",
    )
    .bind(id)
    .bind(driver.0)
    .bind(id)
    .bind(driver.0)
    .bind(driver.0)
    .bind(driver.0)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Removes one of the driver's vehicles; the active one only while they hold
/// nothing. They stay a driver, without an active vehicle until they pick one.
pub async fn remove(pool: &SqlitePool, driver: UserId, id: VehicleId, now: UnixTime) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "DELETE FROM vehicles
         WHERE id = ? AND driver_id = ?
           AND (id IS NOT (SELECT active_vehicle_id FROM drivers WHERE telegram_id = ?)
                OR (NOT EXISTS (SELECT 1 FROM requests WHERE driver_id = ? AND state IN ('confirming', 'assigned'))
                    AND NOT EXISTS (SELECT 1 FROM counteroffers WHERE driver_id = ? AND expires_at > ?)))",
    )
    .bind(id)
    .bind(driver.0)
    .bind(driver.0)
    .bind(driver.0)
    .bind(driver.0)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The driver changes one field of their vehicle. A plate or photo of an
/// approved vehicle waits for an admin (decision 122); anything else applies
/// at once. `None` if the vehicle is not theirs.
pub async fn change(
    pool: &SqlitePool,
    driver: UserId,
    id: VehicleId,
    field: Field,
    value: &str,
) -> sqlx::Result<Option<Saved>> {
    let at_once = match field {
        Field::Plate => "UPDATE vehicles SET plate = ? WHERE id = ? AND driver_id = ? AND status = 'pending'",
        Field::Photo => "UPDATE vehicles SET photo_file_id = ? WHERE id = ? AND driver_id = ? AND status = 'pending'",
        Field::Model => "UPDATE vehicles SET model = ? WHERE id = ? AND driver_id = ?",
        Field::Color => "UPDATE vehicles SET color = ? WHERE id = ? AND driver_id = ?",
    };
    if sqlx::query(at_once).bind(value).bind(id).bind(driver.0).execute(pool).await?.rows_affected() == 1 {
        return Ok(Some(Saved::Applied));
    }
    let waiting = match field {
        Field::Plate => "UPDATE vehicles SET pending_plate = ? WHERE id = ? AND driver_id = ? AND status = 'approved'",
        Field::Photo => {
            "UPDATE vehicles SET pending_photo_file_id = ? WHERE id = ? AND driver_id = ? AND status = 'approved'"
        }
        Field::Model | Field::Color => return Ok(None),
    };
    let done = sqlx::query(waiting).bind(value).bind(id).bind(driver.0).execute(pool).await?;
    Ok((done.rows_affected() == 1).then_some(Saved::Waiting))
}

/// An admin decides on a waiting plate or photo; approved, it replaces the
/// old one. Returns false if there was none (already decided).
pub async fn decide_change(pool: &SqlitePool, id: VehicleId, field: Field, approve: bool) -> sqlx::Result<bool> {
    let query = match (field, approve) {
        (Field::Plate, true) => {
            "UPDATE vehicles SET plate = pending_plate, pending_plate = NULL WHERE id = ? AND pending_plate IS NOT NULL"
        }
        (Field::Plate, false) => "UPDATE vehicles SET pending_plate = NULL WHERE id = ? AND pending_plate IS NOT NULL",
        (Field::Photo, true) => {
            "UPDATE vehicles SET photo_file_id = pending_photo_file_id, pending_photo_file_id = NULL
             WHERE id = ? AND pending_photo_file_id IS NOT NULL"
        }
        (Field::Photo, false) => {
            "UPDATE vehicles SET pending_photo_file_id = NULL WHERE id = ? AND pending_photo_file_id IS NOT NULL"
        }
        (Field::Model | Field::Color, _) => return Ok(false),
    };
    let done = sqlx::query(query).bind(id).execute(pool).await?;
    Ok(done.rows_affected() == 1)
}

/// An admin approves a new vehicle of an approved driver; it becomes their
/// active one if they have none. Returns false if someone already decided.
pub async fn approve(pool: &SqlitePool, id: VehicleId, by: UserId) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let done = sqlx::query("UPDATE vehicles SET status = 'approved', approved_by = ? WHERE id = ? AND status = 'pending'")
        .bind(by.0)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if done.rows_affected() != 1 {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE drivers SET active_vehicle_id = ?
         WHERE telegram_id = (SELECT driver_id FROM vehicles WHERE id = ?) AND active_vehicle_id IS NULL",
    )
    .bind(id)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// An admin rejects a new vehicle: it is deleted (decision 129).
pub async fn reject(pool: &SqlitePool, id: VehicleId) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM vehicles WHERE id = ? AND status = 'pending'").bind(id).execute(pool).await?;
    Ok(done.rows_affected() == 1)
}

/// The kinds the client can pick from (decision 127): those of an approved
/// vehicle of an active driver, whether it is their active one or not.
pub async fn registered_kinds(pool: &SqlitePool) -> sqlx::Result<Vec<VehicleKind>> {
    let codes: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT vehicles.kind FROM vehicles JOIN drivers ON drivers.telegram_id = vehicles.driver_id
         WHERE drivers.status = 'active' AND vehicles.status = 'approved'",
    )
    .fetch_all(pool)
    .await?;
    Ok(codes.iter().filter_map(|c| VehicleKind::parse(c)).collect())
}

fn from_row(row: &SqliteRow) -> sqlx::Result<Vehicle> {
    let decode = |column: &str, value: String| sqlx::Error::ColumnDecode {
        index: column.into(),
        source: format!("unknown value {value:?}").into(),
    };
    let kind: String = row.try_get("kind")?;
    let status: String = row.try_get("status")?;
    Ok(Vehicle {
        id: row.try_get("id")?,
        driver: UserId(row.try_get("driver_id")?),
        kind: VehicleKind::parse(&kind).ok_or_else(|| decode("kind", kind.clone()))?,
        plate: row.try_get("plate")?,
        model: row.try_get("model")?,
        color: row.try_get("color")?,
        photo_file_id: row.try_get("photo_file_id")?,
        status: VehicleStatus::parse(&status).ok_or_else(|| decode("status", status.clone()))?,
        pending_plate: row.try_get("pending_plate")?,
        pending_photo_file_id: row.try_get("pending_photo_file_id")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    const DRIVER: UserId = UserId(7);
    const ADMIN: UserId = UserId(1);

    async fn driver(pool: &SqlitePool, id: UserId, status: &str) {
        sqlx::query("INSERT INTO drivers (telegram_id, alias, status, created_at) VALUES (?, 'Pepe', ?, 0)")
            .bind(id.0)
            .bind(status)
            .execute(pool)
            .await
            .unwrap();
    }

    fn new(kind: VehicleKind) -> NewVehicle {
        NewVehicle { kind, plate: "P123456".into(), model: "Lada".into(), color: "rojo".into(), photo_file_id: None }
    }

    async fn approved(pool: &SqlitePool, kind: VehicleKind) -> VehicleId {
        let id = add(pool, DRIVER, &new(kind), 10).await.unwrap().unwrap();
        assert!(approve(pool, id, ADMIN).await.unwrap());
        id
    }

    async fn hold_request(pool: &SqlitePool, state: &str) {
        sqlx::query(
            "INSERT INTO requests (id, client_id, pickup_lat, pickup_lon, dropoff_lat, dropoff_lon, km, price,
                 payment, state, driver_id, created_at, expires_at)
             VALUES ('ABC123', 99, 0, 0, 0, 0, 1, 500, 'cash', ?, ?, 0, 1000)",
        )
        .bind(state)
        .bind(DRIVER.0)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn up_to_five_vehicles() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        for _ in 0..MAX_PER_DRIVER {
            assert!(add(&pool, DRIVER, &new(VehicleKind::Moto), 10).await.unwrap().is_some());
        }
        assert_eq!(add(&pool, DRIVER, &new(VehicleKind::Car), 10).await.unwrap(), None);
        assert_eq!(for_driver(&pool, DRIVER).await.unwrap().len(), MAX_PER_DRIVER);
    }

    #[tokio::test]
    async fn approving_decides_once_and_makes_the_first_one_active() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        let moto = add(&pool, DRIVER, &new(VehicleKind::Moto), 10).await.unwrap().unwrap();
        assert_eq!(active(&pool, DRIVER).await.unwrap(), None);
        assert!(approve(&pool, moto, ADMIN).await.unwrap());
        assert!(!approve(&pool, moto, ADMIN).await.unwrap());
        assert!(!reject(&pool, moto).await.unwrap());
        assert_eq!(active(&pool, DRIVER).await.unwrap().map(|v| v.id), Some(moto));

        // A second one does not replace the active one.
        let car = approved(&pool, VehicleKind::Car).await;
        assert_eq!(active(&pool, DRIVER).await.unwrap().map(|v| v.id), Some(moto));
        assert_eq!(get(&pool, car).await.unwrap().unwrap().status, VehicleStatus::Approved);
    }

    #[tokio::test]
    async fn a_rejected_vehicle_is_deleted() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        let id = add(&pool, DRIVER, &new(VehicleKind::Car), 10).await.unwrap().unwrap();
        assert!(reject(&pool, id).await.unwrap());
        assert_eq!(get(&pool, id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn switching_needs_an_approved_vehicle_and_a_free_driver() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        let moto = approved(&pool, VehicleKind::Moto).await;
        let car = approved(&pool, VehicleKind::Car).await;
        let pending = add(&pool, DRIVER, &new(VehicleKind::Tricycle), 10).await.unwrap().unwrap();

        assert!(!set_active(&pool, DRIVER, pending, 10).await.unwrap());
        assert!(set_active(&pool, DRIVER, car, 10).await.unwrap());

        // A live counteroffer locks it; an expired one does not.
        hold_request(&pool, "open").await;
        sqlx::query("INSERT INTO counteroffers (request_id, driver_id, amount, expires_at) VALUES ('ABC123', ?, 600, 100)")
            .bind(DRIVER.0)
            .execute(&pool)
            .await
            .unwrap();
        assert!(bidding(&pool, DRIVER, 50).await.unwrap());
        assert!(!set_active(&pool, DRIVER, moto, 50).await.unwrap());
        assert!(set_active(&pool, DRIVER, moto, 150).await.unwrap());

        // So does a request waiting for confirmation.
        sqlx::query("UPDATE requests SET state = 'confirming'").execute(&pool).await.unwrap();
        assert!(!set_active(&pool, DRIVER, car, 150).await.unwrap());
        assert_eq!(active(&pool, DRIVER).await.unwrap().map(|v| v.id), Some(moto));
    }

    #[tokio::test]
    async fn a_vehicle_is_never_switched_to_someone_else() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        driver(&pool, UserId(8), "active").await;
        let car = approved(&pool, VehicleKind::Car).await;
        assert!(!set_active(&pool, UserId(8), car, 10).await.unwrap());
        assert!(!remove(&pool, UserId(8), car, 10).await.unwrap());
        assert_eq!(change(&pool, UserId(8), car, Field::Model, "Moskvich").await.unwrap(), None);
    }

    #[tokio::test]
    async fn the_active_vehicle_is_removed_only_when_free() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        let moto = approved(&pool, VehicleKind::Moto).await;
        let car = approved(&pool, VehicleKind::Car).await;
        hold_request(&pool, "assigned").await;
        assert!(!remove(&pool, DRIVER, moto, 10).await.unwrap());
        assert!(remove(&pool, DRIVER, car, 10).await.unwrap());

        sqlx::query("DELETE FROM requests").execute(&pool).await.unwrap();
        assert!(remove(&pool, DRIVER, moto, 10).await.unwrap());
        // Still a driver, without an active vehicle.
        assert_eq!(active(&pool, DRIVER).await.unwrap(), None);
        let status: String =
            sqlx::query_scalar("SELECT status FROM drivers WHERE telegram_id = ?").bind(DRIVER.0).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "active");
    }

    #[tokio::test]
    async fn plate_and_photo_of_an_approved_vehicle_wait_for_an_admin() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        let car = approved(&pool, VehicleKind::Car).await;

        assert_eq!(change(&pool, DRIVER, car, Field::Color, "azul").await.unwrap(), Some(Saved::Applied));
        assert_eq!(change(&pool, DRIVER, car, Field::Plate, "P999999").await.unwrap(), Some(Saved::Waiting));
        assert_eq!(change(&pool, DRIVER, car, Field::Photo, "photo2").await.unwrap(), Some(Saved::Waiting));
        let v = get(&pool, car).await.unwrap().unwrap();
        assert_eq!((v.color.as_str(), v.plate.as_str()), ("azul", "P123456"));
        assert_eq!(v.pending_plate.as_deref(), Some("P999999"));

        assert!(decide_change(&pool, car, Field::Plate, true).await.unwrap());
        assert!(!decide_change(&pool, car, Field::Plate, false).await.unwrap());
        assert!(decide_change(&pool, car, Field::Photo, false).await.unwrap());
        let v = get(&pool, car).await.unwrap().unwrap();
        assert_eq!((v.plate.as_str(), v.photo_file_id.as_deref()), ("P999999", None));
        assert!(!v.has_pending_change());
    }

    #[tokio::test]
    async fn a_vehicle_not_approved_yet_changes_at_once() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "pending").await;
        let id = add(&pool, DRIVER, &new(VehicleKind::Moto), 10).await.unwrap().unwrap();
        assert_eq!(change(&pool, DRIVER, id, Field::Plate, "P222222").await.unwrap(), Some(Saved::Applied));
        assert_eq!(get(&pool, id).await.unwrap().unwrap().plate, "P222222");
    }

    #[tokio::test]
    async fn the_client_sees_kinds_of_active_drivers_approved_vehicles() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        driver(&pool, UserId(8), "blocked").await;
        approved(&pool, VehicleKind::Moto).await;
        // Not the active vehicle, but registered and approved: it counts.
        approved(&pool, VehicleKind::Car).await;
        add(&pool, DRIVER, &new(VehicleKind::Tricycle), 10).await.unwrap();
        let blocked = add(&pool, UserId(8), &new(VehicleKind::Tricycle), 10).await.unwrap().unwrap();
        assert!(approve(&pool, blocked, ADMIN).await.unwrap());

        let mut kinds = registered_kinds(&pool).await.unwrap();
        kinds.sort_by_key(|k| k.code());
        assert_eq!(kinds, vec![VehicleKind::Car, VehicleKind::Moto]);
    }

    #[tokio::test]
    async fn removing_a_driver_removes_their_vehicles() {
        let (pool, _dir) = test_pool().await;
        driver(&pool, DRIVER, "active").await;
        let moto = approved(&pool, VehicleKind::Moto).await;
        sqlx::query("DELETE FROM drivers WHERE telegram_id = ?").bind(DRIVER.0).execute(&pool).await.unwrap();
        assert_eq!(get(&pool, moto).await.unwrap(), None);
    }
}
