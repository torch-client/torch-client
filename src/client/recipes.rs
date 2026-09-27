use std::sync::Arc;

use azalea_protocol::common::recipe::{RecipeDisplayData, SlotDisplayData};
use azalea_protocol::packets::game::ClientboundGamePacket;

use crate::gui::toast::ToastEvent;
use crate::session::SharedMutex;

const FLAG_NOTIFICATION: u8 = 1;

pub(crate) fn packet(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::RecipeBookAdd(p) = packet else {
        return;
    };
    let mut events: Vec<ToastEvent> = Vec::new();
    for entry in &p.entries {
        if entry.flags & FLAG_NOTIFICATION == 0 {
            continue;
        }
        let (station, result) = match &entry.contents.display {
            RecipeDisplayData::Shapeless(d) => (&d.crafting_station, &d.result),
            RecipeDisplayData::Shaped(d) => (&d.crafting_station, &d.result),
            RecipeDisplayData::Furnace(d) => (&d.crafting_station, &d.result),
            RecipeDisplayData::Stonecutter(d) => (&d.crafting_station, &d.result),
            RecipeDisplayData::Smithing(d) => (&d.crafting_station, &d.result),
        };
        events.push(ToastEvent::Recipe {
            station: first_stack(station).unwrap_or_default(),
            result: first_stack(result).unwrap_or_default(),
        });
    }
    if !events.is_empty() {
        crate::session::queue_toasts(shared, events);
    }
}

fn first_stack(display: &SlotDisplayData) -> Option<String> {
    match display {
        SlotDisplayData::Item(d) => {
            Some(d.item.to_str().trim_start_matches("minecraft:").to_owned())
        }
        SlotDisplayData::ItemStack(d) => crate::client::tick::item_id(&d.stack).map(str::to_owned),
        SlotDisplayData::WithAnyPotion(d) => first_stack(&d.contents),
        SlotDisplayData::OnlyWithComponent(d) => first_stack(&d.contents),
        SlotDisplayData::WithRemainder(d) => first_stack(&d.input),
        SlotDisplayData::Dyed(d) => first_stack(&d.target),
        SlotDisplayData::SmithingTrim(d) => first_stack(&d.base),
        SlotDisplayData::Composite(d) => d.contents.iter().find_map(first_stack),
        SlotDisplayData::Empty | SlotDisplayData::AnyFuel | SlotDisplayData::Tag(_) => None,
    }
}
