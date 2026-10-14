//! A client requesting a ride: pickup → confirm → landmark → destination →
//! confirm → route and price → payment → publish. Each point is a pin, a
//! written address (rules over OSM streets, Jev when there are several
//! candidates), or a free description placed in a chosen municipality.
//! Everyone has a @username, so no phone is asked for.

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, MessageId};

use super::dialog::{Candidate, Dialog, Republish, Ride, Spot, Trip};
use super::{App, chat, effects, keyboards, now};
use crate::db;
use crate::db::requests::InsertOutcome;
use crate::domain::fare::{self, Cup};
use crate::domain::ids::{RequestId, UserId};
use crate::domain::reputation::Role;
use crate::domain::request::{DomainError, NewRequest, PaymentMethod, Point, Request, State};
use crate::domain::driver;
use crate::i18n::{self, Button, Lang, Text, parse_button};
use crate::location::jev::{self, Decision};

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
/// Max length of a written address or description.
const ADDRESS_MAX: usize = 150;

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

async fn ask_dropoff(cx: &Ctx<'_>, pickup: Spot) -> anyhow::Result<()> {
    cx.save(Ride::Dropoff { pickup }).await?;
    cx.say(Text::AskDropoff, keyboards::step(cx.lang, &[])).await
}

/// Which point of the trip a step is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leg {
    Pickup,
    Dropoff,
}

