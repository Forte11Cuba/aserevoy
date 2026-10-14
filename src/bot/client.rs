//! A client requesting a ride: pickup → confirm → landmark → destination →
//! confirm → route and price → payment → publish. Each point is a pin, a
//! written address (rules over OSM streets, Jev when there are several
//! candidates), coordinates or a map link, or a free description placed in
//! its reparto.
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
    if text.and_then(parse_button) == Some(Button::StepBack) {
        return back(cx, ride).await;
    }

    match ride {
        Ride::RepartoPickup { text: description, municipality, .. } if point.is_none() && written.is_some() => {
            reparto_typed(cx, None, description, municipality, written.unwrap_or_default()).await
        }
        Ride::RepartoDropoff { pickup, text: description, municipality, .. } if point.is_none() && written.is_some() => {
            reparto_typed(cx, Some(pickup), description, municipality, written.unwrap_or_default()).await
        }
        Ride::Pickup | Ride::ChoosePickup { .. } | Ride::DescribePickup { .. } | Ride::RepartoPickup { .. } => {
            if let Some(pickup) = point {
                cx.save(Ride::ConfirmPickup { pickup, label: None }).await?;
                return cx.say(Text::ConfirmPickup, keyboards::confirm_point(cx.lang, "pick")).await;
            }
            match written {
                Some(text) => resolve(cx, Leg::Pickup, None, text).await,
                None => cx.say_plain(Text::NeedLocation).await,
            }
        }
        Ride::Dropoff { pickup }
        | Ride::ChooseDropoff { pickup, .. }
        | Ride::DescribeDropoff { pickup, .. }
        | Ride::RepartoDropoff { pickup, .. } => {
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
        Ride::Price { trip, .. } => match text.map(|t| t.parse::<Cup>()) {
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
        Ride::Summary { trip, price, payment } => show_summary(cx, trip, price, payment).await,
    }
}

/// ↩️ Back: the previous step, keeping what was already chosen. From the
/// first step it leaves the form.
async fn back(cx: &Ctx<'_>, ride: Ride) -> anyhow::Result<()> {
    match ride {
        Ride::Pickup => {
            db::dialogs::clear(&cx.app.pool, cx.user).await?;
            let menu = cx.app.menu(cx.user, cx.lang).await?;
            cx.say(Text::Cancelled, menu).await
        }
        Ride::ChoosePickup { .. }
        | Ride::DescribePickup { .. }
        | Ride::RepartoPickup { .. }
        | Ride::ConfirmPickup { .. }
        | Ride::Reference { .. } => ask_pickup(cx).await,
        // A described pickup has no landmark step: back to the pickup itself.
        Ride::Dropoff { pickup } if pickup.approx => ask_pickup(cx).await,
        Ride::Dropoff { pickup } => {
            cx.save(Ride::Reference { pickup: pickup.point }).await?;
            cx.say(Text::AskReference, keyboards::step(cx.lang, &[keyboards::skip(cx.lang)])).await
        }
        Ride::ChooseDropoff { pickup, .. }
        | Ride::DescribeDropoff { pickup, .. }
        | Ride::RepartoDropoff { pickup, .. }
        | Ride::ConfirmDropoff { pickup, .. } => ask_dropoff(cx, pickup).await,
        Ride::Price { trip, .. } => ask_dropoff(cx, trip.pickup).await,
        Ride::Payment { trip, price } => show_price(cx, trip, price).await,
        Ride::Summary { trip, price, .. } => ask_payment(cx, trip, price).await,
    }
}

/// The last look before publishing: both points, distance, price and payment.
async fn show_summary(cx: &Ctx<'_>, trip: Trip, price: Cup, payment: PaymentMethod) -> anyhow::Result<()> {
    let place = |spot: &Spot| {
        let municipality = cx.app.zones.municipality(spot.point).unwrap_or("?");
        match (&spot.note, spot.approx) {
            (Some(note), true) => note.clone(),
            _ => match cx.app.streets.nearest_corner(spot.point) {
                Some(corner) => format!("{municipality} ({corner})"),
                None => municipality.to_owned(),
            },
        }
    };
    let approx = trip.pickup.approx || trip.dropoff.approx;
    let text = i18n::ride_summary(cx.lang, &place(&trip.pickup), &place(&trip.dropoff), trip.km, approx, price, payment);
    cx.save(Ride::Summary { trip, price, payment }).await?;
    cx.bot.send_message(chat(cx.user), text).reply_markup(keyboards::summary(cx.lang)).await?;
    Ok(())
}

/// A written address: the rules find the candidates; with one, it is used;
/// with several, Jev picks (≥ 0.7) or the client chooses among the likely
/// ones; with none, the client may describe it and pick the municipality.
async fn resolve(cx: &Ctx<'_>, leg: Leg, pickup: Option<Spot>, text: &str) -> anyhow::Result<()> {
    // Coordinates or a map link: a point, confirmed like any other.
    match cx.app.map_links.point(text).await {
        Ok(Some(point)) if cx.app.zones.municipality(point).is_none() => {
            return cx.say_plain(Text::OutsideHavana).await;
        }
        Ok(Some(point)) => {
            let label = i18n::text(cx.lang, Text::MapPoint).to_owned();
            return show_found(cx, leg, pickup, Candidate { label, point }).await;
        }
        Ok(None) => {}
        Err(e) => {
            // Never log the link: it is a location.
            tracing::warn!(error = %e, "map link failed");
            return cx.say_plain(Text::MapLinkFailed).await;
        }
    }
    let Some(text) = driver::clean_field(text, ADDRESS_MAX) else {
        return cx.say_plain(Text::InvalidField).await;
    };
    let candidates = cx.app.streets.candidates(&text);
    let inferred_any = candidates.iter().any(|c| c.inferred);
    let found: Vec<Candidate> =
        candidates.into_iter().map(|c| Candidate { label: c.label, point: c.point }).collect();
    let decision = match found.len() {
        0 => Decision::Nothing,
        // Deduced from the cross streets: the client knows which street it is
        // and Jev has no name to go by.
        n if inferred_any => Decision::Offer((0..n).collect()),
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
        Decision::Nothing => match cx.app.streets.reparto_in_text(&text) {
            // The description names a reparto ("…, Alamar"): place it there.
            Some((reparto, _)) => described_at(cx, pickup, text, Candidate { label: reparto.name, point: reparto.point }).await,
            None => ask_description(cx, pickup, text).await,
        },
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
        (Ride::DescribePickup { text }, "mun", _) => ask_reparto(cx, None, text, index).await?,
        (Ride::DescribeDropoff { pickup, text }, "mun", _) => ask_reparto(cx, Some(pickup), text, index).await?,
        (Ride::RepartoPickup { ref options, .. } | Ride::RepartoDropoff { ref options, .. }, "zpg", _) => {
            // Another page of repartos, in the same message.
            if let (Some(page), Some(msg)) = (index, pressed) {
                let labels: Vec<&str> = options.iter().map(|c| c.label.as_str()).collect();
                let markup = keyboards::repartos(cx.lang, &labels, page);
                let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).reply_markup(markup).await;
            }
            false
        }
        (Ride::RepartoPickup { text, options, .. }, "zone", _) => match index.and_then(|i| options.get(i)) {
            Some(reparto) => described_at(cx, None, text, reparto.clone()).await.map(|_| true)?,
            None => false,
        },
        (Ride::RepartoDropoff { pickup, text, options, .. }, "zone", _) => match index.and_then(|i| options.get(i)) {
            Some(reparto) => described_at(cx, Some(pickup), text, reparto.clone()).await.map(|_| true)?,
            None => false,
        },
        (Ride::Price { trip, offer }, "padj", delta) => {
            // Never below the minimum; the message shows it.
            if let Ok(delta) = delta.parse::<Cup>() {
                let offer = (offer + delta).max(trip.minimum);
                if let Some(msg) = pressed {
                    let (text, markup) = (price_text(cx, &trip, offer), keyboards::price_adjust(cx.lang, offer));
                    let _ = cx.bot.edit_message_text(chat(cx.user), msg, text).reply_markup(markup).await;
                }
                cx.save(Ride::Price { trip, offer }).await?;
            }
            false
        }
        (Ride::Price { trip, offer }, "pok", _) => {
            ask_payment(cx, trip, offer).await?;
            true
        }
        (Ride::Payment { trip, price }, "pay", method) => match PaymentMethod::parse(method) {
            Some(payment) => {
                show_summary(cx, trip, price, payment).await?;
                true
            }
            None => false,
        },
        (Ride::Summary { trip, price, payment }, "pub", _) => {
            publish(cx, trip, price, payment).await?;
            true
        }
        (Ride::Summary { trip, price, .. }, "sback", _) => {
            ask_payment(cx, trip, price).await?;
            true
        }
        // A button from an earlier step: ignore it.
        _ => false,
    };
    if moved_on && let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    Ok(())
}

