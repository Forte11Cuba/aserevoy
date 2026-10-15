//! Routes updates: private messages, button presses (in private chats and in
//! the channel) and requests to join the offers channel.
//!
//! Before anything else, every private message and button press goes through
//! three gates: the anti-flood (1 action per second), a Telegram @username
//! (so both sides of a trip can reach each other), and the accepted terms.

use teloxide::dispatching::UpdateHandler;
use teloxide::prelude::*;
use teloxide::types::{ChatJoinRequest, MessageId, User};

use super::admin::{self, Action};
use super::client::{self, Ctx};
use super::dialog::Dialog;
use super::{AppRef, HandlerResult, auction, chat, driver, keyboards, now, panel, rating, settings, support, trip, user_id};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::reputation::Role;
use crate::domain::request::CancelReason;
use crate::i18n::{self, Button, TERMS_VERSION, Text, parse_button};

pub async fn run(bot: Bot, app: AppRef) {
    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![app])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

fn schema() -> UpdateHandler<anyhow::Error> {
    dptree::entry()
        .branch(Update::filter_message().filter(|m: Message| m.chat.is_private()).endpoint(on_message))
        .branch(Update::filter_callback_query().endpoint(on_callback))
        .branch(Update::filter_chat_join_request().endpoint(on_join_request))
}

async fn ctx<'a>(bot: &'a Bot, app: &'a AppRef, from: &User) -> anyhow::Result<Ctx<'a>> {
    Ok(Ctx { bot, app, user: user_id(from), lang: app.lang_for(from).await? })
}

async fn on_message(bot: Bot, app: AppRef, msg: Message) -> HandlerResult {
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let cx = ctx(&bot, &app, from).await?;
    if app.too_fast(cx.user) {
        return Ok(());
    }
    let text = msg.text().map(str::trim);
    let start_payload = text.and_then(|t| t.strip_prefix("/start")).map(|rest| rest.trim().to_owned());

    // Gates: a @username, then the terms. A /start link is kept to resume later.
    if from.username.is_none() || !app.accepted_terms(cx.user).await? {
        if let Some(payload) = &start_payload {
            let payload = (!payload.is_empty()).then(|| payload.clone());
            db::dialogs::set(&app.pool, cx.user, &Dialog::Terms { payload }, now()).await?;
        }
        return if from.username.is_none() { ask_username(&cx).await } else { show_terms(&cx).await };
    }

    let pressed = text.and_then(parse_button);
    if pressed == Some(Button::Cancel) {
        let had_dialog = db::dialogs::get::<Dialog>(&app.pool, cx.user).await?.is_some();
        db::dialogs::clear(&app.pool, cx.user).await?;
        let t = if had_dialog { Text::Cancelled } else { Text::Menu };
        return menu(&cx, t).await;
    }
    if let Some(payload) = start_payload {
        db::dialogs::clear(&app.pool, cx.user).await?;
        return resume_start(&cx, Some(payload)).await;
    }
    if let Some(code) = text.and_then(|t| t.strip_prefix("/caso")) {
        return support::lookup(&cx, code).await;
    }
    if text == Some("/metricas") {
        return admin::show_metrics(&cx).await;
    }
    if text == Some("/tarifas") {
        return admin::show_fares(&cx).await;
    }
    if let Some(args) = text.and_then(|t| t.strip_prefix("/tarifa ")) {
        return admin::set_fare(&cx, args).await;
    }
    match db::dialogs::get::<Dialog>(&app.pool, cx.user).await? {
        Some(Dialog::Ride(ride)) => return client::on_message(&cx, ride, &msg).await,
        Some(Dialog::DriverForm(form)) => return driver::on_message(&cx, form, &msg).await,
        Some(Dialog::Raise { request }) => {
            match text.map(|t| t.trim().trim_end_matches("CUP").trim().parse::<i64>()) {
                Some(Ok(amount)) => return auction::raise_typed(&cx, &request, amount).await,
                // Something else: the question is dropped and it is handled as usual.
                _ => db::dialogs::clear(&app.pool, cx.user).await?,
            }
        }
        Some(Dialog::EditField { field }) => return driver::on_edit_message(&cx, field, &msg).await,
        Some(Dialog::EditClient { field }) => return settings::on_message(&cx, field, &msg).await,
        Some(Dialog::Counteroffer { request, amount }) if pressed == Some(Button::SendOffer) => {
            return auction::send_counteroffer(&cx, &request, amount).await;
        }
        Some(Dialog::Counteroffer { request, .. }) => return auction::counteroffer_typed(&cx, &request, &msg).await,
        // Only their buttons use them; messages work as usual.
        Some(Dialog::Republish(_) | Dialog::Terms { .. }) | None => {}
    }
    match pressed {
        Some(Button::RequestRide) => client::start(&cx).await,
        Some(Button::ApplyDriver) => driver::apply(&cx).await,
        Some(Button::WorkWithUs) => driver::work_with_us(&cx).await,
        Some(Button::ApplyManager) => driver::manager_interest(&cx, from.username.as_deref()).await,
        Some(Button::MyProfile) => driver::show_profile(&cx).await,
        Some(Button::DriverMenu) => driver::menu(&cx).await,
        Some(Button::MyStats) => driver::stats(&cx).await,
        Some(Button::InviteDriver) => driver::invite(&cx).await,
        Some(Button::Support) => support::open(&cx).await,
        Some(Button::Settings) => settings::show(&cx).await,
        Some(Button::HowItWorks) => {
            bot.send_message(chat(cx.user), i18n::guide(cx.lang, None))
                .reply_markup(keyboards::guides(cx.lang, None))
                .await?;
            Ok(())
        }
        Some(Button::AdminPanel) => panel::show(&cx).await,
        Some(b @ (Button::PanelDrivers | Button::PanelAdmins | Button::PanelFares | Button::PanelMetrics)) => {
            panel::section(&cx, b).await
        }
        Some(Button::MainMenu) => menu(&cx, Text::Menu).await,
        Some(Button::SwitchLang) => {
            let lang = cx.lang.other();
            db::users::set_lang(&app.pool, cx.user, lang).await?;
            menu(&Ctx { lang, ..cx }, Text::LangChanged).await
        }
        _ => menu(&cx, Text::Menu).await,
    }
}

