use crate::client::ban::{self, BanDetails};
use crate::gui::painter::Painter;
use crate::gui::tooltip::translate;
use crate::gui::{GuiState, Screen, ScreenCtx, confirmlink};

const SUSPENSION_HELP: &str = "https://aka.ms/mcjavamoderation";

pub(crate) fn before_multiplayer(state: &mut GuiState) {
    if ban::take_unseen() {
        state.nav = Some(Screen::BanNotice);
    }
}

pub(crate) fn poll(state: &mut GuiState) {
    if !ban::take_join_refused() {
        return;
    }
    crate::client::bot::request_disconnect();
    crate::client::bot::shutdown_connection();
    ban::check_once();
    if ban::current().is_some() {
        state.nav = Some(Screen::BanNotice);
    } else {
        let banned = translate("disconnect.loginFailedInfo.userBanned", &[]);
        let reason = translate("disconnect.loginFailedInfo", &[banned]);
        state.disconnect_reason = Some(crate::text::parse_formatted(&reason));
        state.nav = Some(Screen::Disconnected);
    }
}

pub(crate) fn draw(p: &mut Painter, ctx: &ScreenCtx) -> Option<Screen> {
    let Some(ban) = ban::current() else {
        return Some(Screen::Multiplayer);
    };
    crate::gui::menu::background(p, ctx.vw, ctx.vh);
    let title = translate(
        if ban.expires.is_some() {
            "gui.banned.title.temporary"
        } else {
            "gui.banned.title.permanent"
        },
        &[],
    );
    let buttons = [
        translate("chat.link.open", &[]),
        translate("chat.copy", &[]),
        translate("gui.acknowledge", &[]),
    ];
    let chosen = confirmlink::draw_box(
        p,
        ctx,
        &title,
        &message(&ban),
        None,
        &buttons,
        confirmlink::LINK_BUTTON_WIDTH,
    );
    match chosen {
        Some(0) => crate::platform::url::open(SUSPENSION_HELP),
        Some(1) => crate::gui::render::clipboard_set(SUSPENSION_HELP),
        Some(_) => {}
        None if ctx.input.escape => {}
        None => return None,
    }
    Some(Screen::Multiplayer)
}

fn message(ban: &BanDetails) -> String {
    let status = match ban.expires {
        Some(expires) => {
            let now = crate::platform::time::SystemTime::now()
                .duration_since(crate::platform::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64);
            let left = expires - now;
            let (key, amount) = duration_unit(left);
            let duration = translate(key, &[amount.to_string()]);
            let duration = translate("gui.banned.description.temporary.duration", &[duration]);
            translate("gui.banned.description.temporary", &[duration])
        }
        None => translate("gui.banned.description.permanent", &[]),
    };
    let mut text = translate(
        "gui.banned.description",
        &[reason_text(ban), status, SUSPENSION_HELP.to_string()],
    );
    if let Some(id) = &ban.id {
        text.push_str("\n\n");
        text.push_str(&crate::lang::trf!("gui.ban_notice.ban_id", id));
    }
    text
}

fn reason_text(ban: &BanDetails) -> String {
    let id = ban
        .reason
        .as_deref()
        .filter(|r| !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|r| r.parse::<i32>().ok());
    let Some(id) = id else {
        return translate("gui.banned.description.unknownreason", &[]);
    };
    let reason = match (reason_key(id), &ban.reason_message) {
        (Some(key), _) => translate(key, &[]),
        (None, Some(message)) => translate(
            "gui.banned.description.reason_id_message",
            &[id.to_string(), message.clone()],
        ),
        (None, None) => translate("gui.banned.description.reason_id", &[id.to_string()]),
    };
    translate("gui.banned.description.reason", &[reason])
}

fn reason_key(id: i32) -> Option<&'static str> {
    Some(match id {
        2 => "gui.banned.reason.false_reporting",
        5 => "gui.banned.reason.hate_speech",
        16 | 25 => "gui.banned.reason.hate_terrorism_notorious_figure",
        17 | 19 | 23 | 31 => "gui.banned.reason.generic_violation",
        21 => "gui.banned.reason.harassment_or_bullying",
        27 => "gui.banned.reason.defamation_impersonation_false_information",
        28 => "gui.banned.reason.drugs",
        29 => "gui.banned.reason.fraud",
        30 => "gui.banned.reason.spam_or_advertising",
        32 => "gui.banned.reason.nudity_or_pornography",
        33 | 35 | 36 => "gui.banned.reason.sexually_inappropriate",
        34 => "gui.banned.reason.extreme_violence_or_gore",
        53 => "gui.banned.reason.imminent_harm_to_person_or_property",
        _ => return None,
    })
}

fn duration_unit(seconds_left: i64) -> (&'static str, i64) {
    let hours = seconds_left / 3600;
    if hours > 72 {
        ("gui.days", seconds_left / 86_400)
    } else if hours < 1 {
        ("gui.minutes", seconds_left / 60)
    } else {
        ("gui.hours", hours)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_units() {
        assert_eq!(duration_unit(73 * 3600), ("gui.days", 3));
        assert_eq!(duration_unit(72 * 3600 + 3599), ("gui.hours", 72));
        assert_eq!(duration_unit(3600), ("gui.hours", 1));
        assert_eq!(duration_unit(3599), ("gui.minutes", 59));
        assert_eq!(duration_unit(-30), ("gui.minutes", 0));
    }

    #[test]
    fn reasons() {
        assert_eq!(reason_key(2), Some("gui.banned.reason.false_reporting"));
        assert_eq!(
            reason_key(36),
            Some("gui.banned.reason.sexually_inappropriate")
        );
        assert_eq!(reason_key(1), None);
        assert_eq!(reason_key(52), None);
    }
}
