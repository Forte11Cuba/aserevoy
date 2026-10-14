//! A client requesting a ride: pickup pin → confirm → landmark → destination
//! pin → confirm → route and price → payment → publish. Everyone has a
//! @username, so no phone is asked for.

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, MessageId};

use super::dialog::{Dialog, Republish, Ride, Trip};
use super::{App, chat, effects, keyboards, now};
use crate::db;
use crate::db::requests::InsertOutcome;
use crate::domain::fare::{self, Cup};
use crate::domain::ids::{RequestId, UserId};
use crate::domain::reputation::Role;
use crate::domain::request::{DomainError, NewRequest, PaymentMethod, Point, Request, State};
use crate::domain::driver;
use crate::i18n::{self, Button, Lang, Text, parse_button};

/// Who is talking and in which language.
pub struct Ctx<'a> {
    pub bot: &'a Bot,
    pub app: &'a App,
    pub user: UserId,
    pub lang: Lang,
}

impl Ctx<'_> {
    async fn save(&self, ride: Ride) -> anyhow::Result<()> {
        db::dialogs::set(&self.app.pool, self.user, &Dialog::Ride(ride), now()).await
    }

    async fn say(&self, t: Text, markup: impl Into<teloxide::types::ReplyMarkup>) -> anyhow::Result<()> {
        self.bot.send_message(chat(self.user), i18n::text(self.lang, t)).reply_markup(markup).await?;
        Ok(())
    }

    async fn say_plain(&self, t: Text) -> anyhow::Result<()> {
        self.bot.send_message(chat(self.user), i18n::text(self.lang, t)).await?;
        Ok(())
    }
}

/// Max length of the landmark text shown to the driver.
const REFERENCE_MAX: usize = 100;

pub async fn start(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if let Some(blocked) = cx.app.blocked_message(cx.user, Role::Client, cx.lang).await? {
        cx.bot.send_message(chat(cx.user), blocked).await?;
        return Ok(());
    }
    if let Some(request) = db::requests::active_for(&cx.app.pool, cx.user).await? {
        let id = &request.id;
        let markup: InlineKeyboardMarkup = match request.state {
            State::Assigned if request.client == cx.user => keyboards::client_trip(cx.lang, id),
            State::Assigned => keyboards::driver_trip(cx.lang, id),
            State::Confirming if request.client != cx.user => keyboards::awaiting_client(cx.lang, id),
            _ => keyboards::open_request(cx.lang, id),
        };
        cx.bot
            .send_message(chat(cx.user), i18n::already_active(cx.lang, &request.id))
            .reply_markup(markup)
            .await?;
        return Ok(());
    }
    ask_pickup(cx).await
}

async fn ask_pickup(cx: &Ctx<'_>) -> anyhow::Result<()> {
    cx.save(Ride::Pickup).await?;
    cx.say(Text::AskPickup, keyboards::step(cx.lang, &[keyboards::send_location(cx.lang)])).await
}

async fn ask_dropoff(cx: &Ctx<'_>, pickup: Point, reference: Option<String>) -> anyhow::Result<()> {
    cx.save(Ride::Dropoff { pickup, reference }).await?;
    cx.say(Text::AskDropoff, keyboards::step(cx.lang, &[])).await
}

