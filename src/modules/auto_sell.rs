use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use azalea::Client;
use azalea_protocol::packets::game::s_chat_command::ServerboundChatCommand;

use super::registry::{Id, auto_sell as setting};
use super::{Edge, Phase, items, store};
use crate::session::{SharedMutex, SlotStack};

const COOLDOWN_TICKS: u32 = 100;

static COOLDOWN: AtomicU32 = AtomicU32::new(0);

static NEXT: AtomicU32 = AtomicU32::new(0);

static EDGE: Edge = Edge::new(Id::AutoSell);

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return,
        Phase::Started => NEXT.store(0, Relaxed),
        Phase::Stopped => return,
        Phase::Running => {}
    }
    let left = COOLDOWN.load(Relaxed);
    if left > 0 {
        COOLDOWN.store(left - 1, Relaxed);
        return;
    }

    if items::LIST.selected() == 0 {
        return;
    }

    let item = {
        let session = &shared.lock().unwrap().session;
        if session.container_id != 0 {
            None
        } else {
            next_match(&session.menu_slots)
        }
    };
    let Some(item) = item else {
        return;
    };

    let template = s.text(setting::COMMAND);
    if template.trim().is_empty() {
        return;
    }

    let command = template.replace("%s", &item);
    bot.write_packet(ServerboundChatCommand {
        command: command.strip_prefix('/').unwrap_or(&command).to_string(),
    });
    COOLDOWN.store(COOLDOWN_TICKS, Relaxed);
}

fn next_match(slots: &[SlotStack]) -> Option<&'static str> {
    let matches = || {
        slots
            .iter()
            .filter(|s| !s.item.is_empty() && items::item_selected(s.item))
    };
    let count = matches().count();
    if count == 0 {
        return None;
    }
    let idx = NEXT.fetch_add(1, Relaxed) as usize % count;
    matches().nth(idx).map(|s| s.item)
}
