//! What the bot is waiting for in a private chat. Stored as JSON in the
//! `dialogs` table so a restart does not lose a half-filled form.

use serde::{Deserialize, Serialize};

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
    /// A driver making a counteroffer: `amount` is what the +/− buttons have
    /// reached (it starts at the client's price); ✅ Send sends it, and a
    /// typed amount is sent at once.
    Counteroffer {
        request: String,
        #[serde(default)]
        amount: i64,
    },
    /// The @username guide or the terms are shown; `payload` is the `/start`
    /// argument to resume after them (an invitation, a counteroffer link).
    Terms { payload: Option<String> },
}

/// A closed request's trip, without the client's phone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Republish {
    pub pickup: Spot,
    pub dropoff: Spot,
    pub km: f64,
    pub price: Cup,
    pub payment: PaymentMethod,
}

/// A client requesting a ride, step by step. Pickup and destination are each
/// a pin, a written address the bot understood, or (as a last resort) a free
/// description with the municipality chosen by hand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ride {
    Pickup,
    /// Several places match what the client wrote: they pick one.
    ChoosePickup { text: String, options: Vec<Candidate> },
    /// Nothing matched: the client picks a municipality for their description.
    DescribePickup {
        text: String,
        /// The candidates shown before "None of these", to go back to.
        #[serde(default)]
        choices: Vec<Candidate>,
    },
    /// Then the reparto within it: typed, or one of `options` (buttons).
    RepartoPickup {
        text: String,
        municipality: String,
        options: Vec<Candidate>,
        #[serde(default)]
        choices: Vec<Candidate>,
    },
    ConfirmPickup { pickup: Point, label: Option<String> },
    Reference { pickup: Point },
    Dropoff { pickup: Spot },
    ChooseDropoff { pickup: Spot, text: String, options: Vec<Candidate> },
    DescribeDropoff {
        pickup: Spot,
        text: String,
        #[serde(default)]
        choices: Vec<Candidate>,
    },
    RepartoDropoff {
        pickup: Spot,
        text: String,
        municipality: String,
        options: Vec<Candidate>,
        #[serde(default)]
        choices: Vec<Candidate>,
    },
    ConfirmDropoff { pickup: Spot, dropoff: Point, label: Option<String> },
    /// A description placed at a reparto: "is it around here?" before going
    /// on. `pickup` is `None` while the pickup itself is being confirmed.
    ConfirmApprox { pickup: Option<Spot>, spot: Spot },
    /// The client adjusts their offer, starting at the suggested price.
    Price { trip: Trip, offer: Cup },
    Payment { trip: Trip, price: Cup },
    /// The last look: publish or go back.
    Summary { trip: Trip, price: Cup, payment: PaymentMethod },
}

/// A place the bot found for a written address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub label: String,
    pub point: Point,
    /// What the button shows: the reparto, then the label ("Miramar · Calle 12 entre …").
    #[serde(default)]
    pub short: String,
}

/// Pickup or destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    pub point: Point,
    /// No exact location: the point is the municipality's centre.
    pub approx: bool,
    /// The landmark (pickup) or the client's own description.
    pub note: Option<String>,
}

/// The route once both points are confirmed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trip {
    pub pickup: Spot,
    pub dropoff: Spot,
    /// Already increased if a point is approximate.
    pub km: f64,
    #[serde(default)]
    pub minutes: f64,
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
}

impl DriverForm {
    pub fn inviter(&self) -> Option<UserId> {
        self.invite.as_ref().map(|(_, by)| UserId(*by))
    }
}
