pub mod equipment_effects;

use azalea_chat::FormattedText;
use azalea_core::tick::GameTick;
use azalea_entity::{PlayerAbilities, inventory::Inventory as Inv};
use azalea_inventory::operations::ClickOperation;
pub use azalea_inventory::*;
use azalea_protocol::packets::game::{
    s_container_click::{HashedStack, ServerboundContainerClick},
    s_container_close::ServerboundContainerClose,
    s_set_carried_item::ServerboundSetCarriedItem,
};
use azalea_registry::builtin::MenuKind;
use azalea_world::{WorldName, Worlds};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use indexmap::IndexMap;
use tracing::{error, warn};

use crate::{
    inventory::equipment_effects::{collect_equipment_changes, handle_equipment_changes},
    packet::game::SendGamePacketEvent,
};

#[doc(hidden)]
#[deprecated = "moved to `azalea_entity::inventory::Inventory`."]
pub type Inventory = azalea_entity::inventory::Inventory;

pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            (
                ensure_has_sent_carried_item.after(super::mining::handle_mining_queued),
                collect_equipment_changes
                    .after(super::interact::handle_start_use_item_queued)
                    .before(azalea_physics::ai_step),
            ),
        )
        .add_observer(handle_client_side_close_container_trigger)
        .add_observer(handle_menu_opened_trigger)
        .add_observer(handle_container_close_event)
        .add_observer(handle_set_container_content_trigger)
        .add_observer(handle_container_click_event)
        .add_observer(handle_set_selected_hotbar_slot_event)
        .add_observer(handle_equipment_changes);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, SystemSet)]
pub struct InventorySystems;

#[derive(Clone, Debug, EntityEvent)]
pub struct MenuOpenedEvent {
    pub entity: Entity,
    pub window_id: i32,
    pub menu_type: MenuKind,
    pub title: FormattedText,
}
fn handle_menu_opened_trigger(event: On<MenuOpenedEvent>, mut query: Query<&mut Inv>) {
    let mut inventory = query.get_mut(event.entity).unwrap();
    inventory.id = event.window_id;
    inventory.container_menu = Some(Menu::from_kind(event.menu_type));
    inventory.container_menu_title = Some(event.title.clone());
}

#[derive(EntityEvent)]
pub struct CloseContainerEvent {
    pub entity: Entity,
    pub id: i32,
}
fn handle_container_close_event(
    close_container: On<CloseContainerEvent>,
    mut commands: Commands,
    query: Query<(Entity, &Inv)>,
) {
    let (entity, inventory) = query.get(close_container.entity).unwrap();
    if close_container.id != inventory.id {
        warn!(
            "Tried to close container with ID {}, but the current container ID is {}",
            close_container.id, inventory.id
        );
        return;
    }

    commands.trigger(SendGamePacketEvent::new(
        entity,
        ServerboundContainerClose {
            container_id: inventory.id,
        },
    ));
    commands.trigger(ClientsideCloseContainerEvent {
        entity: close_container.entity,
    });
}

#[derive(Clone, EntityEvent)]
pub struct ClientsideCloseContainerEvent {
    pub entity: Entity,
}
pub fn handle_client_side_close_container_trigger(
    event: On<ClientsideCloseContainerEvent>,
    mut query: Query<&mut Inv>,
) {
    let mut inventory = query.get_mut(event.entity).unwrap();

    if let Some(inventory_menu) = inventory.container_menu.take() {

        let new_inventory = inventory_menu.slots()[inventory_menu.player_slots_range()].to_vec();
        let new_inventory = <[ItemStack; 36]>::try_from(new_inventory).unwrap();
        *inventory.inventory_menu.as_player_mut().inventory = new_inventory;
    }

    inventory.id = 0;
    inventory.container_menu_title = None;
}

