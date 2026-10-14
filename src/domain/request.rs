//! A ride request and its state machine:
//!
//! ```text
//!           ┌── counteroffer / raise price / still waiting ──┐
//!           ▼                                                │
//! Open ◀────── driver withdraws ─┐
//! Open ─────┴─ driver accepts ─▶ Confirming ─ client confirms ─▶ Assigned ─ complete / auto-close ─▶ Completed
//!  │  └──── client accepts a counteroffer ─────────────────────▶ Assigned
//!  │                              │ client cancels / no answer      │ client or driver cancels
//!  ├─ client cancels ─────────────┴──────────▶ Cancelled ◀──────────┘
//!  └─ 60 min, or no answer to "still waiting?" ─▶ Expired
//! ```
//!
//! Atomicity (one winner per request, one trip per driver) is enforced by the
//! database; these methods validate the transition and return the effects.

use super::effect::{Alert, Effect, Notice, Place};
use super::fare::{self, Cup, Fares};
use super::ids::{RequestId, UserId};

/// Unix time in seconds.
pub type UnixTime = i64;

/// How long a request stays in the channel.
pub const REQUEST_TTL_SECS: i64 = 60 * 60;
/// When the client is asked "still waiting?".
pub const STILL_WAITING_AFTER_SECS: i64 = 30 * 60;
/// How long the client has to answer it.
pub const STILL_WAITING_ANSWER_SECS: i64 = 10 * 60;
/// How long the client has to confirm the driver who accepted.
pub const CONFIRM_SECS: i64 = 5 * 60;
/// How long a counteroffer stays valid.
pub const COUNTEROFFER_SECS: i64 = 5 * 60;
/// Assigned trips nobody marks as completed are closed after this.
pub const AUTO_COMPLETE_SECS: i64 = 2 * 60 * 60;
/// A client may cancel with "the driver isn't coming" without it counting
/// once this long passed after assignment without "Arrived".
pub const DRIVER_LATE_SECS: i64 = 15 * 60;
/// After "Arrived", the driver waits this long before "the client didn't
/// show up" counts against the client.
pub const NO_SHOW_WAIT_SECS: i64 = 10 * 60;

/// Why an assigned trip is cancelled. The first two are the client's, the
/// rest the driver's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    DriverNotComing,
    ChangedMind,
    ClientNoShow,
    Unsafe,
    Other,
}

impl CancelReason {
    pub const FOR_CLIENT: [CancelReason; 2] = [CancelReason::DriverNotComing, CancelReason::ChangedMind];
    pub const FOR_DRIVER: [CancelReason; 3] = [CancelReason::ClientNoShow, CancelReason::Unsafe, CancelReason::Other];

    pub fn code(self) -> &'static str {
        match self {
            CancelReason::DriverNotComing => "late",
            CancelReason::ChangedMind => "mind",
            CancelReason::ClientNoShow => "noshow",
            CancelReason::Unsafe => "unsafe",
            CancelReason::Other => "other",
        }
    }

    pub fn parse(code: &str) -> Option<CancelReason> {
        Self::FOR_CLIENT.into_iter().chain(Self::FOR_DRIVER).find(|r| r.code() == code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Open,
    /// A driver accepted; the client has to confirm them.
    Confirming,
    Assigned,
    Completed,
    Cancelled,
    Expired,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Open => "open",
            State::Confirming => "confirming",
            State::Assigned => "assigned",
            State::Completed => "completed",
            State::Cancelled => "cancelled",
            State::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "open" => State::Open,
            "confirming" => State::Confirming,
            "assigned" => State::Assigned,
            "completed" => State::Completed,
            "cancelled" => State::Cancelled,
            "expired" => State::Expired,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PaymentMethod {
    Cash,
    Transfer,
}

impl PaymentMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "cash",
            PaymentMethod::Transfer => "transfer",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cash" => Some(PaymentMethod::Cash),
            "transfer" => Some(PaymentMethod::Transfer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("price {offered} is below the minimum {minimum}")]
    PriceBelowMinimum { offered: Cup, minimum: Cup },
    #[error("the new price must be higher than {current}")]
    PriceNotHigher { current: Cup },
    #[error("the request is no longer open")]
    NotOpen,
    #[error("the request is not assigned")]
    NotAssigned,
    #[error("a driver cannot take their own request")]
    OwnRequest,
    #[error("only the client or the assigned driver can do this")]
    NotParticipant,
    #[error("the counteroffer is no longer valid")]
    OfferGone,
    #[error("a reason is needed to cancel this trip")]
    ReasonNeeded,
    #[error("already done")]
    AlreadyDone,
}

/// A driver's latest counteroffer on an open request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counteroffer {
    pub driver: UserId,
    pub amount: Cup,
    pub expires_at: UnixTime,
    /// Its message in the client's chat, once sent.
    pub client_msg_id: Option<i32>,
}