/// What a `/start` link asked for: an invitation, a counteroffer, or just the menu.
async fn resume_start(cx: &Ctx<'_>, payload: Option<String>) -> HandlerResult {
    let payload = payload.unwrap_or_default();
    if let Some(code) = payload.strip_prefix("inv_") {
        return driver::start_from_invite(cx, code).await;
    }
    if let Some(code) = payload.strip_prefix("adm_") {
        return admin::redeem_admin_invite(cx, code).await;
    }
    if let Some(id) = payload.strip_prefix("co_") {
        return auction::start_counteroffer(cx, id).await;
    }
    menu(cx, Text::Welcome).await
}

async fn ask_username(cx: &Ctx<'_>) -> HandlerResult {
    let markup = keyboards::username_set(cx.lang);
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NeedUsername)).reply_markup(markup).await?;
    Ok(())
}

async fn show_terms(cx: &Ctx<'_>) -> HandlerResult {
    cx.bot.send_message(chat(cx.user), i18n::terms(cx.lang)).reply_markup(keyboards::accept_terms(cx.lang)).await?;
    Ok(())
}

async fn menu(cx: &Ctx<'_>, t: Text) -> HandlerResult {
    let markup = cx.app.menu(cx.user, cx.lang).await?;
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, t)).reply_markup(markup).await?;
    Ok(())
}

/// Always answers the press, even on error, so the button stops spinning.
async fn on_callback(bot: Bot, app: AppRef, q: CallbackQuery) -> HandlerResult {
    let result = handle_callback(&bot, &app, &q).await;
    let answer = bot.answer_callback_query(q.id.clone());
    match &result {
        Ok(Some(text)) => answer.text(text).show_alert(true).await?,
        Ok(None) => answer.await?,
        Err(_) => {
            let lang = app.lang_for(&q.from).await.unwrap_or_default();
            answer.text(i18n::text(lang, Text::SomethingFailed)).show_alert(true).await?
        }
    };
    result.map(|_| ())
}

