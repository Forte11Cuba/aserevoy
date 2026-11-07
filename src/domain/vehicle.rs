//! Vehicles (decisions 116 to 130). A driver has up to five, of any kind, and
//! works with one, the active one. A request is for one kind, and only a
//! driver whose active vehicle is of that kind can take it.

use super::ids::UserId;
use super::request::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum VehicleKind {
    #[default]
    Moto,
    Car,
    Tricycle,
}

impl VehicleKind {
    /// In the order the client sees them.
    pub const ALL: [VehicleKind; 3] = [VehicleKind::Moto, VehicleKind::Car, VehicleKind::Tricycle];

    /// In the database and in callbacks.
    pub fn code(self) -> &'static str {
        match self {
            VehicleKind::Moto => "moto",
            VehicleKind::Car => "car",
            VehicleKind::Tricycle => "tricycle",
        }
    }

    pub fn parse(code: &str) -> Option<VehicleKind> {
        VehicleKind::ALL.into_iter().find(|k| k.code() == code)
    }

    /// The word a superadmin writes in `/tarifa carro km 80`, in Spanish or
    /// English.
    pub fn parse_word(word: &str) -> Option<VehicleKind> {
        match word.to_lowercase().as_str() {
            "moto" => Some(VehicleKind::Moto),
            "carro" | "car" => Some(VehicleKind::Car),
            "triciclo" | "tricycle" => Some(VehicleKind::Tricycle),
            _ => None,
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            VehicleKind::Moto => "🏍",
            VehicleKind::Car => "🚗",
            VehicleKind::Tricycle => "🛺",
        }
    }
}

/// A rejected vehicle is deleted (decision 129), so there are only two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleStatus {
    /// Waiting for an admin: it cannot be used yet.
    Pending,
    Approved,
}

impl VehicleStatus {
    pub fn code(self) -> &'static str {
        match self {
            VehicleStatus::Pending => "pending",
            VehicleStatus::Approved => "approved",
        }
    }

    pub fn parse(code: &str) -> Option<VehicleStatus> {
        match code {
            "pending" => Some(VehicleStatus::Pending),
            "approved" => Some(VehicleStatus::Approved),
            _ => None,
        }
    }
}

pub type VehicleId = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vehicle {
    pub id: VehicleId,
    pub driver: UserId,
    pub kind: VehicleKind,
    pub plate: String,
    pub model: String,
    pub color: String,
    pub photo_file_id: Option<String>,
    pub status: VehicleStatus,
    /// A new plate or photo waiting for an admin (decision 122); meanwhile the
    /// vehicle works with the old ones.
    pub pending_plate: Option<String>,
    pub pending_photo_file_id: Option<String>,
}

impl Vehicle {
    pub fn approved(&self) -> bool {
        self.status == VehicleStatus::Approved
    }

    pub fn has_pending_change(&self) -> bool {
        self.pending_plate.is_some() || self.pending_photo_file_id.is_some()
    }
}

pub const MAX_PER_DRIVER: usize = 5;

/// Whether a driver can accept or counteroffer a request for `wanted`, with
/// `active` as their active vehicle. The database checks it again atomically.
pub fn check_fits(wanted: VehicleKind, active: Option<&Vehicle>) -> Result<(), DomainError> {
    match active {
        Some(v) if !v.approved() => Err(DomainError::NoActiveVehicle),
        Some(v) if v.kind != wanted => Err(DomainError::WrongVehicle { wanted, active: v.kind }),
        Some(_) => Ok(()),
        None => Err(DomainError::NoActiveVehicle),
    }
}

/// The vehicle to offer in "🚗 Use … and accept" (decision 128) when the
/// active one does not fit: the driver's first approved one of that kind.
pub fn switch_candidate(wanted: VehicleKind, vehicles: &[Vehicle]) -> Option<&Vehicle> {
    vehicles.iter().find(|v| v.kind == wanted && v.approved())
}

/// Whether the driver can make `target` their active vehicle. `busy` is a
/// request confirming or a trip assigned; `bidding`, a live counteroffer: the
/// client who confirmed a car must not get a motorbike (decision 120).
pub fn check_switch(target: &Vehicle, busy: bool, bidding: bool) -> Result<(), DomainError> {
    if !target.approved() {
        return Err(DomainError::VehicleNotApproved);
    }
    if busy || bidding {
        return Err(DomainError::VehicleLocked);
    }
    Ok(())
}

/// Removing the active vehicle has the same limits as switching away from it;
/// any other one can go at any time.
pub fn check_remove(target: &Vehicle, active: Option<VehicleId>, busy: bool, bidding: bool) -> Result<(), DomainError> {
    if active == Some(target.id) && (busy || bidding) {
        return Err(DomainError::VehicleLocked);
    }
    Ok(())
}

pub fn check_add(count: usize) -> Result<(), DomainError> {
    if count >= MAX_PER_DRIVER {
        return Err(DomainError::TooManyVehicles);
    }
    Ok(())
}