/// What the client fills in before publishing.
#[derive(Debug, Clone)]
pub struct NewRequest {
    pub client: UserId,
    pub client_phone: Option<String>,
    pub pickup: Point,
    pub dropoff: Point,
    /// No exact location: a written description placed at its reparto.
    pub pickup_approx: bool,
    pub dropoff_approx: bool,
    pub reference: Option<String>,
    /// The client's description of the destination, if they wrote one.
    pub dropoff_note: Option<String>,
    pub km: f64,
    pub price: Cup,
    pub payment: PaymentMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: RequestId,
    pub client: UserId,
    pub client_phone: Option<String>,
    pub pickup: Point,
    pub dropoff: Point,
    pub pickup_approx: bool,
    pub dropoff_approx: bool,
    pub reference: Option<String>,
    pub dropoff_note: Option<String>,
    pub km: f64,
    pub price: Cup,
    /// The suggested price when published.
    pub suggested: Cup,
    pub payment: PaymentMethod,
    pub state: State,
    /// Holding the request (`Confirming`) or doing the trip (`Assigned`).
    pub driver: Option<UserId>,
    pub channel_msg_id: Option<i32>,
    pub created_at: UnixTime,
    pub assigned_at: Option<UnixTime>,
    /// When to ask "still waiting?"; `None` once asked.
    pub check_at: Option<UnixTime>,
    /// Deadline to answer "still waiting?" while the question is pending.
    pub answer_by: Option<UnixTime>,
    /// Deadline to confirm the driver while `Confirming`.
    pub confirm_by: Option<UnixTime>,
    /// When the client tapped "On my way" and the driver "Arrived".
    pub on_my_way_at: Option<UnixTime>,
    pub arrived_at: Option<UnixTime>,
    pub expires_at: UnixTime,
    /// Active counteroffers, while `Open`.
    pub counteroffers: Vec<Counteroffer>,
}

/// What the sweeper has to do with a request whose deadline passed.
#[derive(Debug, Clone, PartialEq)]
pub enum Tick {
    /// The request closes: delete it and run the effects.
    Close(Vec<Effect>),
    /// Ask "still waiting?": save the new deadline and run the effects.
    AskStillWaiting(Vec<Effect>),
}

impl Request {
    pub fn open(
        id: RequestId,
        new: NewRequest,
        fares: &Fares,
        now: UnixTime,
    ) -> Result<(Request, Vec<Effect>), DomainError> {
        let suggested = fare::suggested_price(new.km, fares);
        let minimum = fare::minimum_price(suggested, fares);
        if new.price < minimum {
            return Err(DomainError::PriceBelowMinimum { offered: new.price, minimum });
        }
        let request = Request {
            id: id.clone(),
            client: new.client,
            client_phone: new.client_phone,
            pickup: new.pickup,
            dropoff: new.dropoff,
            pickup_approx: new.pickup_approx,
            dropoff_approx: new.dropoff_approx,
            reference: new.reference,
            dropoff_note: new.dropoff_note,
            km: new.km,
            price: new.price,
            suggested,
            payment: new.payment,
            state: State::Open,
            driver: None,
            channel_msg_id: None,
            created_at: now,
            assigned_at: None,
            check_at: Some(now + STILL_WAITING_AFTER_SECS),
            answer_by: None,
            confirm_by: None,
            on_my_way_at: None,
            arrived_at: None,
            expires_at: now + REQUEST_TTL_SECS,
            counteroffers: Vec::new(),
        };
        let effects = vec![
            Effect::PublishToChannel { request: id.clone() },
            Effect::Notify { to: new.client, notice: Notice::Published { request: id } },
        ];
        Ok((request, effects))
    }

    fn check_open(&self, now: UnixTime) -> Result<(), DomainError> {
        if self.state != State::Open || now >= self.expires_at {
            return Err(DomainError::NotOpen);
        }
        Ok(())
    }

    fn check_client(&self, by: UserId) -> Result<(), DomainError> {
        if by != self.client {
            return Err(DomainError::NotParticipant);
        }
        Ok(())
    }

    /// ✅ Accept in the channel: the driver holds the request while the client
    /// decides. The caller must reserve it atomically in the database and only
    /// apply the effects if it won.
    pub fn accept(&mut self, driver: UserId, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        self.check_open(now)?;
        if driver == self.client {
            return Err(DomainError::OwnRequest);
        }
        self.state = State::Confirming;
        self.driver = Some(driver);
        self.confirm_by = Some(now + CONFIRM_SECS);
        self.answer_by = None;
        let id = self.id.clone();
        Ok(vec![
            Effect::MarkChannelConfirming { request: id.clone() },
            Effect::WithdrawDriverOffers { driver },
            Effect::Notify { to: self.client, notice: Notice::ConfirmDriver { request: id.clone(), driver } },
            Effect::Notify { to: driver, notice: Notice::AwaitingClient { request: id } },
        ])
    }

    /// The client confirms the driver who accepted, within the deadline.
    pub fn confirm(&mut self, by: UserId, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        if self.state != State::Confirming || self.confirm_by.is_some_and(|by| now >= by) {
            return Err(DomainError::NotOpen);
        }
        self.check_client(by)?;
        let driver = self.driver.expect("confirming request has a driver");
        self.state = State::Assigned;
        self.assigned_at = Some(now);
        self.confirm_by = None;
        let notice = Notice::DriverConfirmed { request: self.id.clone(), driver };
        Ok(self.assigned_effects(driver, notice))
    }