/// Returns the alert to show, if any.
async fn handle_callback(bot: &Bot, app: &AppRef, q: &CallbackQuery) -> anyhow::Result<Option<String>> {
    let cx = ctx(bot, app, &q.from).await?;
    if app.too_fast(cx.user) {
        return Ok(None);
    }
    let data = q.data.as_deref().unwrap_or_default();
    let (kind, arg) = data.split_once(':').unwrap_or((data, ""));
    // Only a message in the user's own chat: handlers edit it there. One in
    // a channel has the same kind of id and must not be mistaken.
    let pressed: Option<MessageId> = q.message.as_ref().filter(|m| m.chat().id == chat(cx.user)).map(|m| m.id());
    let spend = |msg: Option<MessageId>| async move {
        if let Some(msg) = msg {
            let _ = bot.edit_message_reply_markup(chat(cx.user), msg).await;
        }
    };

    // Gates, as for messages; "I've set it" and "I accept" pass them.
    if q.from.username.is_none() {
        return Ok(Some(i18n::text(cx.lang, Text::StillNoUsername).into()));
    }
    let terms_ok = app.accepted_terms(cx.user).await?;
    match kind {
        "unm" => {
            spend(pressed).await;
            return if terms_ok { resume_after_gates(&cx).await } else { show_terms(&cx).await.map(|_| None) };
        }
        "terms" => {
            spend(pressed).await;
            if !terms_ok {
                db::support::accept_terms(&app.pool, cx.user, TERMS_VERSION, now()).await?;
                db::reputation::touch(&app.pool, cx.user, Role::Client, now()).await?;
                bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::TermsAccepted)).await?;
            }
            return resume_after_gates(&cx).await;
        }
        _ if !terms_ok => return Ok(Some(i18n::text(cx.lang, Text::TermsFirst).into())),
        _ => {}
    }

    let id = arg.split(':').next().and_then(RequestId::parse);
    let alert = match (kind, id) {
        ("acc", Some(id)) => Some(trip::accept(bot, app, cx.user, cx.lang, &id).await?),
        ("cad", Some(id)) => match number(arg) {
            Some(delta) => auction::adjust_counteroffer(&cx, &id, delta, pressed).await?,
            None => None,
        },
        // Buttons from before the +/− steps: they send at once.
        ("cof", Some(id)) => match number(arg) {
            Some(amount) => auction::counteroffer(&cx, &id, amount).await?,
            None => None,
        },
        ("can", Some(id)) => trip::cancel_pressed(bot, app, cx.user, cx.lang, &id, pressed).await?,
        ("cb", Some(id)) => {
            trip::cancel_back(bot, app, cx.user, cx.lang, &id, pressed).await?;
            None
        }
        ("omw" | "arr", Some(id)) => trip::on_my_way_or_arrived(bot, app, cx.user, cx.lang, &id, kind == "arr").await?,
        ("shr", Some(id)) => trip::share(bot, app, cx.user, cx.lang, &id).await?,
        ("fin" | "cr" | "cok" | "cno" | "wd" | "rai" | "kw", Some(id)) => {
            let result = match kind {
                "fin" => trip::complete(bot, app, cx.user, cx.lang, &id).await?,
                "wd" => trip::withdraw(bot, app, cx.user, cx.lang, &id).await?,
                "cr" => {
                    let reason = arg.split_once(':').and_then(|(_, r)| CancelReason::parse(r));
                    trip::cancel(bot, app, cx.user, cx.lang, &id, reason).await?
                }
                "cok" | "cno" => match number(arg) {
                    Some(driver) => {
                        let confirmed = kind == "cok";
                        trip::confirm_or_decline(bot, app, cx.user, cx.lang, &id, UserId(driver), confirmed).await?
                    }
                    None => None,
                },
                "kw" => auction::keep_waiting(&cx, &id).await?,
                _ => match number(arg) {
                    Some(price) => auction::raise(&cx, &id, price).await?,
                    None => None,
                },
            };
            // Done: the buttons of the pressed message are spent.
            if result.is_none() {
                spend(pressed).await;
            }
            result
        }
        ("up", Some(id)) => auction::ask_raise(&cx, &id).await?,
        ("coa" | "cor", Some(id)) => match number(arg) {
            Some(driver) if kind == "coa" => auction::accept_offer(&cx, &id, UserId(driver)).await?,
            Some(driver) => auction::reject_offer(&cx, &id, UserId(driver)).await?,
            None => None,
        },
        ("rep", _) => {
            if let Some(Dialog::Republish(again)) = db::dialogs::get(&app.pool, cx.user).await? {
                spend(pressed).await;
                client::republish(&cx, again).await?;
                None
            } else {
                Some(i18n::text(cx.lang, Text::NoActiveRequest).into())
            }
        }
        ("rs", _) => {
            let (code, stars) = arg.split_once(':').unwrap_or((arg, ""));
            if let Ok(stars) = stars.parse() {
                rating::stars(&cx, code, stars, pressed).await?;
            }
            None
        }
        ("rtg", _) => {
            let (code, tag) = arg.split_once(':').unwrap_or((arg, ""));
            rating::toggle(&cx, code, tag, pressed).await?;
            None
        }
        ("rdn", _) => {
            rating::done(&cx, arg, pressed).await?;
            None
        }
        ("adm", _) => {
            let (action, user) = arg.split_once(':').unwrap_or((arg, ""));
            match (Action::parse(action), user.parse().map(UserId)) {
                (Some(action), Ok(user)) => Some(admin::moderate(&cx, action, user).await?),
                _ => None,
            }
        }
        ("ap", _) => panel::on_callback(&cx, arg, pressed).await?,
        ("ina", _) => admin::invite_admin(&cx).await?,
        ("rma", _) => match arg.parse().map(UserId) {
            Ok(admin_id) => admin::remove_admin(&cx, admin_id, pressed).await?,
            Err(_) => None,
        },
        ("dok" | "dno", _) => driver::review(&cx, kind == "dok", arg, pressed).await?,
        ("dna" | "dnr", _) => driver::review_name(&cx, kind == "dna", arg, pressed).await?,
        ("df", _) => driver::edit_field(&cx, arg).await?,
        // A guide of "❓ How it works", in the same message.
        ("help", _) => {
            if let (Some(guide), Some(msg)) = (i18n::Guide::parse(arg), pressed) {
                let text = i18n::guide(cx.lang, Some(guide));
                let markup = keyboards::guides(cx.lang, Some(guide));
                let _ = bot.edit_message_text(chat(cx.user), msg, text).reply_markup(markup).await;
            }
            None
        }
        ("set", _) => {
            settings::on_callback(&cx, arg).await?;
            None
        }
        // Buttons of the old inline "Work with us" message.
        ("work", _) => {
            spend(pressed).await;
            match arg {
                "drv" => driver::apply(&cx).await?,
                _ => driver::manager_interest(&cx, q.from.username.as_deref()).await?,
            }
            None
        }
        // The old "✏️ Edit profile" button: the profile with its field buttons.
        ("dedit", _) => {
            driver::show_profile(&cx).await?;
            None
        }
        ("pick" | "drop" | "apx" | "cand" | "mun" | "zone" | "zpg" | "dback" | "padj" | "pok" | "pay" | "pub" | "sback", _) => {
            if let Some(Dialog::Ride(ride)) = db::dialogs::get(&app.pool, cx.user).await? {
                client::on_callback(&cx, ride, kind, arg, pressed).await?;
            }
            None
        }
        _ => None,
    };
    Ok(alert)
}

/// Both gates passed: continue with the `/start` link that brought the user,
/// if any, or show the menu.
async fn resume_after_gates(cx: &Ctx<'_>) -> anyhow::Result<Option<String>> {
    let payload = match db::dialogs::get::<Dialog>(&cx.app.pool, cx.user).await? {
        Some(Dialog::Terms { payload }) => {
            db::dialogs::clear(&cx.app.pool, cx.user).await?;
            payload
        }
        _ => None,
    };
    resume_start(cx, payload).await?;
    Ok(None)
}

/// The number after the request id in `<id>:<number>`.
fn number(arg: &str) -> Option<i64> {
    arg.split_once(':').and_then(|(_, n)| n.parse().ok())
}

/// Only active drivers get into the offers channel.
async fn on_join_request(bot: Bot, app: AppRef, request: ChatJoinRequest) -> HandlerResult {
    if request.chat.id != app.channel {
        return Ok(());
    }
    let user = user_id(&request.from);
    if app.driver_status(user).await? == Some(DriverStatus::Active) {
        bot.approve_chat_join_request(app.channel, request.from.id).await?;
        tracing::info!("driver joined the channel");
    } else {
        bot.decline_chat_join_request(app.channel, request.from.id).await?;
    }
    Ok(())
}