/// The kinds the client can pick (decision 127): those some active driver has
/// an approved vehicle of, active or not, in the order of `ALL`. With none,
/// the moto, so a request can always be published.
pub fn offered_kinds(registered: &[VehicleKind]) -> Vec<VehicleKind> {
    let kinds: Vec<VehicleKind> = VehicleKind::ALL.into_iter().filter(|k| registered.contains(k)).collect();
    if kinds.is_empty() { vec![VehicleKind::Moto] } else { kinds }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vehicle(id: VehicleId, kind: VehicleKind, status: VehicleStatus) -> Vehicle {
        Vehicle {
            id,
            driver: UserId(7),
            kind,
            plate: "P123456".into(),
            model: "Lada".into(),
            color: "rojo".into(),
            photo_file_id: None,
            status,
            pending_plate: None,
            pending_photo_file_id: None,
        }
    }

    #[test]
    fn codes_round_trip() {
        for kind in VehicleKind::ALL {
            assert_eq!(VehicleKind::parse(kind.code()), Some(kind));
        }
        assert_eq!(VehicleKind::parse("bus"), None);
        for status in [VehicleStatus::Pending, VehicleStatus::Approved] {
            assert_eq!(VehicleStatus::parse(status.code()), Some(status));
        }
    }

    #[test]
    fn words_in_both_languages() {
        assert_eq!(VehicleKind::parse_word("Carro"), Some(VehicleKind::Car));
        assert_eq!(VehicleKind::parse_word("TRICYCLE"), Some(VehicleKind::Tricycle));
        assert_eq!(VehicleKind::parse_word("moto"), Some(VehicleKind::Moto));
        assert_eq!(VehicleKind::parse_word("km"), None);
    }

    #[test]
    fn only_an_approved_active_vehicle_of_the_kind_fits() {
        let moto = vehicle(1, VehicleKind::Moto, VehicleStatus::Approved);
        let pending_car = vehicle(2, VehicleKind::Car, VehicleStatus::Pending);
        assert_eq!(check_fits(VehicleKind::Moto, Some(&moto)), Ok(()));
        assert_eq!(
            check_fits(VehicleKind::Car, Some(&moto)),
            Err(DomainError::WrongVehicle { wanted: VehicleKind::Car, active: VehicleKind::Moto })
        );
        assert_eq!(check_fits(VehicleKind::Car, Some(&pending_car)), Err(DomainError::NoActiveVehicle));
        assert_eq!(check_fits(VehicleKind::Moto, None), Err(DomainError::NoActiveVehicle));
    }

    #[test]
    fn the_shortcut_offers_an_approved_vehicle_of_the_kind() {
        let vehicles = [
            vehicle(1, VehicleKind::Moto, VehicleStatus::Approved),
            vehicle(2, VehicleKind::Car, VehicleStatus::Pending),
            vehicle(3, VehicleKind::Car, VehicleStatus::Approved),
        ];
        assert_eq!(switch_candidate(VehicleKind::Car, &vehicles).map(|v| v.id), Some(3));
        assert_eq!(switch_candidate(VehicleKind::Tricycle, &vehicles), None);
    }

    #[test]
    fn switching_waits_for_approval_and_for_a_free_driver() {
        let car = vehicle(2, VehicleKind::Car, VehicleStatus::Approved);
        let pending = vehicle(3, VehicleKind::Car, VehicleStatus::Pending);
        assert_eq!(check_switch(&car, false, false), Ok(()));
        assert_eq!(check_switch(&car, true, false), Err(DomainError::VehicleLocked));
        assert_eq!(check_switch(&car, false, true), Err(DomainError::VehicleLocked));
        assert_eq!(check_switch(&pending, false, false), Err(DomainError::VehicleNotApproved));
    }

    #[test]
    fn only_the_active_vehicle_is_locked_against_removal() {
        let moto = vehicle(1, VehicleKind::Moto, VehicleStatus::Approved);
        assert_eq!(check_remove(&moto, Some(1), true, false), Err(DomainError::VehicleLocked));
        assert_eq!(check_remove(&moto, Some(1), false, false), Ok(()));
        assert_eq!(check_remove(&moto, Some(2), true, true), Ok(()));
        assert_eq!(check_remove(&moto, None, true, false), Ok(()));
    }

    #[test]
    fn up_to_five_vehicles() {
        assert_eq!(check_add(4), Ok(()));
        assert_eq!(check_add(5), Err(DomainError::TooManyVehicles));
    }

    #[test]
    fn the_client_sees_the_kinds_someone_registered() {
        use VehicleKind::*;
        assert_eq!(offered_kinds(&[Car, Tricycle, Car]), vec![Car, Tricycle]);
        assert_eq!(offered_kinds(&[Tricycle, Moto]), vec![Moto, Tricycle]);
        assert_eq!(offered_kinds(&[]), vec![Moto]);
    }
}