    /// The driver who accepted changes their mind before the client confirms:
    /// the request goes back to the channel with its buttons.
    pub fn withdraw(&mut self, by: UserId) -> Result<Vec<Effect>, DomainError> {
        if self.state != State::Confirming {
            return Err(DomainError::NotOpen);
        }
        if self.driver != Some(by) {
            return Err(DomainError::NotParticipant);
        }
        self.state = State::Open;
        self.driver = None;
        self.confirm_by = None;
        let id = self.id.clone();
        Ok(vec![
            Effect::UpdateChannelPost { request: id.clone() },
            Effect::Notify { to: self.client, notice: Notice::DriverWithdrew { request: id } },
        ])
    }

    /// The client takes a driver's counteroffer: assigned at that price, with
    /// no confirmation step (the client already chose the driver).
    pub fn accept_counteroffer(
        &mut self,
        by: UserId,
        driver: UserId,
        now: UnixTime,
    ) -> Result<Vec<Effect>, DomainError> {
        self.check_open(now)?;
        self.check_client(by)?;
        let offer = self
            .counteroffers
            .iter()
            .find(|o| o.driver == driver && o.expires_at > now)
            .ok_or(DomainError::OfferGone)?;
        self.price = offer.amount;
        self.state = State::Assigned;
        self.driver = Some(driver);
        self.assigned_at = Some(now);
        self.answer_by = None;
        let mut effects = vec![Effect::WithdrawDriverOffers { driver }];
        let notice = Notice::DriverAssigned { request: self.id.clone(), driver };
        effects.extend(self.assigned_effects(driver, notice));
        Ok(effects)
    }

    /// Effects of entering `Assigned`: the post is marked taken, the other
    /// drivers who made offers are told, and both sides get each other's
    /// details. The driver only now learns the exact points.
    fn assigned_effects(&mut self, driver: UserId, to_client: Notice) -> Vec<Effect> {
        let id = self.id.clone();
        let mut effects = vec![Effect::MarkChannelTaken { request: id.clone() }];
        effects.extend(self.dismiss_offers(Some(driver)));
        effects.extend([
            Effect::Notify { to: self.client, notice: to_client },
            Effect::Notify { to: driver, notice: Notice::TripAssigned { request: id, client: self.client } },
            Effect::SendLocation { to: driver, point: self.pickup, place: Place::Pickup },
            Effect::SendLocation { to: driver, point: self.dropoff, place: Place::Dropoff },
        ]);
        effects
    }

    /// Clears the counteroffers and their messages, telling every driver but
    /// `except` that the request is gone.
    fn dismiss_offers(&mut self, except: Option<UserId>) -> Vec<Effect> {
        let mut effects = Vec::new();
        for offer in std::mem::take(&mut self.counteroffers) {
            effects.extend(self.hide(&offer));
            if Some(offer.driver) != except {
                let notice = Notice::RequestGone { request: self.id.clone() };
                effects.push(Effect::Notify { to: offer.driver, notice });
            }
        }
        effects
    }

    /// Deletes the counteroffer's message from the client's chat, if it was sent.
    fn hide(&self, offer: &Counteroffer) -> Option<Effect> {
        offer.client_msg_id.map(|msg| Effect::HideOffer { client: self.client, msg })
    }

    /// A driver offers to do the trip for `amount`, above the current price.
    /// It replaces their previous counteroffer on this request.
    pub fn counteroffer(&mut self, driver: UserId, amount: Cup, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        self.check_open(now)?;
        if driver == self.client {
            return Err(DomainError::OwnRequest);
        }
        if amount <= self.price {
            return Err(DomainError::PriceNotHigher { current: self.price });
        }
        // The new one replaces the previous counteroffer and its message.
        let mut effects = Vec::new();
        if let Some(pos) = self.counteroffers.iter().position(|o| o.driver == driver) {
            let previous = self.counteroffers.remove(pos);
            effects.extend(self.hide(&previous));
        }
        let expires_at = now + COUNTEROFFER_SECS;
        self.counteroffers.push(Counteroffer { driver, amount, expires_at, client_msg_id: None });
        effects.push(Effect::ShowOffer { request: self.id.clone(), driver });
        Ok(effects)
    }

    /// The client turns down a counteroffer; the driver may make another.
    pub fn reject_counteroffer(&mut self, by: UserId, driver: UserId) -> Result<Vec<Effect>, DomainError> {
        if self.state != State::Open {
            return Err(DomainError::NotOpen);
        }
        self.check_client(by)?;
        let pos = self.counteroffers.iter().position(|o| o.driver == driver).ok_or(DomainError::OfferGone)?;
        let offer = self.counteroffers.remove(pos);
        let mut effects: Vec<Effect> = self.hide(&offer).into_iter().collect();
        let notice = Notice::CounterofferRejected { request: self.id.clone() };
        effects.push(Effect::Notify { to: driver, notice });
        Ok(effects)
    }

