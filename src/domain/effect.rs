use super::ids::{RequestId, UserId};
use super::request::Point;

/// What the bot layer must do after a domain operation. The domain never talks
/// to Telegram or the database; it only returns these.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    PublishToChannel { request: RequestId },
    /// Edit the channel post to "Taken" and remove its buttons.
    MarkChannelTaken { request: RequestId },
    DeleteFromChannel { request: RequestId },
    Notify { to: UserId, notice: Notice },
    SendLocation { to: UserId, point: Point },
    /// The request reached a final state: delete its row.
    Forget { request: RequestId },
}

/// Messages for users. The bot renders them in the user's language (see `i18n`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// To the client: their request is in the channel.
    Published { request: RequestId },
    /// To the client: a driver took the trip (rendered with the driver's details).
    DriverAssigned { request: RequestId, driver: UserId },
    /// To the driver: they won the trip (rendered with the client's contact).
    TripAssigned { request: RequestId, client: UserId },
    Completed { request: RequestId },
    /// The client cancelled.
    CancelledByClient { request: RequestId },
    /// The driver cancelled; the client is offered to republish at the same price.
    CancelledByDriver { request: RequestId },
    /// Nobody took it in time; the client is offered to publish again.
    Expired { request: RequestId },
}
