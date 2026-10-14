//! A ride request and its state machine:
//!
//! ```text
//! Open ──accept──▶ Assigned ──complete / auto-close──▶ Completed
//!  │                  │
//!  ├─cancel──▶ Cancelled ◀──cancel──┘
//!  └─expire──▶ Expired
//! ```
//!
//! Atomicity (one winner per request, one trip per driver) is enforced by the
//! database; these methods validate the transition and return the effects.

use super::effect::{Effect, Notice};
use super::fare::{self, Cup, Fares};
use super::ids::{RequestId, UserId};

/// Unix time in seconds.
pub type UnixTime = i64;

/// How long a request stays open in the channel.
pub const ROUND_SECS: i64 = 5 * 60;
/// Assigned trips nobody marks as completed are closed after this.
pub const AUTO_COMPLETE_SECS: i64 = 2 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Open,
    Assigned,
    Completed,
    Cancelled,
    Expired,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Open => "open",
            State::Assigned => "assigned",
            State::Completed => "completed",
            State::Cancelled => "cancelled",
            State::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "open" => State::Open,
            "assigned" => State::Assigned,
            "completed" => State::Completed,
            "cancelled" => State::Cancelled,
            "expired" => State::Expired,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    #[error("the request is no longer open")]
    NotOpen,
    #[error("the request is not assigned")]
    NotAssigned,
    #[error("a driver cannot accept their own request")]
    OwnRequest,
    #[error("only the client or the assigned driver can do this")]
    NotParticipant,
}

/// What the client fills in before publishing.
#[derive(Debug, Clone)]
pub struct NewRequest {
    pub client: UserId,
    pub client_phone: Option<String>,
    pub pickup: Point,
    pub dropoff: Point,
    pub reference: Option<String>,
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
    pub reference: Option<String>,
    pub km: f64,
    pub price: Cup,
    pub payment: PaymentMethod,
    pub state: State,
    pub driver: Option<UserId>,
    pub channel_msg_id: Option<i32>,
    pub created_at: UnixTime,
    pub assigned_at: Option<UnixTime>,
    pub expires_at: UnixTime,
}

impl Request {
    pub fn open(
        id: RequestId,
        new: NewRequest,
        fares: &Fares,
        now: UnixTime,
    ) -> Result<(Request, Vec<Effect>), DomainError> {
        let minimum = fare::minimum_price(fare::suggested_price(new.km, fares), fares);
        if new.price < minimum {
            return Err(DomainError::PriceBelowMinimum { offered: new.price, minimum });
        }
        let request = Request {
            id: id.clone(),
            client: new.client,
            client_phone: new.client_phone,
            pickup: new.pickup,
            dropoff: new.dropoff,
            reference: new.reference,
            km: new.km,
            price: new.price,
            payment: new.payment,
            state: State::Open,
            driver: None,
            channel_msg_id: None,
            created_at: now,
            assigned_at: None,
            expires_at: now + ROUND_SECS,
        };
        let effects = vec![
            Effect::PublishToChannel { request: id.clone() },
            Effect::Notify { to: new.client, notice: Notice::Published { request: id } },
        ];
        Ok((request, effects))
    }

    /// Validates the acceptance. The caller must then assign atomically in the
    /// database and only apply the effects if it won.
    pub fn accept(&mut self, driver: UserId, now: UnixTime) -> Result<Vec<Effect>, DomainError> {
        if self.state != State::Open || now >= self.expires_at {
            return Err(DomainError::NotOpen);
        }
        if driver == self.client {
            return Err(DomainError::OwnRequest);
        }
        self.state = State::Assigned;
        self.driver = Some(driver);
        self.assigned_at = Some(now);
        let id = &self.id;
        Ok(vec![
            Effect::MarkChannelTaken { request: id.clone() },
            Effect::Notify {
                to: self.client,
                notice: Notice::DriverAssigned { request: id.clone(), driver },
            },
            Effect::Notify {
                to: driver,
                notice: Notice::TripAssigned { request: id.clone(), client: self.client },
            },
            Effect::SendLocation { to: driver, point: self.pickup },
        ])
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
        Ok(self.close_effects(vec![Effect::Notify {
            to: other,
            notice: Notice::Completed { request: self.id.clone() },
        }]))
    }

    /// The client can cancel while open or assigned; the driver only once assigned.
    pub fn cancel(&mut self, by: UserId) -> Result<Vec<Effect>, DomainError> {
        let notify = match (self.state, self.driver) {
            (State::Open, _) if by == self.client => None,
            (State::Assigned, Some(driver)) if by == self.client => Some(Effect::Notify {
                to: driver,
                notice: Notice::CancelledByClient { request: self.id.clone() },
            }),
            (State::Assigned, Some(driver)) if by == driver => Some(Effect::Notify {
                to: self.client,
                notice: Notice::CancelledByDriver { request: self.id.clone() },
            }),
            (State::Open | State::Assigned, _) => return Err(DomainError::NotParticipant),
            _ => return Err(DomainError::NotOpen),
        };
        self.state = State::Cancelled;
        Ok(self.close_effects(notify.into_iter().collect()))
    }

