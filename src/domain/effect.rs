use super::ids::{RequestId, UserId};
use super::request::Point;

/// What the bot layer must do after a domain operation. The domain never talks
/// to Telegram or the database; it only returns these.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    PublishToChannel { request: RequestId },
    /// Rewrite the channel post after the price changed.
    UpdateChannelPost { request: RequestId },
    /// Edit the channel post to "Waiting for the client" and remove its buttons.
    MarkChannelConfirming { request: RequestId },
    /// Edit the channel post to "Taken" and remove its buttons.
    MarkChannelTaken { request: RequestId },
    DeleteFromChannel { request: RequestId },
    Notify { to: UserId, notice: Notice },
    SendLocation { to: UserId, point: Point, place: Place },
    /// Show the client the current counteroffers in their single, edited
    /// message (or remove it when none are left).
    RefreshOffers { request: RequestId },
    /// The driver is now busy: drop their counteroffers on other requests.
    WithdrawDriverOffers { driver: UserId },
    /// The request reached a final state: delete its row.
    Forget { request: RequestId },
}

/// Which point of the trip a location is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Pickup,
    Dropoff,
}

/// Messages for users. The bot renders them in the user's language (see `i18n`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// To the client: their request is in the channel.
    Published { request: RequestId },
    /// To the client: their new price is in the channel.
    PriceRaised { request: RequestId },
    /// To the client, 30 minutes in: are they still waiting?
    StillWaiting { request: RequestId },
    /// To the client: a driver accepted; confirm or cancel (rendered with the
    /// driver's card, without the phone).
    ConfirmDriver { request: RequestId, driver: UserId },
    /// To the driver: the client has to confirm them.
    AwaitingClient { request: RequestId },
    /// To the client after confirming: the driver's phone.
    DriverConfirmed { request: RequestId, driver: UserId },
    /// To the client after accepting a counteroffer: the driver's full card.
    DriverAssigned { request: RequestId, driver: UserId },
    /// To the driver: they won the trip (rendered with the client's contact).
    TripAssigned { request: RequestId, client: UserId },
    /// To the driver: the client cancelled instead of confirming them.
    ClientDeclined { request: RequestId },
    /// To the driver: the client did not confirm them in time.
    ClientDidNotConfirm { request: RequestId },
    /// To a driver who made a counteroffer: the client rejected it.
    CounterofferRejected { request: RequestId },
    /// To a driver who made a counteroffer: the request was taken or closed.
    RequestGone { request: RequestId },
    Completed { request: RequestId },
    /// The client cancelled the assigned trip.
    CancelledByClient { request: RequestId },
    /// The driver cancelled; the client is offered to publish again.
    CancelledByDriver { request: RequestId },
    /// To the client: they did not confirm the driver in time; publish again?
    ConfirmTimedOut { request: RequestId },
    /// To the client: they turned the driver down; publish again?
    DeclinedDriver { request: RequestId },
    /// Nobody took it in time; the client is offered to publish again.
    Expired { request: RequestId },
}