/// After the municipality: its repartos as buttons, by pages, or typed.
/// A municipality is too big to be a point (Habana del Este spans 30 km).
async fn ask_reparto(cx: &Ctx<'_>, pickup: Option<Spot>, text: String, index: Option<usize>) -> anyhow::Result<bool> {
    let names = cx.app.zones.names();
    let Some(municipality) = index.and_then(|i| names.get(i)).map(|m| (*m).to_owned()) else {
        return Ok(false);
    };
    let options: Vec<Candidate> = cx
        .app
        .streets
        .repartos_in(&municipality)
        .into_iter()
        .map(|r| Candidate { label: r.name, point: r.point })
        .collect();
    let markup = keyboards::repartos(cx.lang, &options.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), 0);
    let question = Text::AskReparto;
    match pickup {
        None => cx.save(Ride::RepartoPickup { text, municipality, options }).await?,
        Some(pickup) => cx.save(Ride::RepartoDropoff { pickup, text, municipality, options }).await?,
    }
    cx.say(question, markup).await?;
    Ok(true)
}

/// The client typed the reparto: one match is used, several become buttons.
async fn reparto_typed(
    cx: &Ctx<'_>,
    pickup: Option<Spot>,
    text: String,
    municipality: String,
    typed: &str,
) -> anyhow::Result<()> {
    let found: Vec<Candidate> = cx
        .app
        .streets
        .find_repartos(&municipality, typed)
        .into_iter()
        .take(8)
        .map(|r| Candidate { label: r.name, point: r.point })
        .collect();
    match found.len() {
        0 => {
            let message = i18n::reparto_not_found(cx.lang, &municipality);
            cx.bot.send_message(chat(cx.user), message).await?;
            Ok(())
        }
        1 => described_at(cx, pickup, text, found[0].clone()).await,
        _ => {
            let markup = keyboards::repartos(cx.lang, &found.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), 0);
            match pickup {
                None => cx.save(Ride::RepartoPickup { text, municipality, options: found }).await?,
                Some(pickup) => cx.save(Ride::RepartoDropoff { pickup, text, municipality, options: found }).await?,
            }
            cx.say(Text::ChooseReparto, markup).await
        }
    }
}

