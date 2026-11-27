use azalea::Client;
use azalea::block::BlockState;
use azalea::entity::inventory::Inventory;
use azalea_core::game_type::GameMode;
use azalea_inventory::ItemStack;
use azalea_inventory::components::EquipmentSlot;
use azalea_registry::builtin::ItemKind;

pub fn item_for(state: BlockState) -> Option<ItemKind> {
    crate::items::placed::block_item(state.as_block_kind())
}

pub fn count(bot: &Client, item: ItemKind) -> u32 {
    let Ok(inv) = bot.component::<Inventory>() else {
        return 0;
    };
    let menu = &inv.inventory_menu;
    let of = |stack: &ItemStack| {
        if stack.is_present() && stack.kind() == item {
            stack.count().max(0) as u32
        } else {
            0
        }
    };
    let slots: u32 = menu
        .player_slots_without_hotbar_range()
        .chain(menu.hotbar_slots_range())
        .filter_map(|i| menu.slot(i))
        .map(of)
        .sum();
    slots + inv.get_equipment(EquipmentSlot::Offhand).map_or(0, of)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Ready(u8),
    Moving,
    Missing,
}

enum Step {
    Select(u8),
    Swap(u16, u8),
    Creative(u16, ItemStack),
    Wait,
    Missing,
}

pub fn hold(bot: &Client, item: ItemKind, spare: u8) -> Held {
    let spare = spare.min(8);
    let creative = bot
        .component::<GameMode>()
        .is_ok_and(|gm| *gm == GameMode::Creative);
    let selected = bot.selected_hotbar_slot().unwrap_or(0);
    let step = {
        let Ok(inv) = bot.component::<Inventory>() else {
            return Held::Missing;
        };
        if inv.container_menu.is_some() || inv.carried.is_present() {
            Step::Wait
        } else {
            let menu = &inv.inventory_menu;
            let first = *menu.hotbar_slots_range().start();
            let holds = |i: usize| {
                menu.slot(i)
                    .is_some_and(|s| s.is_present() && s.kind() == item)
            };
            let empty = |i: usize| menu.slot(i).is_none_or(|s| s.is_empty());
            let hotbar = std::iter::once(selected)
                .chain(0..9)
                .find(|&h| holds(first + usize::from(h)));
            if let Some(h) = hotbar {
                Step::Select(h)
            } else if let Some(source) =
                menu.player_slots_without_hotbar_range().find(|&i| holds(i))
            {
                let target = (0..9u8)
                    .find(|&h| empty(first + usize::from(h)))
                    .unwrap_or(spare);
                Step::Swap(source as u16, target)
            } else if creative {
                Step::Creative((first + usize::from(spare)) as u16, full_stack(item))
            } else {
                Step::Missing
            }
        }
    };
    match step {
        Step::Select(h) => {
            crate::play::interaction::select_hotbar_slot(bot, h);
            Held::Ready(h)
        }
        Step::Swap(source, target) => {
            crate::play::inventory_bridge::swap_with_hotbar(bot, source, target);
            Held::Moving
        }
        Step::Creative(slot, stack) => {
            crate::play::inventory_bridge::set_creative_slot(bot, slot, stack);
            Held::Moving
        }
        Step::Wait => Held::Moving,
        Step::Missing => Held::Missing,
    }
}

fn full_stack(item: ItemKind) -> ItemStack {
    let mut stack = ItemStack::new(item, 1);
    if let Some(data) = stack.as_present_mut() {
        data.count = azalea_inventory::bundle::max_stack_size(data);
    }
    stack
}