/// A message while a ride is being requested.
pub async fn on_message(cx: &Ctx<'_>, ride: Ride, msg: &Message) -> anyhow::Result<()> {
    let point = msg.location().map(|l| Point { lat: l.latitude, lon: l.longitude });
    let text = msg.text().map(str::trim).filter(|t| !t.is_empty());
    let skipped = text.and_then(parse_button) == Some(Button::Skip);
    // Free text that is not one of our buttons: maybe a written address.
    let written = text.filter(|t| parse_button(t).is_none());

    match ride {
        Ride::Pickup | Ride::ChoosePickup { .. } | Ride::DescribePickup { .. } => {
            if let Some(pickup) = point {
                cx.save(Ride::ConfirmPickup { pickup, label: None }).await?;
                return cx.say(Text::ConfirmPickup, keyboards::confirm_point(cx.lang, "pick")).await;
            }
            match written {
                Some(text) => resolve(cx, Leg::Pickup, None, text).await,
                None => cx.say_plain(Text::NeedLocation).await,
            }
        }
        Ride::Dropoff { pickup } | Ride::ChooseDropoff { pickup, .. } | Ride::DescribeDropoff { pickup, .. } => {
            if let Some(dropoff) = point {
                cx.save(Ride::ConfirmDropoff { pickup, dropoff, label: None }).await?;
                return cx.say(Text::ConfirmDropoff, keyboards::confirm_point(cx.lang, "drop")).await;
            }
            match written {
                Some(text) => resolve(cx, Leg::Dropoff, Some(pickup), text).await,
                None => cx.say_plain(Text::NeedLocation).await,
            }
        }
        Ride::Reference { pickup } => {
            let exact = |note| Spot { point: pickup, approx: false, note };
            if skipped {
                return ask_dropoff(cx, exact(None)).await;
            }
            match text.and_then(|t| driver::clean_field(t, REFERENCE_MAX)) {
                Some(reference) => ask_dropoff(cx, exact(Some(reference))).await,
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

/// A written address: the rules find the candidates; with one, it is used;
/// with several, Jev picks (≥ 0.7) or the client chooses among the likely
/// ones; with none, the client may describe it and pick the municipality.
async fn resolve(cx: &Ctx<'_>, leg: Leg, pickup: Option<Spot>, text: &str) -> anyhow::Result<()> {
    let Some(text) = driver::clean_field(text, ADDRESS_MAX) else {
        return cx.say_plain(Text::InvalidField).await;
    };
    let found: Vec<Candidate> = cx
        .app
        .streets
        .candidates(&text)
        .into_iter()
        .map(|c| Candidate { label: c.label, point: c.point })
        .collect();
    let decision = match found.len() {
        0 => Decision::Nothing,
        1 => Decision::Use(0),
        _ => match &cx.app.jev {
            Some(jev) => {
                let labels: Vec<String> = found.iter().map(|c| c.label.clone()).collect();
                match jev.ask(&text, &labels).await {
                    Ok(verdict) => jev::decide(&verdict),
                    Err(e) => {
                        // Jev down: the client chooses among the first ones.
                        tracing::warn!(error = %e, "jev failed");
                        Decision::Offer((0..found.len().min(jev::MAX_OFFERED)).collect())
                    }
                }
            }
            None => Decision::Offer((0..found.len().min(jev::MAX_OFFERED)).collect()),
        },
    };
    match decision {
        Decision::Use(i) => show_found(cx, leg, pickup, found[i].clone()).await,
        Decision::Offer(indices) => {
            let options: Vec<Candidate> = indices.into_iter().map(|i| found[i].clone()).collect();
            let markup = keyboards::candidates(cx.lang, &options.iter().map(|c| c.label.as_str()).collect::<Vec<_>>());
            match pickup {
                None => cx.save(Ride::ChoosePickup { text, options }).await?,
                Some(pickup) => cx.save(Ride::ChooseDropoff { pickup, text, options }).await?,
            }
            cx.say(Text::ChooseAddress, markup).await
        }
        Decision::Nothing => ask_description(cx, pickup, text).await,
    }
}

/// Shows the place found on the map and asks "Is it here?".
async fn show_found(cx: &Ctx<'_>, leg: Leg, pickup: Option<Spot>, found: Candidate) -> anyhow::Result<()> {
    cx.bot.send_location(chat(cx.user), found.point.lat, found.point.lon).await?;
    let (which, question) = match (leg, pickup) {
        (Leg::Pickup, _) => {
            cx.save(Ride::ConfirmPickup { pickup: found.point, label: Some(found.label.clone()) }).await?;
            ("pick", i18n::confirm_written(cx.lang, true, &found.label))
        }
        (Leg::Dropoff, pickup) => {
            let pickup = pickup.expect("the destination comes after the pickup");
            cx.save(Ride::ConfirmDropoff { pickup, dropoff: found.point, label: Some(found.label.clone()) }).await?;
            ("drop", i18n::confirm_written(cx.lang, false, &found.label))
        }
    };
    cx.bot.send_message(chat(cx.user), question).reply_markup(keyboards::confirm_point(cx.lang, which)).await?;
    Ok(())
}

/// Nothing found: send the pin, or keep the description and pick the
/// municipality (the driver gets the description after confirmation).
async fn ask_description(cx: &Ctx<'_>, pickup: Option<Spot>, text: String) -> anyhow::Result<()> {
    match pickup {
        None => cx.save(Ride::DescribePickup { text }).await?,
        Some(pickup) => cx.save(Ride::DescribeDropoff { pickup, text }).await?,
    }
    cx.say(Text::AddressNotFound, keyboards::municipalities(&cx.app.zones.names())).await
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
    let index = arg.parse::<usize>().ok();
    let moved_on = match (ride, kind, arg) {
        (Ride::ConfirmPickup { pickup, .. }, "pick", "ok") => {
            cx.save(Ride::Reference { pickup }).await?;
            cx.say(Text::AskReference, keyboards::step(cx.lang, &[keyboards::skip(cx.lang)])).await?;
            true
        }
        (Ride::ConfirmPickup { .. }, "pick", "fix") => {
            ask_pickup(cx).await?;
            true
        }
        (Ride::ConfirmDropoff { pickup, dropoff, .. }, "drop", "ok") => {
            let dropoff = Spot { point: dropoff, approx: false, note: None };
            confirm_route(cx, pickup, dropoff).await?
        }
        (Ride::ConfirmDropoff { pickup, .. }, "drop", "fix") => {
            ask_dropoff(cx, pickup).await?;
            true
        }
        (Ride::ChoosePickup { text, .. }, "cand", "none") => {
            ask_description(cx, None, text).await?;
            true
        }
        (Ride::ChooseDropoff { pickup, text, .. }, "cand", "none") => {
            ask_description(cx, Some(pickup), text).await?;
            true
        }
        (Ride::ChoosePickup { options, .. }, "cand", _) => match index.and_then(|i| options.get(i)) {
            Some(found) => {
                show_found(cx, Leg::Pickup, None, found.clone()).await?;
                true
            }
            None => false,
        },
        (Ride::ChooseDropoff { pickup, options, .. }, "cand", _) => match index.and_then(|i| options.get(i)) {
            Some(found) => {
                show_found(cx, Leg::Dropoff, Some(pickup), found.clone()).await?;
                true
            }
            None => false,
        },
        (Ride::DescribePickup { text }, "mun", _) => match described(cx, index, text) {
            // A description is its own landmark: straight to the destination.
            Some(pickup) => {
                ask_dropoff(cx, pickup).await?;
                true
            }
            None => false,
        },
        (Ride::DescribeDropoff { pickup, text }, "mun", _) => match described(cx, index, text) {
            Some(dropoff) => confirm_route(cx, pickup, dropoff).await?,
            None => false,
        },
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

/// The description placed at the chosen municipality's centre.
fn described(cx: &Ctx<'_>, municipality: Option<usize>, text: String) -> Option<Spot> {
    let names = cx.app.zones.names();
    let point = cx.app.zones.centre(names.get(municipality?)?)?;
    Some(Spot { point, approx: true, note: Some(text) })
}

/// Returns false (and keeps the step) if the route could not be computed.
async fn confirm_route(cx: &Ctx<'_>, pickup: Spot, dropoff: Spot) -> anyhow::Result<bool> {
    let route = match cx.app.osrm.route(pickup.point, dropoff.point).await {
        Ok(route) => route,
        Err(e) => {
            tracing::warn!(error = %e, "route failed");
            cx.say_plain(Text::RouteFailed).await?;
            return Ok(false);
        }
    };
    // Without an exact point the route is a guess: the driver is paid for it.
    let approx = pickup.approx || dropoff.approx;
    let (km, minutes) = if approx {
        (fare::approximate_km(route.km), fare::approximate_km(route.minutes))
    } else {
        (route.km, route.minutes)
    };
    let fares = db::fares::load(&cx.app.pool).await?;
    let suggested = fare::suggested_price(km, &fares);
    let minimum = fare::minimum_price(suggested, &fares);
    let trip = Trip { pickup, dropoff, km, suggested, minimum };
    cx.save(Ride::Price { trip }).await?;
    let prompt = i18n::price_prompt(cx.lang, km, minutes, suggested, minimum, approx);
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
        pickup: trip.pickup.point,
        dropoff: trip.dropoff.point,
        pickup_approx: trip.pickup.approx,
        dropoff_approx: trip.dropoff.approx,
        reference: trip.pickup.note,
        dropoff_note: trip.dropoff.note,
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
