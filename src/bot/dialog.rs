//! What the bot is waiting for in a private chat. Stored as JSON in the
//! `dialogs` table so a restart does not lose a half-filled form.

use serde::{Deserialize, Serialize};

use crate::domain::driver::PaymentMethods;
use crate::domain::fare::Cup;
use crate::domain::ids::UserId;
use crate::domain::request::{PaymentMethod, Point};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Dialog {
    Ride(Ride),
    DriverForm(DriverForm),
    /// After a request closed without a trip: what "Publish again" reuses.
    /// Any other message leaves it in place.
    Republish(Republish),
}

/// A closed request's trip, without the client's phone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Republish {
    pub pickup: Point,
    pub reference: Option<String>,
    pub dropoff: Point,
    pub km: f64,
    pub price: Cup,
    pub payment: PaymentMethod,
}

/// A client requesting a ride, step by step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ride {
    Pickup,
    ConfirmPickup { pickup: Point },
    Reference { pickup: Point },
    Dropoff { pickup: Point, reference: Option<String> },
    ConfirmDropoff { pickup: Point, reference: Option<String>, dropoff: Point },
    Price { trip: Trip },
    Payment { trip: Trip, price: Cup },
    Phone { trip: Trip, price: Cup, payment: PaymentMethod },
}

/// The route once both points are confirmed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trip {
    pub pickup: Point,
    pub reference: Option<String>,
    pub dropoff: Point,
    pub km: f64,
    pub suggested: Cup,
    pub minimum: Cup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormStep {
    Alias,
    Phone,
    Plate,
    BikeModel,
    BikeColor,
    Photo,
    BikePhoto,
    Methods,
}

/// A driver filling in or editing their profile. When editing, fields start
/// with the current values and Skip keeps them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriverForm {
    pub step: Option<FormStep>,
    /// Invitation being used, with who created it.
    pub invite: Option<(String, i64)>,
    pub alias: Option<String>,
    pub phone: Option<String>,
    pub plate: Option<String>,
    pub bike_model: Option<String>,
    pub bike_color: Option<String>,
    pub photo_file_id: Option<String>,
    pub bike_photo_file_id: Option<String>,
    pub payment_methods: Option<PaymentMethods>,
}

impl DriverForm {
    pub fn inviter(&self) -> Option<UserId> {
        self.invite.as_ref().map(|(_, by)| UserId(*by))
    }
}