/// A message while a ride is being requested.
pub async fn on_message(cx: &Ctx<'_>, ride: Ride, msg: &Message) -> anyhow::Result<()> {
    let point = msg.location().map(|l| Point { lat: l.latitude, lon: l.longitude });
    let text = msg.text().map(str::trim);
    let skipped = text.and_then(parse_button) == Some(Button::Skip);

    match ride {
        Ride::Pickup => match point {
            Some(pickup) => {
                cx.save(Ride::ConfirmPickup { pickup }).await?;
                cx.say(Text::ConfirmPickup, keyboards::confirm_point(cx.lang, "pick")).await
            }
            None => cx.say_plain(Text::NeedLocation).await,
        },
        Ride::Dropoff { pickup, reference } => match point {
            Some(dropoff) => {
                cx.save(Ride::ConfirmDropoff { pickup, reference, dropoff }).await?;
                cx.say(Text::ConfirmDropoff, keyboards::confirm_point(cx.lang, "drop")).await
            }
            None => cx.say_plain(Text::NeedLocation).await,
        },
        Ride::Reference { pickup } => {
            if skipped {
                return ask_dropoff(cx, pickup, None).await;
            }
            match text.and_then(|t| driver::clean_field(t, REFERENCE_MAX)) {
                Some(reference) => ask_dropoff(cx, pickup, Some(reference)).await,
                None => cx.say_plain(Text::InvalidField).await,
            }
        }
        Ride::Price { trip } => match text.map(|t| t.parse::<Cup>()) {
            Some(Ok(price)) if price >= trip.minimum => ask_payment(cx, trip, price).await,
            Some(Ok(offered)) => {
                let error = DomainError::PriceBelowMinimum { offered, minimum: trip.minimum };
                cx.bot.send_message(chat(cx.user), i18n::domain_error(cx.lang, &error)).await?;
                Ok(())
            }
            _ => cx.say_plain(Text::AmountNotNumber).await,
        },
        // These steps wait for an inline button; repeat the question.
        Ride::ConfirmPickup { .. } => cx.say(Text::ConfirmPickup, keyboards::confirm_point(cx.lang, "pick")).await,
        Ride::ConfirmDropoff { .. } => {
            cx.say(Text::ConfirmDropoff, keyboards::confirm_point(cx.lang, "drop")).await
        }
        Ride::Payment { .. } => cx.say(Text::AskPayment, keyboards::payment(cx.lang)).await,
    }
}

/// An inline button pressed during the ride form. `pressed` is the message
/// that carried it; its buttons are removed once the step moves on.
pub async fn on_callback(
    cx: &Ctx<'_>,
    ride: Ride,
    kind: &str,
    arg: &str,
    pressed: Option<MessageId>,
) -> anyhow::Result<()> {
    let moved_on = match (ride, kind, arg) {
        (Ride::ConfirmPickup { pickup }, "pick", "ok") => {
            cx.save(Ride::Reference { pickup }).await?;
            cx.say(Text::AskReference, keyboards::step(cx.lang, &[keyboards::skip(cx.lang)])).await?;
            true
        }
        (Ride::ConfirmPickup { .. }, "pick", "fix") => {
            ask_pickup(cx).await?;
            true
        }
        (Ride::ConfirmDropoff { pickup, reference, dropoff }, "drop", "ok") => {
            confirm_route(cx, pickup, reference, dropoff).await?
        }
        (Ride::ConfirmDropoff { pickup, reference, .. }, "drop", "fix") => {
            ask_dropoff(cx, pickup, reference).await?;
            true
        }
        (Ride::Price { trip }, "price", amount) => match amount.parse::<Cup>() {
            Ok(price) if price >= trip.minimum => {
                ask_payment(cx, trip, price).await?;
                true
            }
            _ => false,
        },
        (Ride::Payment { trip, price }, "pay", method) => match PaymentMethod::parse(method) {
            Some(payment) => {
                publish(cx, trip, price, payment).await?;
                true
            }
            None => false,
        },
        // A button from an earlier step: ignore it.
        _ => false,
    };
    if moved_on && let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    Ok(())
}