#[derive(Debug, EntityEvent)]
pub struct ContainerClickEvent {
    pub entity: Entity,
    pub window_id: i32,
    pub operation: ClickOperation,
}
pub fn handle_container_click_event(
    container_click: On<ContainerClickEvent>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Inv, Option<&PlayerAbilities>, &WorldName)>,
    worlds: Res<Worlds>,
) {
    let (entity, mut inventory, player_abilities, world_name) =
        query.get_mut(container_click.entity).unwrap();
    if inventory.id != container_click.window_id {
        error!(
            "Tried to click container with ID {}, but the current container ID is {}. Click packet won't be sent.",
            container_click.window_id, inventory.id
        );
        return;
    }

    let Some(world) = worlds.get(world_name) else {
        return;
    };

    let old_slots = inventory.menu().slots();
    inventory.simulate_click(
        &container_click.operation,
        player_abilities.unwrap_or(&PlayerAbilities::default()),
    );
    let new_slots = inventory.menu().slots();

    let registry_holder = &world.read().registries;

    let mut changed_slots: IndexMap<u16, HashedStack> = IndexMap::new();
    for (slot_index, old_slot) in old_slots.iter().enumerate() {
        let new_slot = &new_slots[slot_index];
        if old_slot != new_slot {
            changed_slots.insert(
                slot_index as u16,
                HashedStack::from_item_stack(new_slot, registry_holder),
            );
        }
    }

    commands.trigger(SendGamePacketEvent::new(
        entity,
        ServerboundContainerClick {
            container_id: container_click.window_id,
            state_id: inventory.menu_state_id(),
            slot_num: container_click
                .operation
                .slot_num()
                .map(|n| n as i16)
                .unwrap_or(-999),
            button_num: container_click.operation.button_num(),
            click_type: container_click.operation.click_type(),
            changed_slots,
            carried_item: HashedStack::from_item_stack(&inventory.carried, registry_holder),
        },
    ));
}

#[derive(EntityEvent)]
pub struct SetContainerContentEvent {
    pub entity: Entity,
    pub slots: Vec<ItemStack>,
    pub container_id: i32,
    pub state_id: u32,
    pub carried_item: ItemStack,
}
pub fn handle_set_container_content_trigger(
    set_container_content: On<SetContainerContentEvent>,
    mut query: Query<&mut Inv>,
) {
    let mut inventory = query.get_mut(set_container_content.entity).unwrap();

    if set_container_content.container_id != inventory.id {
        warn!(
            "Got SetContainerContentEvent for container with ID {}, but the current container ID is {}",
            set_container_content.container_id, inventory.id
        );
        return;
    }

    let menu = inventory.menu_mut();
    for (i, slot) in set_container_content.slots.iter().enumerate() {
        if let Some(slot_mut) = menu.slot_mut(i) {
            *slot_mut = slot.clone();
        }
    }
    inventory.container_state_id = set_container_content.state_id;
    inventory.carried = set_container_content.carried_item.clone();
    inventory.sync_player_slots(true);
}

pub fn trace(args: std::fmt::Arguments<'_>) {
    use std::io::Write as _;
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static FILE: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> =
        std::sync::OnceLock::new();

    if !*ON.get_or_init(|| std::env::var_os("MC_INV_TRACE").is_some()) {
        return;
    }
    eprintln!("{args}");
    let file = FILE.get_or_init(|| {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(TRACE_PATH)
        {
            Ok(f) => Some(std::sync::Mutex::new(f)),
            Err(e) => {
                eprintln!("[inv] could not open {TRACE_PATH}: {e} (stderr only)");
                None
            }
        }
    });
    if let Some(file) = file
        && let Ok(mut file) = file.lock()
    {
        let _ = writeln!(file, "{args}");
    }
}

pub const TRACE_PATH: &str = "/tmp/mc-inv-trace.log";

#[derive(EntityEvent)]
pub struct SetSelectedHotbarSlotEvent {
    pub entity: Entity,
    pub slot: u8,
}
pub fn handle_set_selected_hotbar_slot_event(
    set_selected_hotbar_slot: On<SetSelectedHotbarSlotEvent>,
    mut query: Query<&mut Inv>,
) {
    let mut inventory = query.get_mut(set_selected_hotbar_slot.entity).unwrap();
    inventory.selected_hotbar_slot = set_selected_hotbar_slot.slot;
}

#[derive(Component)]
pub struct LastSentSelectedHotbarSlot {
    pub slot: u8,
}
pub fn ensure_has_sent_carried_item(
    mut commands: Commands,
    query: Query<(Entity, &Inv, Option<&LastSentSelectedHotbarSlot>)>,
) {
    for (entity, inventory, last_sent) in query.iter() {
        if let Some(last_sent) = last_sent {
            if last_sent.slot == inventory.selected_hotbar_slot {
                continue;
            }

            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundSetCarriedItem {
                    slot: inventory.selected_hotbar_slot as u16,
                },
            ));
        }

        commands.entity(entity).insert(LastSentSelectedHotbarSlot {
            slot: inventory.selected_hotbar_slot,
        });
    }
}