/// The description placed at the reparto's point: the next step for a pickup
/// (a description is its own landmark), or the route for a destination.
async fn described_at(cx: &Ctx<'_>, pickup: Option<Spot>, text: String, reparto: Candidate) -> anyhow::Result<()> {
    let spot = Spot { point: reparto.point, approx: true, note: Some(format!("{text} ({})", reparto.label)) };
    match pickup {
        None => ask_dropoff(cx, spot).await,
        Some(pickup) => confirm_route(cx, pickup, spot).await.map(|_| ()),
    }
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
    // From a reparto's point the route is an estimate, used as is: the
    // auction (the client's buttons, counteroffers) corrects the price.
    let (km, minutes) = (route.km, route.minutes);
    let fares = db::fares::load(&cx.app.pool).await?;
    let suggested = fare::suggested_price(km, &fares);
    let minimum = fare::minimum_price(suggested, &fares);
    let trip = Trip { pickup, dropoff, km, minutes, suggested, minimum };
    show_price(cx, trip, suggested).await?;
    Ok(true)
}

/// The offer step, with `offer` as the current amount.
async fn show_price(cx: &Ctx<'_>, trip: Trip, offer: Cup) -> anyhow::Result<()> {
    let text = price_text(cx, &trip, offer);
    cx.save(Ride::Price { trip, offer }).await?;
    cx.bot.send_message(chat(cx.user), text).reply_markup(keyboards::price_adjust(cx.lang, offer)).await?;
    Ok(())
}

fn price_text(cx: &Ctx<'_>, trip: &Trip, offer: Cup) -> String {
    let approx = trip.pickup.approx || trip.dropoff.approx;
    i18n::price_prompt(cx.lang, trip.km, trip.minutes, trip.suggested, trip.minimum, offer, approx)
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
        minutes: 0.0,
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
