//! A driver's vehicles: the shortcut to switch to the right one for a request
//! (decision 128).

use teloxide::prelude::*;

use super::client::Ctx;
use super::{App, auction, chat, keyboards, now, trip};
use crate::db;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{DomainError, Request};
use crate::domain::vehicle::{self, VehicleId};
use crate::i18n::{self, Lang};

/// What the driver wanted to do with the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Accept,
    Counteroffer,
}

impl Then {
    /// In the callback: `vsa:<request>:<vehicle>` or `vsc:…`.
    pub fn code(self) -> &'static str {
        match self {
            Then::Accept => "vsa",
            Then::Counteroffer => "vsc",
        }
    }
}

/// The driver's active vehicle does not fit `request` (`error`). If they have
/// an approved one that does, they get a private message saying why, with a
/// button to switch to it and go on, and `None` is returned. Otherwise, why.
pub async fn offer_switch(
    bot: &Bot,
    app: &App,
    driver: UserId,
    lang: Lang,
    request: &Request,
    error: &DomainError,
    then: Then,
) -> anyhow::Result<Option<String>> {
    let vehicles = db::vehicles::for_driver(&app.pool, driver).await?;
    let explained = i18n::domain_error(lang, error);
    let Some(fit) = vehicle::switch_candidate(request.vehicle_kind, &vehicles) else {
        return Ok(Some(explained));
    };
    let markup = keyboards::switch_vehicle(lang, &request.id, fit, then);
    bot.send_message(chat(driver), explained).reply_markup(markup).await?;
    Ok(None)
}

/// `vsa`/`vsc`: makes `vehicle` active, then accepts or opens the
/// counteroffer. Returns the alert, if any.
pub async fn switch_and(cx: &Ctx<'_>, id: &RequestId, vehicle: VehicleId, then: Then) -> anyhow::Result<Option<String>> {
    let app = cx.app;
    let now = now();
    if !db::vehicles::set_active(&app.pool, cx.user, vehicle, now).await? {
        let busy = db::requests::driver_busy(&app.pool, cx.user).await?;
        let bidding = db::vehicles::bidding(&app.pool, cx.user, now).await?;
        let error = match db::vehicles::get(&app.pool, vehicle).await?.filter(|v| v.driver == cx.user) {
            Some(v) => vehicle::check_switch(&v, busy, bidding).err().unwrap_or(DomainError::VehicleLocked),
            None => DomainError::NoActiveVehicle,
        };
        return Ok(Some(i18n::domain_error(cx.lang, &error)));
    }
    tracing::info!(request = %id, "vehicle switched to take a request");
    match then {
        Then::Accept => Ok(Some(trip::accept(cx.bot, app, cx.user, cx.lang, id).await?)),
        Then::Counteroffer => {
            auction::start_counteroffer(cx, id.as_str()).await?;
            Ok(None)
        }
    }
}
