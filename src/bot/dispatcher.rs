//! Routes updates: private messages, button presses (in private chats and in
//! the channel) and requests to join the offers channel.

use teloxide::dispatching::UpdateHandler;
use teloxide::prelude::*;
use teloxide::types::{ChatJoinRequest, MessageId};

use super::client::{self, Ctx};
use super::dialog::Dialog;
use super::{AppRef, HandlerResult, chat, driver, trip, user_id};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::ids::RequestId;
use crate::i18n::{self, Button, Text, parse_button};

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

async fn on_message(bot: Bot, app: AppRef, msg: Message) -> HandlerResult {
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let cx = Ctx {
        bot: &bot,
        app: &app,
        user: user_id(from),
        lang: app.lang_for(from).await?,
        has_username: from.username.is_some(),
    };
    let text = msg.text().map(str::trim);
    let pressed = text.and_then(parse_button);

    if pressed == Some(Button::Cancel) {
        let had_dialog = db::dialogs::get::<Dialog>(&app.pool, cx.user).await?.is_some();
        db::dialogs::clear(&app.pool, cx.user).await?;
        let t = if had_dialog { Text::Cancelled } else { Text::Menu };
        return menu(&cx, t).await;
    }
    if let Some(t) = text
        && let Some(rest) = t.strip_prefix("/start")
    {
        db::dialogs::clear(&app.pool, cx.user).await?;
        return match rest.trim().strip_prefix("inv_") {
            Some(code) => driver::start_from_invite(&cx, code).await,
            None => menu(&cx, Text::Welcome).await,
        };
    }
    match db::dialogs::get::<Dialog>(&app.pool, cx.user).await? {
        Some(Dialog::Ride(ride)) => return client::on_message(&cx, ride, &msg).await,
        Some(Dialog::DriverForm(form)) => return driver::on_message(&cx, form, &msg).await,
        None => {}
    }
    match pressed {
        Some(Button::RequestRide) => client::start(&cx).await,
        Some(Button::ApplyDriver) => driver::apply(&cx).await,
        Some(Button::MyProfile) => driver::show_profile(&cx).await,
        Some(Button::InviteDriver) => driver::invite(&cx).await,
        Some(Button::SwitchLang) => {
            let lang = cx.lang.other();
            db::users::set_lang(&app.pool, cx.user, lang).await?;
            menu(&Ctx { lang, ..cx }, Text::LangChanged).await
        }
        _ => menu(&cx, Text::Menu).await,
    }
}

async fn menu(cx: &Ctx<'_>, t: Text) -> HandlerResult {
    let markup = cx.app.menu(cx.user, cx.lang).await?;
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, t)).reply_markup(markup).await?;
    Ok(())
}

async fn on_callback(bot: Bot, app: AppRef, q: CallbackQuery) -> HandlerResult {
    let cx = Ctx {
        bot: &bot,
        app: &app,
        user: user_id(&q.from),
        lang: app.lang_for(&q.from).await?,
        has_username: q.from.username.is_some(),
    };
    let data = q.data.as_deref().unwrap_or_default();
    let (kind, arg) = data.split_once(':').unwrap_or((data, ""));
    let pressed: Option<MessageId> = q.message.as_ref().map(|m| m.id());

    let alert = match kind {
        "acc" => match RequestId::parse(arg) {
            Some(id) => Some(trip::accept(&bot, &app, cx.user, cx.lang, &id).await?),
            None => None,
        },
        "fin" | "can" => match RequestId::parse(arg) {
            Some(id) => {
                let result = if kind == "fin" {
                    trip::complete(&bot, &app, cx.user, cx.lang, &id).await?
                } else {
                    trip::cancel(&bot, &app, cx.user, cx.lang, &id).await?
                };
                if result.is_none()
                    && let Some(msg) = pressed
                {
                    let _ = bot.edit_message_reply_markup(chat(cx.user), msg).await;
                }
                result
            }
            None => None,
        },
        "dok" | "dno" => driver::review(&cx, kind == "dok", arg, pressed).await?,
        "dedit" => {
            driver::edit(&cx).await?;
            None
        }
        "pick" | "drop" | "price" | "pay" => {
            if let Some(Dialog::Ride(ride)) = db::dialogs::get(&app.pool, cx.user).await? {
                client::on_callback(&cx, ride, kind, arg, pressed).await?;
            }
            None
        }
        "pm" => {
            if let Some(Dialog::DriverForm(form)) = db::dialogs::get(&app.pool, cx.user).await? {
                driver::on_payment_methods(&cx, form, arg, pressed).await?;
            }
            None
        }
        _ => None,
    };

    let answer = bot.answer_callback_query(q.id.clone());
    match alert {
        Some(text) => answer.text(text).show_alert(true).await?,
        None => answer.await?,
    };
    Ok(())
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