    /// Called by the sweeper. Expires open requests past their round and
    /// auto-completes assigned trips nobody closed.
    pub fn tick(&mut self, now: UnixTime) -> Option<Vec<Effect>> {
        match self.state {
            State::Open if now >= self.expires_at => {
                self.state = State::Expired;
                Some(self.close_effects(vec![Effect::Notify {
                    to: self.client,
                    notice: Notice::Expired { request: self.id.clone() },
                }]))
            }
            State::Assigned
                if self.assigned_at.is_some_and(|at| now >= at + AUTO_COMPLETE_SECS) =>
            {
                self.state = State::Completed;
                Some(self.close_effects(Vec::new()))
            }
            _ => None,
        }
    }

    fn close_effects(&self, mut effects: Vec<Effect>) -> Vec<Effect> {
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

    fn assigned() -> Request {
        let mut r = open();
        r.accept(DRIVER, T0 + 10).unwrap();
        r
    }

    #[test]
    fn opening_publishes_and_sets_the_round_deadline() {
        let id = RequestId::parse("AB7K").unwrap();
        let (r, effects) = Request::open(id.clone(), new_request(580), &fares(), T0).unwrap();
        assert_eq!(r.state, State::Open);
        assert_eq!(r.expires_at, T0 + ROUND_SECS);
        assert_eq!(effects[0], Effect::PublishToChannel { request: id });
    }

    #[test]
    fn opening_rejects_prices_below_the_minimum() {
        let id = RequestId::parse("AB7K").unwrap();
        let err = Request::open(id, new_request(450), &fares(), T0).unwrap_err();
        assert_eq!(err, DomainError::PriceBelowMinimum { offered: 450, minimum: 460 });
    }

    #[test]
    fn accepting_assigns_and_sends_the_pickup_to_the_driver() {
        let mut r = open();
        let effects = r.accept(DRIVER, T0 + 10).unwrap();
        assert_eq!(r.state, State::Assigned);
        assert_eq!(r.driver, Some(DRIVER));
        assert!(effects.contains(&Effect::SendLocation { to: DRIVER, point: r.pickup }));
        assert!(matches!(effects[0], Effect::MarkChannelTaken { .. }));
    }

    #[test]
    fn cannot_accept_own_request_expired_or_taken() {
        assert_eq!(open().accept(CLIENT, T0).unwrap_err(), DomainError::OwnRequest);
        assert_eq!(open().accept(DRIVER, T0 + ROUND_SECS).unwrap_err(), DomainError::NotOpen);
        assert_eq!(assigned().accept(STRANGER, T0 + 20).unwrap_err(), DomainError::NotOpen);
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
    fn cancellation_rules() {
        // Client cancels an open request: nobody else to notify.
        let mut r = open();
        let effects = r.cancel(CLIENT).unwrap();
        assert_eq!(r.state, State::Cancelled);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Notify { .. })));

        // Driver cancels: client is told (and offered to republish).
        let mut r = assigned();
        let effects = r.cancel(DRIVER).unwrap();
        assert!(matches!(
            &effects[0],
            Effect::Notify { to, notice: Notice::CancelledByDriver { .. } } if *to == CLIENT
        ));

        // A driver cannot cancel an open request, nor can strangers.
        assert_eq!(open().cancel(DRIVER).unwrap_err(), DomainError::NotParticipant);
        assert_eq!(assigned().cancel(STRANGER).unwrap_err(), DomainError::NotParticipant);

        // Closed requests cannot be cancelled.
        let mut r = assigned();
        r.complete(CLIENT).unwrap();
        assert_eq!(r.cancel(CLIENT).unwrap_err(), DomainError::NotOpen);
    }

    #[test]
    fn tick_expires_open_requests_after_the_round() {
        let mut r = open();
        assert!(r.tick(T0 + ROUND_SECS - 1).is_none());
        let effects = r.tick(T0 + ROUND_SECS).unwrap();
        assert_eq!(r.state, State::Expired);
        assert!(matches!(
            &effects[0],
            Effect::Notify { notice: Notice::Expired { .. }, .. }
        ));
    }

    #[test]
    fn tick_auto_completes_forgotten_trips() {
        let mut r = assigned();
        assert!(r.tick(T0 + 10 + AUTO_COMPLETE_SECS - 1).is_none());
        assert!(r.tick(T0 + 10 + AUTO_COMPLETE_SECS).is_some());
        assert_eq!(r.state, State::Completed);
    }
}