    /// The client raises the price. Counteroffers at or below it are dropped
    /// and their drivers told they can accept the new price directly.
    /// Raising also answers a pending "still waiting?".
    pub fn raise_price(&mut self, by: UserId, price: Cup, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        self.check_open(now)?;
        self.check_client(by)?;
        if price <= self.price {
            return Err(DomainError::PriceNotHigher { current: self.price });
        }
        self.price = price;
        self.answer_by = None;
        let id = self.id.clone();
        let mut effects = vec![
            Effect::UpdateChannelPost { request: id.clone() },
            Effect::Notify { to: self.client, notice: Notice::PriceRaised { request: id.clone() } },
        ];
        let (outbid, kept): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.counteroffers).into_iter().partition(|o| o.amount <= price);
        self.counteroffers = kept;
        for offer in outbid {
            effects.extend(self.hide(&offer));
            let notice = Notice::OfferOutbid { request: id.clone(), price };
            effects.push(Effect::Notify { to: offer.driver, notice });
        }
        Ok(effects)
    }

    /// The client answers "yes" to "still waiting?".
    pub fn keep_waiting(&mut self, by: UserId, now: UnixTime) -> Result<(), DomainError> {
        self.check_open(now)?;
        self.check_client(by)?;
        self.answer_by = None;
        Ok(())
    }

    /// 🚶 On my way: the client tells the driver they are heading to the pickup.
    pub fn on_my_way(&mut self, by: UserId, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        let driver = self.assigned_driver()?;
        self.check_client(by)?;
        if self.on_my_way_at.is_some() {
            return Err(DomainError::AlreadyDone);
        }
        self.on_my_way_at = Some(now);
        Ok(vec![Effect::Notify { to: driver, notice: Notice::ClientOnTheWay { request: self.id.clone() } }])
    }

    /// 📍 Arrived: the driver is at the pickup.
    pub fn arrived(&mut self, by: UserId, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        let driver = self.assigned_driver()?;
        if by != driver {
            return Err(DomainError::NotParticipant);
        }
        if self.arrived_at.is_some() {
            return Err(DomainError::AlreadyDone);
        }
        self.arrived_at = Some(now);
        Ok(vec![Effect::Notify { to: self.client, notice: Notice::DriverArrived { request: self.id.clone() } }])
    }

    fn assigned_driver(&self) -> Result<UserId, DomainError> {
        match (self.state, self.driver) {
            (State::Assigned, Some(driver)) => Ok(driver),
            _ => Err(DomainError::NotAssigned),
        }
    }

    /// Either participant can mark the trip as completed.
    pub fn complete(&mut self, by: UserId) -> Result<Vec<Effect>, DomainError> {
        if self.state != State::Assigned {
            return Err(DomainError::NotAssigned);
        }
        let driver = self.driver.expect("assigned request has a driver");
        let other = if by == self.client {
            driver
        } else if by == driver {
            self.client
        } else {
            return Err(DomainError::NotParticipant);
        };
        self.state = State::Completed;
        let effects = vec![
            Effect::Notify { to: other, notice: Notice::Completed { request: self.id.clone() } },
            Effect::StartRating { client: self.client, driver },
        ];
        Ok(self.close_effects(effects))
    }

    /// The client can cancel while open, confirming (turning the driver down)
    /// or assigned; the driver only once assigned. An assigned trip needs a
    /// reason, which decides whether it counts against the client:
    /// - the client's cancellations count, except "the driver isn't coming"
    ///   15 minutes after assignment without "Arrived";
    /// - the driver's count against the client only for "the client didn't
    ///   show up" 10 minutes after "Arrived"; "I felt unsafe" alerts the admins.
    pub fn cancel(
        &mut self,
        by: UserId,
        reason: Option<CancelReason>,
        now: UnixTime,
    ) -> Result<Vec<Effect>, DomainError> {
        let id = self.id.clone();
        let mut effects = match (self.state, self.driver) {
            (State::Open, _) if by == self.client => vec![],
            (State::Confirming, Some(driver)) if by == self.client => vec![
                Effect::Notify { to: driver, notice: Notice::ClientDeclined { request: id.clone() } },
                Effect::Notify { to: self.client, notice: Notice::DeclinedDriver { request: id } },
            ],
            (State::Assigned, Some(driver)) if by == self.client => {
                let reason = reason.filter(|r| CancelReason::FOR_CLIENT.contains(r)).ok_or(DomainError::ReasonNeeded)?;
                let driver_late = self.arrived_at.is_none()
                    && self.assigned_at.is_some_and(|at| now >= at + DRIVER_LATE_SECS);
                let mut effects =
                    vec![Effect::Notify { to: driver, notice: Notice::CancelledByClient { request: id } }];
                if !(reason == CancelReason::DriverNotComing && driver_late) {
                    effects.push(Effect::CountCancellation { client: self.client });
                }
                effects
            }
            (State::Assigned, Some(driver)) if by == driver => {
                let reason = reason.filter(|r| CancelReason::FOR_DRIVER.contains(r)).ok_or(DomainError::ReasonNeeded)?;
                let mut effects =
                    vec![Effect::Notify { to: self.client, notice: Notice::CancelledByDriver { request: id.clone() } }];
                match reason {
                    CancelReason::ClientNoShow if self.arrived_at.is_some_and(|at| now >= at + NO_SHOW_WAIT_SECS) => {
                        effects.push(Effect::CountCancellation { client: self.client });
                    }
                    CancelReason::Unsafe => effects.push(Effect::Alert {
                        alert: Alert::UnsafeTrip { request: id, client: self.client, driver },
                    }),
                    _ => {}
                }
                effects
            }
            (State::Open | State::Confirming | State::Assigned, _) => return Err(DomainError::NotParticipant),
            _ => return Err(DomainError::NotOpen),
        };
        self.state = State::Cancelled;
        effects = self.close_effects(effects);
        Ok(effects)
    }

    /// Called by the sweeper when a deadline passes.
    pub fn tick(&mut self, now: UnixTime) -> Option<Tick> {
        let id = self.id.clone();
        match self.state {
            State::Open if now >= self.expires_at || self.answer_by.is_some_and(|by| now >= by) => {
                self.state = State::Expired;
                let notify = vec![Effect::Notify { to: self.client, notice: Notice::Expired { request: id } }];
                Some(Tick::Close(self.close_effects(notify)))
            }
            State::Open if self.answer_by.is_none() && self.check_at.is_some_and(|at| now >= at) => {
                self.check_at = None;
                self.answer_by = Some(now + STILL_WAITING_ANSWER_SECS);
                let ask = Effect::Notify { to: self.client, notice: Notice::StillWaiting { request: id } };
                Some(Tick::AskStillWaiting(vec![ask]))
            }
            State::Confirming if self.confirm_by.is_some_and(|by| now >= by) => {
                let driver = self.driver.expect("confirming request has a driver");
                self.state = State::Cancelled;
                let notify = vec![
                    Effect::Notify { to: driver, notice: Notice::ClientDidNotConfirm { request: id.clone() } },
                    Effect::Notify { to: self.client, notice: Notice::ConfirmTimedOut { request: id } },
                ];
                Some(Tick::Close(self.close_effects(notify)))
            }
            State::Assigned if self.assigned_at.is_some_and(|at| now >= at + AUTO_COMPLETE_SECS) => {
                let driver = self.driver.expect("assigned request has a driver");
                self.state = State::Completed;
                let rate = vec![Effect::StartRating { client: self.client, driver }];
                Some(Tick::Close(self.close_effects(rate)))
            }
            _ => None,
        }
    }

    fn close_effects(&mut self, mut effects: Vec<Effect>) -> Vec<Effect> {
        effects.extend(self.dismiss_offers(None));
        effects.push(Effect::DeleteFromChannel { request: self.id.clone() });
        effects.push(Effect::Forget { request: self.id.clone() });
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT: UserId = UserId(1);
    const DRIVER: UserId = UserId(2);
    const STRANGER: UserId = UserId(3);
    const OTHER_DRIVER: UserId = UserId(4);
    const T0: UnixTime = 1_000_000;

    fn fares() -> Fares {
        Fares { base: 100, per_km: 150, min_percent: 80 }
    }

    fn new_request(price: Cup) -> NewRequest {
        NewRequest {
            client: CLIENT,
            client_phone: None,
            pickup: Point { lat: 23.13, lon: -82.38 },
            dropoff: Point { lat: 23.12, lon: -82.40 },
            pickup_approx: false,
            dropoff_approx: false,
            dropoff_note: None,
            reference: Some("frente a la bodega".into()),
            km: 3.2, // suggested 580, minimum 460
            price,
            payment: PaymentMethod::Cash,
        }
    }

    fn open() -> Request {
        let id = RequestId::parse("AB7K").unwrap();
        Request::open(id, new_request(580), &fares(), T0).unwrap().0
    }

    fn confirming() -> Request {
        let mut r = open();
        r.accept(DRIVER, T0 + 10).unwrap();
        r
    }

    fn assigned() -> Request {
        let mut r = confirming();
        r.confirm(CLIENT, T0 + 20).unwrap();
        r
    }

    fn notified(effects: &[Effect], who: UserId) -> Vec<&Notice> {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::Notify { to, notice } if *to == who => Some(notice),
                _ => None,
            })
            .collect()
    }

    fn has_location(effects: &[Effect]) -> bool {
        effects.iter().any(|e| matches!(e, Effect::SendLocation { .. }))
    }

    #[test]
    fn opening_publishes_with_60_minutes_and_a_check_at_30() {
        let id = RequestId::parse("AB7K").unwrap();
        let (r, effects) = Request::open(id.clone(), new_request(580), &fares(), T0).unwrap();
        assert_eq!(r.state, State::Open);
        assert_eq!(r.expires_at, T0 + REQUEST_TTL_SECS);
        assert_eq!(r.check_at, Some(T0 + STILL_WAITING_AFTER_SECS));
        assert_eq!(effects[0], Effect::PublishToChannel { request: id });
    }

    #[test]
    fn opening_rejects_prices_below_the_minimum() {
        let id = RequestId::parse("AB7K").unwrap();
        let err = Request::open(id, new_request(450), &fares(), T0).unwrap_err();
        assert_eq!(err, DomainError::PriceBelowMinimum { offered: 450, minimum: 460 });
    }

    #[test]
    fn accepting_holds_the_request_without_revealing_the_points() {
        let mut r = open();
        let effects = r.accept(DRIVER, T0 + 10).unwrap();
        assert_eq!(r.state, State::Confirming);
        assert_eq!(r.driver, Some(DRIVER));
        assert_eq!(r.confirm_by, Some(T0 + 10 + CONFIRM_SECS));
        assert!(matches!(effects[0], Effect::MarkChannelConfirming { .. }));
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::ConfirmDriver { .. }]));
        assert!(!has_location(&effects));
    }

    #[test]
    fn confirming_assigns_and_sends_pickup_and_dropoff() {
        let mut r = confirming();
        assert_eq!(r.confirm(DRIVER, T0 + 20).unwrap_err(), DomainError::NotParticipant);
        let effects = r.confirm(CLIENT, T0 + 20).unwrap();
        assert_eq!(r.state, State::Assigned);
        assert!(effects.contains(&Effect::SendLocation { to: DRIVER, point: r.pickup, place: Place::Pickup }));
        assert!(effects.contains(&Effect::SendLocation { to: DRIVER, point: r.dropoff, place: Place::Dropoff }));
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::DriverConfirmed { .. }]));
    }

    #[test]
    fn cannot_accept_own_request_expired_or_held() {
        assert_eq!(open().accept(CLIENT, T0).unwrap_err(), DomainError::OwnRequest);
        assert_eq!(open().accept(DRIVER, T0 + REQUEST_TTL_SECS).unwrap_err(), DomainError::NotOpen);
        assert_eq!(confirming().accept(STRANGER, T0 + 20).unwrap_err(), DomainError::NotOpen);
    }

    #[test]
    fn turning_the_driver_down_cancels_and_tells_both() {
        let mut r = confirming();
        let effects = r.cancel(CLIENT, None, T0 + 30).unwrap();
        assert_eq!(r.state, State::Cancelled);
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::ClientDeclined { .. }]));
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::DeclinedDriver { .. }]));
        assert!(matches!(effects.last(), Some(Effect::Forget { .. })));
        // The waiting driver cannot cancel in the client's place.
        assert_eq!(confirming().cancel(DRIVER, None, T0 + 30).unwrap_err(), DomainError::NotParticipant);
    }

    #[test]
    fn the_driver_can_withdraw_before_confirmation() {
        let mut r = confirming();
        assert_eq!(r.withdraw(STRANGER).unwrap_err(), DomainError::NotParticipant);
        let effects = r.withdraw(DRIVER).unwrap();
        assert_eq!((r.state, r.driver, r.confirm_by), (State::Open, None, None));
        assert!(matches!(effects[0], Effect::UpdateChannelPost { .. }));
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::DriverWithdrew { .. }]));
        // Back in the channel: another driver can take it.
        assert!(r.accept(OTHER_DRIVER, T0 + 30).is_ok());
        assert_eq!(assigned().withdraw(DRIVER).unwrap_err(), DomainError::NotOpen);
    }

    #[test]
    fn confirming_after_the_deadline_fails() {
        let mut r = confirming();
        assert_eq!(r.confirm(CLIENT, T0 + 10 + CONFIRM_SECS).unwrap_err(), DomainError::NotOpen);
    }

    #[test]
    fn unconfirmed_drivers_are_released_after_5_minutes() {
        let mut r = confirming();
        assert!(r.tick(T0 + 10 + CONFIRM_SECS - 1).is_none());
        let Some(Tick::Close(effects)) = r.tick(T0 + 10 + CONFIRM_SECS) else { panic!("should close") };
        assert_eq!(r.state, State::Cancelled);
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::ClientDidNotConfirm { .. }]));
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::ConfirmTimedOut { .. }]));
    }

    #[test]
    fn a_confirmation_in_progress_outlives_the_60_minutes() {
        let mut r = open();
        r.accept(DRIVER, T0 + REQUEST_TTL_SECS - 60).unwrap();
        assert!(r.tick(T0 + REQUEST_TTL_SECS + 1).is_none());
        assert!(r.confirm(CLIENT, T0 + REQUEST_TTL_SECS + 2).is_ok());
    }

    #[test]
    fn still_waiting_is_asked_at_30_minutes_and_expires_without_answer() {
        let mut r = open();
        assert!(r.tick(T0 + STILL_WAITING_AFTER_SECS - 1).is_none());
        let asked = T0 + STILL_WAITING_AFTER_SECS;
        let Some(Tick::AskStillWaiting(effects)) = r.tick(asked) else { panic!("should ask") };
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::StillWaiting { .. }]));
        assert_eq!(r.answer_by, Some(asked + STILL_WAITING_ANSWER_SECS));
        assert!(r.tick(asked + 1).is_none()); // asked only once

        let Some(Tick::Close(effects)) = r.tick(asked + STILL_WAITING_ANSWER_SECS) else { panic!("should expire") };
        assert_eq!(r.state, State::Expired);
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::Expired { .. }]));
    }

    #[test]
    fn answering_keeps_the_request_until_60_minutes() {
        let mut r = open();
        let asked = T0 + STILL_WAITING_AFTER_SECS;
        r.tick(asked).unwrap();
        assert_eq!(r.keep_waiting(DRIVER, asked + 1).unwrap_err(), DomainError::NotParticipant);
        r.keep_waiting(CLIENT, asked + 1).unwrap();
        assert!(r.tick(asked + STILL_WAITING_ANSWER_SECS).is_none());
        assert!(matches!(r.tick(T0 + REQUEST_TTL_SECS), Some(Tick::Close(_))));
        assert_eq!(r.state, State::Expired);
    }

    #[test]
    fn raising_the_price_updates_the_post_and_drops_cheaper_offers() {
        let mut r = open();
        r.counteroffer(DRIVER, 630, T0 + 1).unwrap();
        r.counteroffer(OTHER_DRIVER, 700, T0 + 1).unwrap();
        assert_eq!(r.raise_price(CLIENT, 580, T0 + 2).unwrap_err(), DomainError::PriceNotHigher { current: 580 });
        assert_eq!(r.raise_price(DRIVER, 650, T0 + 2).unwrap_err(), DomainError::NotParticipant);

        let effects = r.raise_price(CLIENT, 650, T0 + 2).unwrap();
        assert_eq!(r.price, 650);
        assert!(matches!(effects[0], Effect::UpdateChannelPost { .. }));
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::OfferOutbid { price: 650, .. }]));
        assert_eq!(r.counteroffers.len(), 1);
        assert_eq!(r.counteroffers[0].driver, OTHER_DRIVER);
    }

    #[test]
    fn counteroffers_replace_previous_ones_and_must_beat_the_price() {
        let mut r = open();
        assert_eq!(r.counteroffer(DRIVER, 580, T0).unwrap_err(), DomainError::PriceNotHigher { current: 580 });
        assert_eq!(r.counteroffer(CLIENT, 700, T0).unwrap_err(), DomainError::OwnRequest);
        let effects = r.counteroffer(DRIVER, 630, T0).unwrap();
        assert_eq!(effects, vec![Effect::ShowOffer { request: r.id.clone(), driver: DRIVER }]);
        r.counteroffers[0].client_msg_id = Some(77); // the bot sent it

        // The new one replaces it, and its message goes.
        let effects = r.counteroffer(DRIVER, 680, T0 + 5).unwrap();
        assert_eq!(effects[0], Effect::HideOffer { client: CLIENT, msg: 77 });
        let expected = Counteroffer { driver: DRIVER, amount: 680, expires_at: T0 + 5 + COUNTEROFFER_SECS, client_msg_id: None };
        assert_eq!(r.counteroffers, vec![expected]);
    }

    #[test]
    fn accepting_a_counteroffer_assigns_at_its_price_and_dismisses_the_rest() {
        let mut r = open();
        r.counteroffer(DRIVER, 630, T0).unwrap();
        r.counteroffer(OTHER_DRIVER, 700, T0).unwrap();
        let effects = r.accept_counteroffer(CLIENT, DRIVER, T0 + 10).unwrap();
        assert_eq!((r.state, r.price, r.driver), (State::Assigned, 630, Some(DRIVER)));
        assert!(matches!(notified(&effects, OTHER_DRIVER)[..], [Notice::RequestGone { .. }]));
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::DriverAssigned { .. }]));
        assert!(has_location(&effects));
        assert!(r.counteroffers.is_empty());
    }

    #[test]
    fn expired_or_unknown_counteroffers_cannot_be_accepted() {
        let mut r = open();
        r.counteroffer(DRIVER, 630, T0).unwrap();
        let late = T0 + COUNTEROFFER_SECS;
        assert_eq!(r.accept_counteroffer(CLIENT, DRIVER, late).unwrap_err(), DomainError::OfferGone);
        assert_eq!(r.accept_counteroffer(CLIENT, STRANGER, T0 + 1).unwrap_err(), DomainError::OfferGone);
        assert_eq!(r.accept_counteroffer(DRIVER, DRIVER, T0 + 1).unwrap_err(), DomainError::NotParticipant);
    }

    #[test]
    fn rejecting_a_counteroffer_tells_the_driver() {
        let mut r = open();
        r.counteroffer(DRIVER, 630, T0).unwrap();
        let effects = r.reject_counteroffer(CLIENT, DRIVER).unwrap();
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::CounterofferRejected { .. }]));
        assert!(r.counteroffers.is_empty());
        assert_eq!(r.reject_counteroffer(CLIENT, DRIVER).unwrap_err(), DomainError::OfferGone);
    }

    #[test]
    fn closing_tells_drivers_with_pending_offers_and_hides_them() {
        let mut r = open();
        r.counteroffer(DRIVER, 630, T0).unwrap();
        r.counteroffers[0].client_msg_id = Some(77);
        let effects = r.cancel(CLIENT, None, T0 + 30).unwrap();
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::RequestGone { .. }]));
        assert!(effects.contains(&Effect::HideOffer { client: CLIENT, msg: 77 }));
        assert!(matches!(effects.last(), Some(Effect::Forget { .. })));
    }

    #[test]
    fn either_participant_completes_and_the_other_is_notified() {
        for (by, other) in [(CLIENT, DRIVER), (DRIVER, CLIENT)] {
            let mut r = assigned();
            let effects = r.complete(by).unwrap();
            assert_eq!(r.state, State::Completed);
            assert!(matches!(effects[0], Effect::Notify { to, .. } if to == other));
            assert!(matches!(effects.last(), Some(Effect::Forget { .. })));
        }
        assert_eq!(assigned().complete(STRANGER).unwrap_err(), DomainError::NotParticipant);
        assert_eq!(open().complete(CLIENT).unwrap_err(), DomainError::NotAssigned);
    }

    #[test]
    fn cancellation_of_assigned_trips() {
        use CancelReason::*;
        let mut r = assigned();
        assert_eq!(r.cancel(DRIVER, None, T0 + 30).unwrap_err(), DomainError::ReasonNeeded);
        // The client's reasons are not the driver's.
        assert_eq!(r.cancel(DRIVER, Some(ChangedMind), T0 + 30).unwrap_err(), DomainError::ReasonNeeded);
        let effects = r.cancel(DRIVER, Some(Other), T0 + 30).unwrap();
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::CancelledByDriver { .. }]));

        let mut r = assigned();
        let effects = r.cancel(CLIENT, Some(ChangedMind), T0 + 30).unwrap();
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::CancelledByClient { .. }]));

        assert_eq!(open().cancel(DRIVER, None, T0 + 30).unwrap_err(), DomainError::NotParticipant);
        assert_eq!(assigned().cancel(STRANGER, Some(Other), T0 + 30).unwrap_err(), DomainError::NotParticipant);
        let mut r = assigned();
        r.complete(CLIENT).unwrap();
        assert_eq!(r.cancel(CLIENT, Some(ChangedMind), T0 + 30).unwrap_err(), DomainError::NotOpen);
    }

    fn counts(effects: &[Effect]) -> bool {
        effects.contains(&Effect::CountCancellation { client: CLIENT })
    }

    #[test]
    fn whose_cancellation_counts() {
        use CancelReason::*;
        let assigned_at = T0 + 20;
        // The client's own cancellations count...
        assert!(counts(&assigned().cancel(CLIENT, Some(ChangedMind), assigned_at + 60).unwrap()));
        // ..."the driver isn't coming" too, unless 15 min passed without "Arrived".
        assert!(counts(&assigned().cancel(CLIENT, Some(DriverNotComing), assigned_at + 60).unwrap()));
        let late = assigned_at + DRIVER_LATE_SECS;
        assert!(!counts(&assigned().cancel(CLIENT, Some(DriverNotComing), late).unwrap()));
        let mut r = assigned();
        r.arrived(DRIVER, assigned_at + 60).unwrap();
        assert!(counts(&r.cancel(CLIENT, Some(DriverNotComing), late).unwrap()));

        // "The client didn't show up" counts only 10 min after "Arrived".
        assert!(!counts(&assigned().cancel(DRIVER, Some(ClientNoShow), late).unwrap()));
        let mut r = assigned();
        r.arrived(DRIVER, assigned_at + 60).unwrap();
        assert!(!counts(&r.clone().cancel(DRIVER, Some(ClientNoShow), assigned_at + 60 + NO_SHOW_WAIT_SECS - 1).unwrap()));
        assert!(counts(&r.cancel(DRIVER, Some(ClientNoShow), assigned_at + 60 + NO_SHOW_WAIT_SECS).unwrap()));

        // "I felt unsafe" never counts against the client and alerts the admins.
        let effects = assigned().cancel(DRIVER, Some(Unsafe), late).unwrap();
        assert!(!counts(&effects));
        assert!(effects.iter().any(|e| matches!(e, Effect::Alert { alert: Alert::UnsafeTrip { .. } })));
    }

    #[test]
    fn on_my_way_and_arrived_tell_the_other_side_once() {
        let mut r = assigned();
        assert_eq!(r.on_my_way(DRIVER, T0 + 30).unwrap_err(), DomainError::NotParticipant);
        let effects = r.on_my_way(CLIENT, T0 + 30).unwrap();
        assert!(matches!(notified(&effects, DRIVER)[..], [Notice::ClientOnTheWay { .. }]));
        assert_eq!(r.on_my_way(CLIENT, T0 + 31).unwrap_err(), DomainError::AlreadyDone);

        assert_eq!(r.arrived(CLIENT, T0 + 40).unwrap_err(), DomainError::NotParticipant);
        let effects = r.arrived(DRIVER, T0 + 40).unwrap();
        assert!(matches!(notified(&effects, CLIENT)[..], [Notice::DriverArrived { .. }]));
        assert_eq!(r.arrived_at, Some(T0 + 40));
        assert_eq!(open().arrived(DRIVER, T0).unwrap_err(), DomainError::NotAssigned);
    }

    #[test]
    fn finished_trips_start_the_rating() {
        let effects = assigned().complete(DRIVER).unwrap();
        assert!(effects.contains(&Effect::StartRating { client: CLIENT, driver: DRIVER }));
        let mut r = assigned();
        let Some(Tick::Close(effects)) = r.tick(T0 + 20 + AUTO_COMPLETE_SECS) else { panic!() };
        assert!(effects.contains(&Effect::StartRating { client: CLIENT, driver: DRIVER }));
        let effects = assigned().cancel(CLIENT, Some(CancelReason::ChangedMind), T0 + 30).unwrap();
        assert!(!effects.iter().any(|e| matches!(e, Effect::StartRating { .. })));
    }

    #[test]
    fn tick_auto_completes_forgotten_trips() {
        let mut r = assigned();
        assert!(r.tick(T0 + 20 + AUTO_COMPLETE_SECS - 1).is_none());
        assert!(matches!(r.tick(T0 + 20 + AUTO_COMPLETE_SECS), Some(Tick::Close(_))));
        assert_eq!(r.state, State::Completed);
    }
}