/// Returns false (and keeps the step) if the route could not be computed.
async fn confirm_route(
    cx: &Ctx<'_>,
    pickup: Point,
    reference: Option<String>,
    dropoff: Point,
) -> anyhow::Result<bool> {
    let route = match cx.app.osrm.route(pickup, dropoff).await {
        Ok(route) => route,
        Err(e) => {
            tracing::warn!(error = %e, "route failed");
            cx.say_plain(Text::RouteFailed).await?;
            return Ok(false);
        }
    };
    let fares = db::fares::load(&cx.app.pool).await?;
    let suggested = fare::suggested_price(route.km, &fares);
    let minimum = fare::minimum_price(suggested, &fares);
    let trip = Trip { pickup, reference, dropoff, km: route.km, suggested, minimum };
    cx.save(Ride::Price { trip }).await?;
    let prompt = i18n::price_prompt(cx.lang, route.km, route.minutes, suggested, minimum);
    cx.bot.send_message(chat(cx.user), prompt).reply_markup(keyboards::prices(suggested)).await?;
    Ok(true)
}

/// 🔁 Publish again: the same trip and price as the request that closed.
pub async fn republish(cx: &Ctx<'_>, again: Republish) -> anyhow::Result<()> {
    if db::requests::active_for(&cx.app.pool, cx.user).await?.is_some() {
        return start(cx).await;
    }
    let fares = db::fares::load(&cx.app.pool).await?;
    let suggested = fare::suggested_price(again.km, &fares);
    let trip = Trip {
        pickup: again.pickup,
        reference: again.reference,
        dropoff: again.dropoff,
        km: again.km,
        suggested,
        minimum: fare::minimum_price(suggested, &fares),
    };
    publish(cx, trip, again.price, again.payment).await
}

async fn ask_payment(cx: &Ctx<'_>, trip: Trip, price: Cup) -> anyhow::Result<()> {
    cx.save(Ride::Payment { trip, price }).await?;
    cx.say(Text::AskPayment, keyboards::payment(cx.lang)).await
}

/// Everyone has a @username, so the client is reachable through it and no
/// phone is asked for or stored.
async fn publish(cx: &Ctx<'_>, trip: Trip, price: Cup, payment: PaymentMethod) -> anyhow::Result<()> {
    let app = cx.app;
    if let Some(blocked) = app.blocked_message(cx.user, Role::Client, cx.lang).await? {
        db::dialogs::clear(&app.pool, cx.user).await?;
        cx.bot.send_message(chat(cx.user), blocked).reply_markup(app.menu(cx.user, cx.lang).await?).await?;
        return Ok(());
    }
    db::reputation::touch(&app.pool, cx.user, Role::Client, now()).await?;
    let fares = db::fares::load(&app.pool).await?;
    let new = NewRequest {
        client: cx.user,
        client_phone: None,
        pickup: trip.pickup,
        dropoff: trip.dropoff,
        reference: trip.reference,
        km: trip.km,
        price,
        payment,
    };

    // Ids are short, so a collision is possible: retry with a new one.
    let mut opened = None;
    for _ in 0..5 {
        let id = RequestId::generate(&mut rand::rng());
        let (request, effects) = match Request::open(id, new.clone(), &fares, now()) {
            Ok(opened) => opened,
            Err(e) => {
                cx.bot.send_message(chat(cx.user), i18n::domain_error(cx.lang, &e)).await?;
                return Ok(());
            }
        };
        match db::requests::insert(&app.pool, &request).await? {
            InsertOutcome::Inserted => {
                opened = Some((request, effects));
                break;
            }
            InsertOutcome::IdTaken => continue,
            InsertOutcome::ClientHasOpenRequest => {
                db::dialogs::clear(&app.pool, cx.user).await?;
                return start(cx).await;
            }
        }
    }
    db::dialogs::clear(&app.pool, cx.user).await?;
    let menu = app.menu(cx.user, cx.lang).await?;
    let Some((mut request, effects)) = opened else {
        cx.say(Text::PublishFailed, menu).await?;
        return Ok(());
    };

    if let Err(e) = effects::apply(cx.bot, app, &mut request, effects).await {
        tracing::error!(request = %request.id, error = %e, "publish failed");
        db::requests::forget(&app.pool, &request.id).await?;
        cx.say(Text::PublishFailed, menu).await?;
    }
    Ok(())
}
