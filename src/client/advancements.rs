use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_update_advancements::{AdvancementProgress, FrameType};
use azalea_registry::identifier::Identifier;

use crate::client::chat_text::to_spans_styled;
use crate::client::tick::item_id;
use crate::gui::toast::{AdvancementFrame, ToastEvent};
use crate::session::SharedMutex;
use crate::text::{Span, Style};

struct Entry {
    display: Option<Display>,
    requirements: Vec<Vec<String>>,
}

struct Display {
    title: Vec<Span>,
    icon: String,
    frame: AdvancementFrame,
}

static STATE: OnceLock<Mutex<HashMap<Identifier, Entry>>> = OnceLock::new();

fn state() -> &'static Mutex<HashMap<Identifier, Entry>> {
    STATE.get_or_init(Default::default)
}

pub(crate) fn reset() {
    state().lock().unwrap().clear();
}

pub(crate) fn packet(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::UpdateAdvancements(p) = packet else {
        return;
    };

    let mut tree = state().lock().unwrap();
    if p.reset {
        tree.clear();
    }
    for id in &p.removed {
        tree.remove(id);
    }
    for holder in &p.added {
        let display = holder
            .value
            .display
            .as_ref()
            .filter(|d| d.show_toast)
            .map(|d| Display {
                title: to_spans_styled(&d.title, Style::colored(0xFFFFFF)),
                icon: item_id(&d.icon).unwrap_or_default().to_owned(),
                frame: match d.frame {
                    FrameType::Task => AdvancementFrame::Task,
                    FrameType::Challenge => AdvancementFrame::Challenge,
                    FrameType::Goal => AdvancementFrame::Goal,
                },
            });
        tree.insert(
            holder.id.clone(),
            Entry {
                display,
                requirements: holder.value.requirements.clone(),
            },
        );
    }

    if p.reset || !p.show_advancements {
        return;
    }
    let mut events: Vec<ToastEvent> = Vec::new();
    for (id, progress) in &p.progress {
        let Some(entry) = tree.get(id) else {
            continue;
        };
        let Some(display) = &entry.display else {
            continue;
        };
        if !is_done(&entry.requirements, progress) {
            continue;
        }
        events.push(ToastEvent::Advancement {
            title: display.title.clone(),
            icon: display.icon.clone(),
            frame: display.frame,
        });
    }
    drop(tree);

    if !events.is_empty() {
        crate::session::queue_toasts(shared, events);
    }
}

fn is_done(requirements: &[Vec<String>], progress: &AdvancementProgress) -> bool {
    if requirements.is_empty() {
        return false;
    }
    requirements.iter().all(|group| {
        group
            .iter()
            .any(|name| progress.get(name).is_some_and(|c| c.date.is_some()))
    })
}
